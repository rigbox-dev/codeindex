pub mod expansion;
pub mod fusion;
pub mod parser;

use anyhow::Result;

use crate::embedding::EmbeddingProvider;
use crate::model::{DependencyInfo, QueryResponse, QueryResult};
use crate::storage::sqlite::SqliteStorage;
use crate::storage::vectors::VectorIndex;

use expansion::expand_dependencies;
use fusion::reciprocal_rank_fusion;
use parser::{parse_query, ParsedQuery};

/// Options controlling query behaviour.
pub struct QueryOptions {
    /// Maximum number of results to return.
    pub top: usize,
    /// Dependency expansion depth.
    pub depth: usize,
    /// Whether to include raw code in results (currently unused in MVP — always None).
    pub include_code: bool,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            top: 5,
            depth: 1,
            include_code: true,
        }
    }
}

/// The main query engine.
pub struct QueryEngine<'a> {
    storage: &'a SqliteStorage,
    vectors: &'a VectorIndex,
    embedding_provider: &'a dyn EmbeddingProvider,
}

impl<'a> QueryEngine<'a> {
    pub fn new(
        storage: &'a SqliteStorage,
        vectors: &'a VectorIndex,
        embedding_provider: &'a dyn EmbeddingProvider,
    ) -> Self {
        Self {
            storage,
            vectors,
            embedding_provider,
        }
    }

    /// Execute a query and return a `QueryResponse`.
    pub fn query(&self, input: &str, opts: &QueryOptions) -> Result<QueryResponse> {
        let parsed = parse_query(input);

        let (ids, scores) = match &parsed {
            ParsedQuery::NaturalLanguage(text) => self.query_natural_language(text, opts)?,
            ParsedQuery::SymbolLookup(name) => self.query_symbol_lookup(name, opts)?,
            ParsedQuery::DependencyQuery { file, symbol } => {
                self.query_dependency(file, symbol, opts)?
            }
            ParsedQuery::FileScope(path) => self.query_file_scope(path, opts)?,
        };

        // Expand dependencies for all returned region ids.
        let effective_depth = match &parsed {
            ParsedQuery::DependencyQuery { .. } => opts.depth.max(2),
            _ => opts.depth,
        };

        let dep_map = expand_dependencies(self.storage, &ids, effective_depth)?;

        let results = self.build_results(&ids, &scores, &dep_map, opts)?;

        Ok(QueryResponse {
            query: input.to_string(),
            results,
            graph_context: None,
        })
    }

    // -------------------------------------------------------------------------
    // Dispatch helpers
    // -------------------------------------------------------------------------

    fn query_natural_language(
        &self,
        text: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<i64>, Vec<f64>)> {
        // FTS5 keyword search.
        let fts_ids = self.storage.search_fts(text).unwrap_or_default();
        let fts_list: Vec<(i64, f64)> = fts_ids
            .iter()
            .enumerate()
            .map(|(rank, &id)| (id, 1.0 / (1.0 + rank as f64)))
            .collect();

        // HNSW semantic search.
        let embedding = self.embedding_provider.embed(&[text.to_string()])?;
        let query_vec = &embedding[0];
        let hnsw_results = self
            .vectors
            .search(query_vec, opts.top * 2)
            .unwrap_or_default();
        let hnsw_list: Vec<(i64, f64)> = hnsw_results
            .into_iter()
            .map(|(id, sim)| (id, sim as f64))
            .collect();

        // Fuse via RRF.
        let fused = reciprocal_rank_fusion(&[fts_list, hnsw_list]);
        Ok(take_top(fused, opts.top))
    }

    fn query_symbol_lookup(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<i64>, Vec<f64>)> {
        let fts_ids = self.storage.search_fts(name).unwrap_or_default();
        let list: Vec<(i64, f64)> = fts_ids
            .iter()
            .enumerate()
            .map(|(rank, &id)| (id, 1.0 / (1.0 + rank as f64)))
            .collect();
        Ok(take_top(list, opts.top))
    }

    fn query_dependency(
        &self,
        file: &str,
        symbol: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<i64>, Vec<f64>)> {
        // Find the specific region by file path + name.
        let mut ids: Vec<i64> = Vec::new();

        if let Some(indexed_file) = self.storage.get_file_by_path(file)? {
            let regions = self.storage.get_regions_for_file(indexed_file.file_id)?;
            for region in regions {
                if region.name == symbol {
                    ids.push(region.region_id);
                    break;
                }
            }
        }

        let list: Vec<(i64, f64)> = ids.iter().map(|&id| (id, 1.0)).collect();
        Ok(take_top(list, opts.top))
    }

    fn query_file_scope(
        &self,
        path: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<i64>, Vec<f64>)> {
        let mut list: Vec<(i64, f64)> = Vec::new();

        if let Some(indexed_file) = self.storage.get_file_by_path(path)? {
            let regions = self.storage.get_regions_for_file(indexed_file.file_id)?;
            for (rank, region) in regions.iter().enumerate() {
                list.push((region.region_id, 1.0 / (1.0 + rank as f64)));
            }
        }

        // For file scope we return ALL regions (no top limit at this stage).
        let total = list.len();
        Ok(take_top(list, total.max(opts.top)))
    }

    // -------------------------------------------------------------------------
    // Result building
    // -------------------------------------------------------------------------

    /// Build `QueryResult` records from a ranked list of (region_id, score) pairs.
    fn build_results(
        &self,
        ids: &[i64],
        scores: &[f64],
        dep_map: &std::collections::HashMap<i64, DependencyInfo>,
        _opts: &QueryOptions,
    ) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (&region_id, &score) in ids.iter().zip(scores.iter()) {
            let region = match self.storage.get_region(region_id)? {
                Some(r) => r,
                None => continue,
            };

            let file = match self.storage.get_file(region.file_id)? {
                Some(f) => f,
                None => continue,
            };

            let dependencies = dep_map
                .get(&region_id)
                .cloned()
                .unwrap_or_default();

            results.push(QueryResult {
                region_id,
                file: file.path,
                kind: region.kind,
                name: region.name,
                lines: [region.start_line, region.end_line],
                signature: region.signature,
                summary: None,
                relevance: score,
                code: None,
                dependencies,
            });
        }

        Ok(results)
    }
}

// -------------------------------------------------------------------------
// Utilities
// -------------------------------------------------------------------------

/// Take the top `n` items from a fused list, returning (ids, scores) split.
fn take_top(list: Vec<(i64, f64)>, n: usize) -> (Vec<i64>, Vec<f64>) {
    let taken: Vec<(i64, f64)> = list.into_iter().take(n).collect();
    let ids = taken.iter().map(|(id, _)| *id).collect();
    let scores = taken.iter().map(|(_, s)| *s).collect();
    (ids, scores)
}

// -------------------------------------------------------------------------
// Integration tests
// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use codeindex_tree_sitter_langs::LanguageRegistry;
    use std::path::Path;

    use crate::embedding::MockEmbeddingProvider;
    use crate::indexer::pipeline::IndexPipeline;
    use crate::storage::sqlite::SqliteStorage;
    use crate::storage::vectors::VectorIndex;

    const DIM: usize = 64;

    fn fixture_root() -> std::path::PathBuf {
        let manifest = env!("CARGO_MANIFEST_DIR");
        Path::new(manifest)
            .join("../../tests/fixtures/rust-project")
            .canonicalize()
            .expect("fixture directory should exist")
    }

    /// Index the fixture project, embed all regions with MockEmbeddingProvider,
    /// and return (SqliteStorage, VectorIndex, MockEmbeddingProvider).
    fn setup() -> (SqliteStorage, VectorIndex, MockEmbeddingProvider) {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let registry = LanguageRegistry::new();
        let mut pipeline = IndexPipeline::new(storage, registry);

        let root = fixture_root();
        pipeline.index_project(&root).unwrap();

        let storage = pipeline.into_storage();
        let provider = MockEmbeddingProvider::new(DIM);

        // Build VectorIndex by embedding all regions.
        let mut vector_index = VectorIndex::new(DIM);

        // Collect all file_ids first (scope the borrow so `storage` can be moved later).
        let file_ids: Vec<i64> = {
            let mut stmt = storage
                .conn()
                .prepare("SELECT file_id FROM files")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };

        for file_id in file_ids {
            let regions = storage.get_regions_for_file(file_id).unwrap();
            for region in regions {
                let text = format!("{}: {}", region.kind, region.signature);
                let vecs = provider.embed(&[text]).unwrap();
                vector_index.add(region.region_id, vecs.into_iter().next().unwrap()).unwrap();
            }
        }

        vector_index.build().unwrap();

        (storage, vector_index, provider)
    }

    #[test]
    fn symbol_lookup_finds_struct() {
        let (storage, vectors, provider) = setup();
        let engine = QueryEngine::new(&storage, &vectors, &provider);

        let opts = QueryOptions {
            top: 10,
            depth: 1,
            include_code: false,
        };

        let response = engine.query(":symbol AuthRequest", &opts).unwrap();

        assert!(
            !response.results.is_empty(),
            "symbol lookup should return at least one result"
        );

        let found = response.results.iter().any(|r| r.name == "AuthRequest");
        assert!(found, "expected to find region named 'AuthRequest', got: {:?}",
            response.results.iter().map(|r| &r.name).collect::<Vec<_>>());
    }

    #[test]
    fn file_scope_returns_all_regions() {
        let (storage, vectors, provider) = setup();
        let engine = QueryEngine::new(&storage, &vectors, &provider);

        let opts = QueryOptions {
            top: 50,
            depth: 0,
            include_code: false,
        };

        let response = engine.query(":file src/auth/types.rs", &opts).unwrap();

        assert!(
            response.results.len() >= 4,
            "expected at least 4 regions for src/auth/types.rs, got {}",
            response.results.len()
        );
    }
}
