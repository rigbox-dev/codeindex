use anyhow::Result;
use std::path::Path;

use codeindex_tree_sitter_langs::{parse_source, LanguageRegistry};

use crate::model::{Dependency, Region};
use crate::storage::sqlite::SqliteStorage;

/// Statistics returned after indexing a project or file.
#[derive(Debug, Default, Clone)]
pub struct IndexStats {
    pub files_indexed: usize,
    pub regions_extracted: usize,
    pub dependencies_extracted: usize,
}

/// Wires together file discovery, parsing, extraction, and storage.
pub struct IndexPipeline {
    storage: SqliteStorage,
    registry: LanguageRegistry,
}

impl IndexPipeline {
    pub fn new(storage: SqliteStorage, registry: LanguageRegistry) -> Self {
        Self { storage, registry }
    }

    /// Consume self and return the underlying storage.
    pub fn into_storage(self) -> SqliteStorage {
        self.storage
    }

    /// Borrow the underlying storage.
    pub fn storage(&self) -> &SqliteStorage {
        &self.storage
    }

    /// Walk `project_root` (respecting `.gitignore`) and index every file whose
    /// extension is handled by a registered plugin.  Returns aggregate stats.
    pub fn index_project(&mut self, project_root: &Path) -> Result<IndexStats> {
        let mut stats = IndexStats::default();

        // Collect all matching paths first so we don't hold a borrow across
        // the mutable `index_file` call.
        let mut paths: Vec<std::path::PathBuf> = Vec::new();

        for result in ignore::WalkBuilder::new(project_root).build() {
            let entry = result?;
            if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                let path = entry.path().to_path_buf();
                // Check if any registered plugin handles this extension.
                if let Some(ext) = path.extension() {
                    let ext_with_dot = format!(".{}", ext.to_string_lossy());
                    if self.registry.plugin_for_extension(&ext_with_dot).is_some() {
                        paths.push(path);
                    }
                }
            }
        }

        for path in paths {
            let region_count = self.index_file(&path, project_root)?;
            stats.files_indexed += 1;
            stats.regions_extracted += region_count;
        }

        Ok(stats)
    }

    /// Index a single file: read → hash → check cache → parse → extract → store.
    /// Returns the number of regions inserted.
    pub fn index_file(&mut self, file_path: &Path, project_root: &Path) -> Result<usize> {
        // Read source bytes.
        let source = std::fs::read(file_path)?;

        // Compute blake3 hash.
        let content_hash = blake3::hash(&source).to_hex().to_string();

        // Relative path (used as the DB key).
        let rel_path = file_path
            .strip_prefix(project_root)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();

        // Determine the language plugin.
        let ext_with_dot = file_path
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();

        let plugin = match self.registry.plugin_for_extension(&ext_with_dot) {
            Some(p) => p,
            None => anyhow::bail!("no plugin for extension {}", ext_with_dot),
        };

        let language_id = plugin.language_id().to_string();

        // Check whether the file is already indexed with the same hash.
        if let Some(existing) = self.storage.get_file_by_path(&rel_path)? {
            if existing.content_hash == content_hash {
                // Unchanged — skip.
                let count = self.storage.get_regions_for_file(existing.file_id)?.len();
                return Ok(count);
            }
            // Hash changed — delete old regions (cascade handles dependencies).
            self.storage.delete_regions_for_file(existing.file_id)?;
        }

        // Upsert file record.
        let file_id = self
            .storage
            .upsert_file(&rel_path, &content_hash, &language_id)?;

        // Parse with tree-sitter.
        let tree = parse_source(plugin, &source)?;

        // Extract regions and dependencies via the plugin.
        let extracted_regions = plugin.extract_regions(&tree, &source);
        let extracted_deps = plugin.extract_dependencies(&tree, &source, &extracted_regions);

        // Insert regions and build a map from extraction index → DB region_id.
        let mut region_ids: Vec<i64> = Vec::with_capacity(extracted_regions.len());

        for extracted in &extracted_regions {
            let parent_region_id = extracted
                .parent_index
                .and_then(|idx| region_ids.get(idx).copied());

            let region_hash = blake3::hash(
                &source[extracted.start_byte as usize..extracted.end_byte as usize],
            )
            .to_hex()
            .to_string();

            let region = Region {
                region_id: 0, // assigned by DB
                file_id,
                parent_region_id,
                kind: extracted.kind.into(),
                name: extracted.name.clone(),
                start_line: extracted.start_line,
                end_line: extracted.end_line,
                start_byte: extracted.start_byte,
                end_byte: extracted.end_byte,
                signature: extracted.signature.clone(),
                content_hash: region_hash,
            };

            let rid = self.storage.insert_region(&region)?;
            region_ids.push(rid);

            // Index in FTS5.
            self.storage
                .index_region_fts(rid, &extracted.name, &extracted.signature, None)?;
        }

        // Insert dependencies.
        for extracted_dep in &extracted_deps {
            let source_region_id = match region_ids.get(extracted_dep.source_region_index) {
                Some(&id) => id,
                None => continue, // defensive: skip out-of-bounds index
            };

            let dep = Dependency {
                source_region_id,
                target_region_id: None,
                target_symbol: extracted_dep.target_symbol.clone(),
                target_path: extracted_dep.target_path.clone(),
                kind: extracted_dep.kind.into(),
            };
            self.storage.insert_dependency(&dep)?;
        }

        Ok(region_ids.len())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use codeindex_tree_sitter_langs::LanguageRegistry;

    fn make_pipeline() -> IndexPipeline {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let registry = LanguageRegistry::new();
        IndexPipeline::new(storage, registry)
    }

    fn fixture_root() -> std::path::PathBuf {
        let manifest = env!("CARGO_MANIFEST_DIR");
        Path::new(manifest)
            .join("../../tests/fixtures/rust-project")
            .canonicalize()
            .expect("fixture directory should exist")
    }

    #[test]
    fn indexes_single_rust_file() {
        let mut pipeline = make_pipeline();
        let root = fixture_root();
        let file = root.join("src/auth/types.rs");

        let region_count = pipeline.index_file(&file, &root).unwrap();

        assert!(
            region_count >= 4,
            "expected at least 4 regions (2 structs, 1 trait, 1 enum), got {}",
            region_count
        );
    }

    #[test]
    fn indexes_full_project() {
        let mut pipeline = make_pipeline();
        let root = fixture_root();

        let stats = pipeline.index_project(&root).unwrap();

        assert!(
            stats.files_indexed >= 4,
            "expected at least 4 .rs files indexed, got {}",
            stats.files_indexed
        );
        assert!(
            stats.regions_extracted > 0,
            "expected at least some regions, got 0"
        );
    }
}
