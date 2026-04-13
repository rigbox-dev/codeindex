# codeindex v0.2 Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make codeindex production-ready with real semantic search, source code in results, complete CLI, C/C++ support, and publishing infrastructure.

**Architecture:** Five independent enhancements to the existing Cargo workspace. Each task is self-contained and can be tested independently. The embedding model task is the largest — it rewires the indexing pipeline and replaces the mock provider.

**Tech Stack:** ort (ONNX Runtime), tokenizers (HuggingFace tokenizer), tree-sitter-c, tree-sitter-cpp, GitHub Actions, Homebrew

**Spec:** `docs/2026-04-12-codeindex-v02-enhancements-design.md`

---

## File Structure Changes

```
codeindex/
├── Cargo.toml                              # MODIFY: add publish metadata, new workspace deps
├── LICENSE                                 # CREATE
├── .github/
│   └── workflows/
│       ├── ci.yml                          # CREATE
│       └── release.yml                     # CREATE
├── crates/
│   ├── core/
│   │   ├── Cargo.toml                      # MODIFY: ort required, add tokenizers
│   │   └── src/
│   │       ├── embedding/
│   │       │   ├── local.rs                # REWRITE: real ONNX provider with tokenizer
│   │       │   └── mod.rs                  # MODIFY: add model download fn
│   │       ├── indexer/
│   │       │   └── pipeline.rs             # MODIFY: embed during indexing
│   │       ├── query/
│   │       │   └── mod.rs                  # MODIFY: source code in results, project_root
│   │       └── storage/
│   │           └── sqlite.rs               # MODIFY: add list_all_files, delete_file
│   ├── tree-sitter-langs/
│   │   ├── Cargo.toml                      # MODIFY: add tree-sitter-c, tree-sitter-cpp
│   │   └── src/
│   │       ├── lib.rs                      # MODIFY: register C/C++ plugins
│   │       ├── c_lang.rs                   # CREATE
│   │       └── cpp_lang.rs                 # CREATE
│   ├── cli/
│   │   ├── Cargo.toml                      # MODIFY: add daemon + mcp-server deps
│   │   └── src/
│   │       ├── main.rs                     # MODIFY: add Watch, wire all stubs
│   │       └── commands/
│   │           ├── config_cmd.rs           # CREATE
│   │           ├── gc.rs                   # CREATE
│   │           ├── stats.rs                # CREATE
│   │           ├── watch.rs                # CREATE
│   │           ├── mcp.rs                  # CREATE
│   │           ├── index.rs                # MODIFY: use real embedding provider
│   │           ├── query.rs                # MODIFY: use real provider, pass project_root
│   │           └── mod.rs                  # MODIFY: add new modules
│   ├── mcp-server/
│   │   └── src/
│   │       └── tools.rs                    # MODIFY: pass project_root to QueryEngine
│   └── daemon/                             # no changes needed
└── tests/
    └── fixtures/
        └── cpp-project/                    # CREATE
            ├── src/
            │   ├── auth.h
            │   ├── auth.cpp
            │   └── main.cpp
            └── CMakeLists.txt
```

---

### Task 1: Model Download & Real Local Embedding Provider

**Files:**
- Modify: `codeindex/crates/core/Cargo.toml`
- Modify: `codeindex/crates/core/src/embedding/mod.rs`
- Rewrite: `codeindex/crates/core/src/embedding/local.rs`

- [ ] **Step 1: Update core Cargo.toml — make ort required, add tokenizers**

In `crates/core/Cargo.toml`, replace the optional ort line and remove the feature flag:

```toml
# Replace this:
ort = { version = "2.0.0-rc.12", optional = true }

# With this:
ort = "2.0.0-rc.12"
tokenizers = { version = "0.20", default-features = false }
```

Remove the `[features]` section entirely (no more `local-embeddings` gate).

- [ ] **Step 2: Add model download function to embedding/mod.rs**

Add at the bottom of `crates/core/src/embedding/mod.rs` (before tests):

```rust
use std::path::PathBuf;

const MODEL_DIR: &str = "codeindex/models";
const MODEL_NAME: &str = "all-MiniLM-L6-v2";
const MODEL_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
const TOKENIZER_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";

/// Return the path to the model directory, downloading files if needed.
pub fn ensure_model() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(MODEL_DIR);
    std::fs::create_dir_all(&cache_dir)?;

    let model_path = cache_dir.join("model.onnx");
    let tokenizer_path = cache_dir.join("tokenizer.json");

    if !model_path.exists() {
        eprintln!("Downloading embedding model ({})...", MODEL_NAME);
        download_file(MODEL_URL, &model_path)?;
    }
    if !tokenizer_path.exists() {
        eprintln!("Downloading tokenizer...");
        download_file(TOKENIZER_URL, &tokenizer_path)?;
    }

    Ok(cache_dir)
}

fn download_file(url: &str, dest: &std::path::Path) -> Result<()> {
    let response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download {}: HTTP {}", url, response.status());
    }
    let bytes = response.bytes()?;
    std::fs::write(dest, &bytes)?;
    Ok(())
}
```

Add `use anyhow::Result;` at the top if not already there (it is).

- [ ] **Step 3: Rewrite local.rs — real ONNX provider with tokenizer**

Replace the entire content of `crates/core/src/embedding/local.rs`:

```rust
use anyhow::{Context, Result};
use ort::{inputs, Session};
use tokenizers::Tokenizer;

use super::EmbeddingProvider;

/// Embedding provider using a local ONNX sentence-transformer model.
pub struct LocalEmbeddingProvider {
    session: Session,
    tokenizer: Tokenizer,
    dimension: usize,
    model_name: String,
}

impl LocalEmbeddingProvider {
    /// Load model and tokenizer from a directory containing `model.onnx` and `tokenizer.json`.
    pub fn from_dir(dir: &std::path::Path) -> Result<Self> {
        let model_path = dir.join("model.onnx");
        let tokenizer_path = dir.join("tokenizer.json");

        let session = Session::builder()
            .context("failed to create ONNX session builder")?
            .commit_from_file(&model_path)
            .with_context(|| format!("failed to load model from {}", model_path.display()))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {}", e))?;

        // all-MiniLM-L6-v2 outputs 384 dimensions
        let dimension = 384;

        Ok(Self {
            session,
            tokenizer,
            dimension,
            model_name: "all-MiniLM-L6-v2".to_string(),
        })
    }
}

impl EmbeddingProvider for LocalEmbeddingProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());

        for text in texts {
            // Tokenize with the model's vocabulary
            let encoding = self.tokenizer.encode(text.as_str(), true)
                .map_err(|e| anyhow::anyhow!("tokenization failed: {}", e))?;

            let token_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
            let attention_mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&m| m as i64).collect();
            let token_type_ids: Vec<i64> = encoding.get_type_ids().iter().map(|&t| t as i64).collect();
            let seq_len = token_ids.len();

            let input_ids = ndarray::Array2::from_shape_vec((1, seq_len), token_ids)
                .context("failed to build input_ids tensor")?;
            let mask = ndarray::Array2::from_shape_vec((1, seq_len), attention_mask)
                .context("failed to build attention_mask tensor")?;
            let type_ids = ndarray::Array2::from_shape_vec((1, seq_len), token_type_ids)
                .context("failed to build token_type_ids tensor")?;

            let outputs = self.session.run(inputs![
                "input_ids" => input_ids.view(),
                "attention_mask" => mask.view(),
                "token_type_ids" => type_ids.view(),
            ]?).context("ONNX inference failed")?;

            let output_tensor = outputs[0]
                .try_extract_tensor::<f32>()
                .context("failed to extract output tensor")?;

            // Mean-pool over sequence dimension: shape is [1, seq_len, hidden]
            let shape = output_tensor.shape().to_vec();
            let embedding: Vec<f32> = match shape.len() {
                2 => output_tensor.iter().take(self.dimension).copied().collect(),
                3 => {
                    let hidden = shape[2];
                    let seq = shape[1];
                    let flat: Vec<f32> = output_tensor.iter().copied().collect();
                    (0..hidden.min(self.dimension))
                        .map(|h| {
                            let sum: f32 = (0..seq).map(|s| flat[s * hidden + h]).sum();
                            sum / seq as f32
                        })
                        .collect()
                }
                _ => anyhow::bail!("unexpected output tensor rank {}", shape.len()),
            };

            // L2-normalize
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            let normalized = if norm > 0.0 {
                embedding.iter().map(|x| x / norm).collect()
            } else {
                embedding
            };

            results.push(normalized);
        }

        Ok(results)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_id(&self) -> &str {
        &self.model_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::ensure_model;

    #[test]
    #[ignore = "downloads model from HuggingFace — run with: cargo test -- --ignored local"]
    fn local_provider_embeds_text() {
        let model_dir = ensure_model().expect("model download failed");
        let provider = LocalEmbeddingProvider::from_dir(&model_dir).expect("failed to load model");

        let texts = vec!["fn authenticate(user: &str) -> bool".to_string()];
        let vecs = provider.embed(&texts).expect("embedding failed");

        assert_eq!(vecs.len(), 1);
        assert_eq!(vecs[0].len(), 384);

        let norm: f32 = vecs[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4, "should be L2-normalized, got norm={}", norm);
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cd codeindex && cargo build -p codeindex-core`
Expected: compiles (ort and tokenizers now required dependencies)

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: real local embedding provider with model download and proper tokenizer"
```

---

### Task 2: Integrate Embeddings Into Indexing Pipeline

**Files:**
- Modify: `codeindex/crates/core/src/indexer/pipeline.rs`
- Modify: `codeindex/crates/core/src/storage/vectors.rs`

- [ ] **Step 1: Add embedding provider and vector index to IndexPipeline**

In `crates/core/src/indexer/pipeline.rs`, update the struct and constructor:

```rust
use crate::embedding::EmbeddingProvider;
use crate::storage::vectors::VectorIndex;

pub struct IndexPipeline {
    storage: SqliteStorage,
    registry: LanguageRegistry,
    embedding_provider: Option<Box<dyn EmbeddingProvider>>,
    vector_index: VectorIndex,
}

impl IndexPipeline {
    pub fn new(storage: SqliteStorage, registry: LanguageRegistry) -> Self {
        Self {
            storage,
            registry,
            embedding_provider: None,
            vector_index: VectorIndex::new(384),
        }
    }

    pub fn with_embedding_provider(mut self, provider: Box<dyn EmbeddingProvider>) -> Self {
        let dim = provider.dimension();
        self.embedding_provider = Some(provider);
        self.vector_index = VectorIndex::new(dim);
        self
    }
```

- [ ] **Step 2: Embed regions during index_file**

After inserting regions and dependencies in `index_file()`, add embedding logic:

```rust
        // Embed regions if provider is available.
        if let Some(provider) = &self.embedding_provider {
            let source_str = String::from_utf8_lossy(&source);
            for (i, extracted) in extracted_regions.iter().enumerate() {
                let region_id = region_ids[i];
                let region_source = &source_str[extracted.start_byte as usize..extracted.end_byte as usize];
                let dep_symbols: Vec<String> = extracted_deps.iter()
                    .filter(|d| d.source_region_index == i)
                    .map(|d| d.target_symbol.clone())
                    .collect();

                let text = crate::embedding::compose_embedding_text(
                    &language_id,
                    &Region {
                        region_id,
                        file_id,
                        parent_region_id: None,
                        kind: extracted.kind.into(),
                        name: extracted.name.clone(),
                        start_line: extracted.start_line,
                        end_line: extracted.end_line,
                        start_byte: extracted.start_byte,
                        end_byte: extracted.end_byte,
                        signature: extracted.signature.clone(),
                        content_hash: String::new(),
                    },
                    None,
                    region_source,
                    &dep_symbols,
                );

                let vecs = provider.embed(&[text])?;
                if let Some(vec) = vecs.into_iter().next() {
                    let _ = self.vector_index.add(region_id, vec);
                }
            }
        }
```

- [ ] **Step 3: Build and save vectors at end of index_project**

At the end of `index_project()`, after the file loop:

```rust
        // Build and persist the vector index.
        if self.embedding_provider.is_some() {
            self.vector_index.build()?;
            let index_dir = project_root.join(".codeindex");
            if index_dir.exists() {
                self.vector_index.save(&index_dir)?;
            }
        }
```

- [ ] **Step 4: Add into_vector_index method**

```rust
    pub fn into_parts(self) -> (SqliteStorage, VectorIndex) {
        (self.storage, self.vector_index)
    }
```

- [ ] **Step 5: Update VectorIndex save/load to actually persist**

In `crates/core/src/storage/vectors.rs`, replace the MVP stub `save`/`load` with a real implementation using serde:

```rust
    pub fn save(&self, dir: &std::path::Path) -> Result<()> {
        // Serialize all points and their region IDs
        let data: Vec<(Vec<f32>, i64)> = match &self.map {
            Some(map) => {
                // Rebuild from the map's values — we need to re-extract points
                // For MVP: store the raw staging data before build
                vec![] // Will use pre_build_data instead
            }
            None => vec![],
        };
        // Use the pre-build snapshot
        let json = serde_json::to_vec(&self.snapshot)?;
        std::fs::write(dir.join("vectors.json"), json)?;
        Ok(())
    }

    pub fn load(dir: &std::path::Path, dimension: usize) -> Result<Self> {
        let path = dir.join("vectors.json");
        if !path.exists() {
            return Ok(Self::new(dimension));
        }
        let data: Vec<(Vec<f32>, i64)> = serde_json::from_slice(&std::fs::read(&path)?)?;
        let mut index = Self::new(dimension);
        for (vec, id) in data {
            index.add(id, vec)?;
        }
        if !index.points.is_empty() {
            index.build()?;
        }
        Ok(index)
    }
```

Also add a `snapshot: Vec<(Vec<f32>, i64)>` field to `VectorIndex` that gets populated during `add()` and preserved through `build()`.

- [ ] **Step 6: Run existing tests**

Run: `cd codeindex && cargo test -p codeindex-core`
Expected: all existing tests pass

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: integrate embeddings into indexing pipeline with vector persistence"
```

---

### Task 3: Wire Real Embedding Provider Into CLI

**Files:**
- Modify: `codeindex/crates/cli/src/commands/index.rs`
- Modify: `codeindex/crates/cli/src/commands/query.rs`

- [ ] **Step 1: Update index.rs to use real provider**

Replace `crates/cli/src/commands/index.rs`:

```rust
use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::embedding::{self, EmbeddingProvider, MockEmbeddingProvider};
use codeindex_core::embedding::local::LocalEmbeddingProvider;
use codeindex_core::embedding::voyage::VoyageEmbeddingProvider;
use codeindex_core::indexer::pipeline::IndexPipeline;
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_tree_sitter_langs::LanguageRegistry;

pub fn run(incremental: bool, quiet: bool) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);

    if !index_dir.exists() {
        anyhow::bail!("Index directory '{}' does not exist. Run `codeindex init` first.", index_dir.display());
    }

    let db_path = index_dir.join("index.db");
    let storage = SqliteStorage::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

    let registry = LanguageRegistry::new();

    // Create embedding provider based on config
    let provider: Box<dyn EmbeddingProvider> = create_embedding_provider(&config);

    let mut pipeline = IndexPipeline::new(storage, registry)
        .with_embedding_provider(provider);

    if !quiet {
        println!("Indexing project at {} ...", project_root.display());
    }

    let stats = pipeline.index_project(&project_root)
        .with_context(|| "Indexing failed")?;

    if !quiet {
        println!("Done. Indexed {} file(s), {} region(s), {} dependency(s).",
            stats.files_indexed, stats.regions_extracted, stats.dependencies_extracted);
    }

    Ok(())
}

fn create_embedding_provider(config: &Config) -> Box<dyn EmbeddingProvider> {
    match config.embedding.provider.as_str() {
        "voyage" => {
            match VoyageEmbeddingProvider::from_env(
                config.embedding.model.as_deref().unwrap_or("voyage-code-3")
            ) {
                Ok(p) => Box::new(p),
                Err(e) => {
                    eprintln!("Warning: Voyage provider failed ({}), falling back to local.", e);
                    create_local_provider()
                }
            }
        }
        _ => create_local_provider(),
    }
}

fn create_local_provider() -> Box<dyn EmbeddingProvider> {
    match embedding::ensure_model().and_then(|dir| LocalEmbeddingProvider::from_dir(&dir)) {
        Ok(p) => Box::new(p),
        Err(e) => {
            eprintln!("Warning: Local model failed ({}), using mock embeddings.", e);
            Box::new(MockEmbeddingProvider::new(384))
        }
    }
}
```

- [ ] **Step 2: Update query.rs to use real provider and load persisted vectors**

Replace `crates/cli/src/commands/query.rs`:

```rust
use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::embedding::{self, EmbeddingProvider, MockEmbeddingProvider};
use codeindex_core::embedding::local::LocalEmbeddingProvider;
use codeindex_core::embedding::voyage::VoyageEmbeddingProvider;
use codeindex_core::query::{QueryEngine, QueryOptions};
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_core::storage::vectors::VectorIndex;

use crate::output;

pub fn run(
    query: String,
    top: usize,
    depth: usize,
    include_code: bool,
    json: bool,
    format: String,
) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);

    if !index_dir.exists() {
        eprintln!("No index found. Run `codeindex init && codeindex index` first.");
        std::process::exit(2);
    }

    let db_path = index_dir.join("index.db");
    if !db_path.exists() {
        eprintln!("Database not found. Run `codeindex index` to build the index.");
        std::process::exit(2);
    }

    let storage = SqliteStorage::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

    // Create embedding provider based on config
    let provider: Box<dyn EmbeddingProvider> = match config.embedding.provider.as_str() {
        "voyage" => {
            match VoyageEmbeddingProvider::from_env(
                config.embedding.model.as_deref().unwrap_or("voyage-code-3")
            ) {
                Ok(p) => Box::new(p),
                Err(_) => create_local_or_mock(),
            }
        }
        _ => create_local_or_mock(),
    };

    let dim = provider.dimension();
    let vector_index = VectorIndex::load(&index_dir, dim)
        .unwrap_or_else(|_| VectorIndex::new(dim));

    let engine = QueryEngine::new(&storage, &vector_index, provider.as_ref(), &project_root);

    let opts = QueryOptions { top, depth, include_code };
    let response = engine.query(&query, &opts).with_context(|| "Query failed")?;

    if response.results.is_empty() {
        if json || format == "json" || !atty::is(atty::Stream::Stdout) {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            println!("No results found for: {}", query);
        }
        std::process::exit(1);
    }

    let use_json = json || format == "json" || !atty::is(atty::Stream::Stdout);
    let use_compact = format == "compact";

    if use_json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if use_compact {
        output::print_compact(&response);
    } else {
        output::print_human(&response);
    }

    Ok(())
}

fn create_local_or_mock() -> Box<dyn EmbeddingProvider> {
    match embedding::ensure_model().and_then(|dir| LocalEmbeddingProvider::from_dir(&dir)) {
        Ok(p) => Box::new(p),
        Err(_) => Box::new(MockEmbeddingProvider::new(384)),
    }
}
```

- [ ] **Step 3: Verify build**

Run: `cd codeindex && cargo build -p codeindex`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: wire real embedding provider into CLI index and query commands"
```

---

### Task 4: Source Code in Query Results

**Files:**
- Modify: `codeindex/crates/core/src/query/mod.rs`
- Modify: `codeindex/crates/mcp-server/src/tools.rs`

- [ ] **Step 1: Add project_root to QueryEngine and implement code extraction**

In `crates/core/src/query/mod.rs`, update `QueryEngine`:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct QueryEngine<'a> {
    storage: &'a SqliteStorage,
    vectors: &'a VectorIndex,
    embedding_provider: &'a dyn EmbeddingProvider,
    project_root: PathBuf,
}

impl<'a> QueryEngine<'a> {
    pub fn new(
        storage: &'a SqliteStorage,
        vectors: &'a VectorIndex,
        embedding_provider: &'a dyn EmbeddingProvider,
        project_root: &Path,
    ) -> Self {
        Self {
            storage,
            vectors,
            embedding_provider,
            project_root: project_root.to_path_buf(),
        }
    }
```

Update `build_results` to read source code:

```rust
    fn build_results(
        &self,
        ids: &[i64],
        scores: &[f64],
        dep_map: &std::collections::HashMap<i64, DependencyInfo>,
        opts: &QueryOptions,
    ) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();
        let mut file_cache: HashMap<String, Vec<u8>> = HashMap::new();

        for (&region_id, &score) in ids.iter().zip(scores.iter()) {
            let region = match self.storage.get_region(region_id)? {
                Some(r) => r,
                None => continue,
            };

            let file = match self.storage.get_file(region.file_id)? {
                Some(f) => f,
                None => continue,
            };

            let code = if opts.include_code {
                let source = file_cache.entry(file.path.clone()).or_insert_with(|| {
                    let full_path = self.project_root.join(&file.path);
                    std::fs::read(&full_path).unwrap_or_default()
                });
                let start = region.start_byte as usize;
                let end = (region.end_byte as usize).min(source.len());
                if start < end {
                    Some(String::from_utf8_lossy(&source[start..end]).to_string())
                } else {
                    None
                }
            } else {
                None
            };

            let dependencies = dep_map.get(&region_id).cloned().unwrap_or_default();

            results.push(QueryResult {
                region_id,
                file: file.path,
                kind: region.kind,
                name: region.name,
                lines: [region.start_line, region.end_line],
                signature: region.signature,
                summary: None,
                relevance: score,
                code,
                dependencies,
            });
        }

        Ok(results)
    }
```

- [ ] **Step 2: Update all QueryEngine::new call sites**

Update the test `setup()` in `query/mod.rs` tests to pass a fixture project_root:

```rust
    let engine = QueryEngine::new(&storage, &vectors, &provider, &fixture_root());
```

Update `crates/mcp-server/src/tools.rs` to pass `self.project_root` to `QueryEngine::new`.

- [ ] **Step 3: Run tests**

Run: `cd codeindex && cargo test --workspace`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat: return source code in query results when include_code is true"
```

---

### Task 5: C Language Plugin

**Files:**
- Modify: `codeindex/crates/tree-sitter-langs/Cargo.toml`
- Create: `codeindex/crates/tree-sitter-langs/src/c_lang.rs`
- Modify: `codeindex/crates/tree-sitter-langs/src/lib.rs`
- Create: `codeindex/tests/fixtures/cpp-project/src/auth.h`
- Create: `codeindex/tests/fixtures/cpp-project/src/auth.cpp`
- Create: `codeindex/tests/fixtures/cpp-project/src/main.cpp`

- [ ] **Step 1: Add tree-sitter-c dependency**

Add to `crates/tree-sitter-langs/Cargo.toml`:
```toml
tree-sitter-c = "0.23"
```

- [ ] **Step 2: Create test fixtures**

```c
// tests/fixtures/cpp-project/src/auth.h
#ifndef AUTH_H
#define AUTH_H

struct AuthRequest {
    const char* username;
    const char* password;
};

struct AuthToken {
    const char* token;
    int expires_at;
};

enum AuthError {
    AUTH_OK = 0,
    AUTH_INVALID_CREDENTIALS,
    AUTH_EXPIRED,
};

int authenticate(struct AuthRequest* req, struct AuthToken* out);
int verify_password(const char* password, const char* hash);

#endif
```

```c
// tests/fixtures/cpp-project/src/auth.cpp
#include "auth.h"
#include <string.h>

int verify_password(const char* password, const char* hash) {
    return strcmp(password, hash) == 0;
}

int authenticate(struct AuthRequest* req, struct AuthToken* out) {
    if (!verify_password(req->password, "secret")) {
        return AUTH_INVALID_CREDENTIALS;
    }
    out->token = "jwt-token";
    out->expires_at = 3600;
    return AUTH_OK;
}
```

```c
// tests/fixtures/cpp-project/src/main.cpp
#include "auth.h"
#include <stdio.h>

int main() {
    struct AuthRequest req = {"admin", "secret"};
    struct AuthToken token;

    int result = authenticate(&req, &token);
    if (result == AUTH_OK) {
        printf("Authenticated: %s\n", token.token);
    }
    return 0;
}
```

- [ ] **Step 3: Implement C plugin**

```rust
// crates/tree-sitter-langs/src/c_lang.rs

use crate::{ExtractedDependency, ExtractedRegion, LanguagePlugin, DependencyKind, RegionKind};
use tree_sitter::{Language, Node, Tree};

pub struct CPlugin;

impl LanguagePlugin for CPlugin {
    fn language_id(&self) -> &str { "c" }
    fn file_extensions(&self) -> &[&str] { &[".c", ".h"] }
    fn tree_sitter_language(&self) -> Language { tree_sitter_c::LANGUAGE.into() }

    fn extract_regions(&self, tree: &Tree, source: &[u8]) -> Vec<ExtractedRegion> {
        let mut regions = Vec::new();
        walk_c_regions(tree.root_node(), source, &mut regions);
        regions
    }

    fn extract_dependencies(&self, tree: &Tree, source: &[u8], regions: &[ExtractedRegion]) -> Vec<ExtractedDependency> {
        let mut deps = Vec::new();
        walk_c_deps(tree.root_node(), source, regions, &mut deps);
        deps
    }
}

pub fn walk_c_regions(node: Node, source: &[u8], regions: &mut Vec<ExtractedRegion>) {
    match node.kind() {
        "function_definition" => {
            let name = node.child_by_field_name("declarator")
                .and_then(|d| extract_function_name(d, source))
                .unwrap_or_default();
            if !name.is_empty() {
                let sig = extract_to_body(&node, source);
                regions.push(ExtractedRegion {
                    kind: RegionKind::Function,
                    name, signature: sig,
                    start_line: node.start_position().row as u32 + 1,
                    end_line: node.end_position().row as u32 + 1,
                    start_byte: node.start_byte() as u32,
                    end_byte: node.end_byte() as u32,
                    parent_index: None,
                });
            }
        }
        "struct_specifier" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source).unwrap_or("").to_string();
                if !name.is_empty() && node.child_by_field_name("body").is_some() {
                    let sig = first_line(&node, source);
                    regions.push(ExtractedRegion {
                        kind: RegionKind::Struct,
                        name, signature: sig,
                        start_line: node.start_position().row as u32 + 1,
                        end_line: node.end_position().row as u32 + 1,
                        start_byte: node.start_byte() as u32,
                        end_byte: node.end_byte() as u32,
                        parent_index: None,
                    });
                }
            }
        }
        "enum_specifier" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source).unwrap_or("").to_string();
                if !name.is_empty() && node.child_by_field_name("body").is_some() {
                    let sig = first_line(&node, source);
                    regions.push(ExtractedRegion {
                        kind: RegionKind::Enum,
                        name, signature: sig,
                        start_line: node.start_position().row as u32 + 1,
                        end_line: node.end_position().row as u32 + 1,
                        start_byte: node.start_byte() as u32,
                        end_byte: node.end_byte() as u32,
                        parent_index: None,
                    });
                }
            }
        }
        _ => {}
    }
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk_c_regions(child, source, regions);
    }
}

pub fn walk_c_deps(node: Node, source: &[u8], regions: &[ExtractedRegion], deps: &mut Vec<ExtractedDependency>) {
    match node.kind() {
        "preproc_include" => {
            let path = node.child_by_field_name("path")
                .map(|n| n.utf8_text(source).unwrap_or("").trim_matches(|c| c == '"' || c == '<' || c == '>').to_string())
                .unwrap_or_default();
            if !path.is_empty() {
                let source_idx = crate::find_enclosing_region(node, regions).unwrap_or(0);
                deps.push(ExtractedDependency {
                    source_region_index: source_idx,
                    target_symbol: path.clone(),
                    target_path: Some(path),
                    kind: DependencyKind::Imports,
                });
            }
        }
        "call_expression" => {
            if let Some(func) = node.child_by_field_name("function") {
                let name = func.utf8_text(source).unwrap_or("").to_string();
                if !name.is_empty() {
                    let source_idx = crate::find_enclosing_region(node, regions).unwrap_or(0);
                    deps.push(ExtractedDependency {
                        source_region_index: source_idx,
                        target_symbol: name,
                        target_path: None,
                        kind: DependencyKind::Calls,
                    });
                }
            }
        }
        _ => {}
    }
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk_c_deps(child, source, regions, deps);
    }
}

fn extract_function_name(declarator: Node, source: &[u8]) -> Option<String> {
    // Walk into nested declarators to find the identifier
    let mut node = declarator;
    loop {
        match node.kind() {
            "identifier" => return Some(node.utf8_text(source).unwrap_or("").to_string()),
            "function_declarator" | "pointer_declarator" | "parenthesized_declarator" => {
                if let Some(child) = node.child_by_field_name("declarator") {
                    node = child;
                    continue;
                }
                // Try first named child
                let cursor = &mut node.walk();
                for child in node.children(cursor) {
                    if child.kind() == "identifier" {
                        return Some(child.utf8_text(source).unwrap_or("").to_string());
                    }
                }
                return None;
            }
            _ => return None,
        }
    }
}

fn extract_to_body(node: &Node, source: &[u8]) -> String {
    let body = node.child_by_field_name("body");
    let end = body.map(|b| b.start_byte()).unwrap_or(node.end_byte());
    std::str::from_utf8(&source[node.start_byte()..end]).unwrap_or("").trim().to_string()
}

fn first_line(node: &Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").lines().next().unwrap_or("").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_source;

    #[test]
    fn extracts_c_functions() {
        let source = br#"
int add(int a, int b) {
    return a + b;
}

void greet(const char* name) {
    printf("Hello %s\n", name);
}
"#;
        let plugin = CPlugin;
        let tree = parse_source(&plugin, source).unwrap();
        let regions = plugin.extract_regions(&tree, source);
        let funcs: Vec<_> = regions.iter().filter(|r| r.kind == RegionKind::Function).collect();
        assert_eq!(funcs.len(), 2);
        assert_eq!(funcs[0].name, "add");
        assert_eq!(funcs[1].name, "greet");
    }

    #[test]
    fn extracts_c_structs() {
        let source = br#"
struct Point {
    int x;
    int y;
};
"#;
        let plugin = CPlugin;
        let tree = parse_source(&plugin, source).unwrap();
        let regions = plugin.extract_regions(&tree, source);
        let structs: Vec<_> = regions.iter().filter(|r| r.kind == RegionKind::Struct).collect();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "Point");
    }

    #[test]
    fn extracts_c_enums() {
        let source = br#"
enum Color {
    RED,
    GREEN,
    BLUE,
};
"#;
        let plugin = CPlugin;
        let tree = parse_source(&plugin, source).unwrap();
        let regions = plugin.extract_regions(&tree, source);
        let enums: Vec<_> = regions.iter().filter(|r| r.kind == RegionKind::Enum).collect();
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "Color");
    }

    #[test]
    fn extracts_c_includes() {
        let source = br#"
#include "auth.h"
#include <stdio.h>

int main() { return 0; }
"#;
        let plugin = CPlugin;
        let tree = parse_source(&plugin, source).unwrap();
        let regions = plugin.extract_regions(&tree, source);
        let deps = plugin.extract_dependencies(&tree, source, &regions);
        let imports: Vec<_> = deps.iter().filter(|d| d.kind == DependencyKind::Imports).collect();
        assert!(imports.iter().any(|d| d.target_symbol == "auth.h"));
        assert!(imports.iter().any(|d| d.target_symbol == "stdio.h"));
    }
}
```

- [ ] **Step 4: Register in lib.rs**

Add `pub mod c_lang;` to `crates/tree-sitter-langs/src/lib.rs`.

Add to `LanguageRegistry::new()`:
```rust
registry.register(Box::new(c_lang::CPlugin));
```

- [ ] **Step 5: Run tests**

Run: `cd codeindex && cargo test -p codeindex-tree-sitter-langs c_lang`
Expected: 4 tests pass

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: add C language plugin with region and dependency extraction"
```

---

### Task 6: C++ Language Plugin

**Files:**
- Modify: `codeindex/crates/tree-sitter-langs/Cargo.toml`
- Create: `codeindex/crates/tree-sitter-langs/src/cpp_lang.rs`
- Modify: `codeindex/crates/tree-sitter-langs/src/lib.rs`

- [ ] **Step 1: Add tree-sitter-cpp dependency**

Add to `crates/tree-sitter-langs/Cargo.toml`:
```toml
tree-sitter-cpp = "0.23"
```

- [ ] **Step 2: Implement C++ plugin**

```rust
// crates/tree-sitter-langs/src/cpp_lang.rs

use crate::{ExtractedDependency, ExtractedRegion, LanguagePlugin, DependencyKind, RegionKind};
use crate::c_lang::{walk_c_regions, walk_c_deps};
use tree_sitter::{Language, Node, Tree};

pub struct CppPlugin;

impl LanguagePlugin for CppPlugin {
    fn language_id(&self) -> &str { "cpp" }
    fn file_extensions(&self) -> &[&str] { &[".cpp", ".hpp", ".cc", ".cxx", ".hh"] }
    fn tree_sitter_language(&self) -> Language { tree_sitter_cpp::LANGUAGE.into() }

    fn extract_regions(&self, tree: &Tree, source: &[u8]) -> Vec<ExtractedRegion> {
        let mut regions = Vec::new();
        walk_cpp_regions(tree.root_node(), source, &mut regions, None);
        regions
    }

    fn extract_dependencies(&self, tree: &Tree, source: &[u8], regions: &[ExtractedRegion]) -> Vec<ExtractedDependency> {
        let mut deps = Vec::new();
        walk_c_deps(tree.root_node(), source, regions, &mut deps);
        deps
    }
}

fn walk_cpp_regions(node: Node, source: &[u8], regions: &mut Vec<ExtractedRegion>, parent_index: Option<usize>) {
    match node.kind() {
        "class_specifier" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                let name = name_node.utf8_text(source).unwrap_or("").to_string();
                if !name.is_empty() {
                    let sig = first_line(&node, source);
                    let idx = regions.len();
                    regions.push(ExtractedRegion {
                        kind: RegionKind::Class,
                        name, signature: sig,
                        start_line: node.start_position().row as u32 + 1,
                        end_line: node.end_position().row as u32 + 1,
                        start_byte: node.start_byte() as u32,
                        end_byte: node.end_byte() as u32,
                        parent_index,
                    });
                    // Recurse into class body for methods
                    if let Some(body) = node.child_by_field_name("body") {
                        let cursor = &mut body.walk();
                        for child in body.children(cursor) {
                            walk_cpp_regions(child, source, regions, Some(idx));
                        }
                    }
                    return;
                }
            }
        }
        "namespace_definition" => {
            let name = node.child_by_field_name("name")
                .map(|n| n.utf8_text(source).unwrap_or("").to_string())
                .unwrap_or_else(|| "anonymous".to_string());
            let sig = first_line(&node, source);
            let idx = regions.len();
            regions.push(ExtractedRegion {
                kind: RegionKind::Module,
                name, signature: sig,
                start_line: node.start_position().row as u32 + 1,
                end_line: node.end_position().row as u32 + 1,
                start_byte: node.start_byte() as u32,
                end_byte: node.end_byte() as u32,
                parent_index,
            });
            if let Some(body) = node.child_by_field_name("body") {
                let cursor = &mut body.walk();
                for child in body.children(cursor) {
                    walk_cpp_regions(child, source, regions, Some(idx));
                }
            }
            return;
        }
        "function_definition" => {
            let name = node.child_by_field_name("declarator")
                .and_then(|d| crate::c_lang::extract_function_name(d, source))
                .unwrap_or_default();
            if !name.is_empty() {
                let kind = if parent_index.is_some() { RegionKind::Method } else { RegionKind::Function };
                let sig = extract_to_body(&node, source);
                regions.push(ExtractedRegion {
                    kind, name, signature: sig,
                    start_line: node.start_position().row as u32 + 1,
                    end_line: node.end_position().row as u32 + 1,
                    start_byte: node.start_byte() as u32,
                    end_byte: node.end_byte() as u32,
                    parent_index,
                });
            }
        }
        // Delegate C-style nodes
        "struct_specifier" | "enum_specifier" => {
            walk_c_regions(node, source, regions);
            return;
        }
        _ => {}
    }
    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk_cpp_regions(child, source, regions, parent_index);
    }
}

fn extract_to_body(node: &Node, source: &[u8]) -> String {
    let body = node.child_by_field_name("body");
    let end = body.map(|b| b.start_byte()).unwrap_or(node.end_byte());
    std::str::from_utf8(&source[node.start_byte()..end]).unwrap_or("").trim().to_string()
}

fn first_line(node: &Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").lines().next().unwrap_or("").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_source;

    #[test]
    fn extracts_cpp_classes_and_methods() {
        let source = br#"
class AuthHandler {
public:
    int authenticate(const char* user, const char* pass) {
        return verify(pass);
    }

private:
    int verify(const char* pass) {
        return 1;
    }
};
"#;
        let plugin = CppPlugin;
        let tree = parse_source(&plugin, source).unwrap();
        let regions = plugin.extract_regions(&tree, source);
        let classes: Vec<_> = regions.iter().filter(|r| r.kind == RegionKind::Class).collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "AuthHandler");
        let methods: Vec<_> = regions.iter().filter(|r| r.kind == RegionKind::Method).collect();
        assert!(methods.len() >= 2, "expected at least 2 methods, got {}", methods.len());
    }

    #[test]
    fn extracts_cpp_namespaces() {
        let source = br#"
namespace auth {
    int login(const char* user) {
        return 1;
    }
}
"#;
        let plugin = CppPlugin;
        let tree = parse_source(&plugin, source).unwrap();
        let regions = plugin.extract_regions(&tree, source);
        let namespaces: Vec<_> = regions.iter().filter(|r| r.kind == RegionKind::Module).collect();
        assert_eq!(namespaces.len(), 1);
        assert_eq!(namespaces[0].name, "auth");
        let funcs: Vec<_> = regions.iter().filter(|r| r.kind == RegionKind::Function || r.kind == RegionKind::Method).collect();
        assert!(funcs.len() >= 1);
    }

    #[test]
    fn extracts_cpp_standalone_functions() {
        let source = br#"
int add(int a, int b) {
    return a + b;
}
"#;
        let plugin = CppPlugin;
        let tree = parse_source(&plugin, source).unwrap();
        let regions = plugin.extract_regions(&tree, source);
        let funcs: Vec<_> = regions.iter().filter(|r| r.kind == RegionKind::Function).collect();
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0].name, "add");
    }
}
```

Make `extract_function_name` pub in c_lang.rs so cpp_lang can use it.

- [ ] **Step 3: Register in lib.rs**

Add `pub mod cpp_lang;` and register:
```rust
registry.register(Box::new(cpp_lang::CppPlugin));
```

- [ ] **Step 4: Run tests**

Run: `cd codeindex && cargo test -p codeindex-tree-sitter-langs cpp_lang`
Expected: 3 tests pass

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: add C++ language plugin with classes, namespaces, and methods"
```

---

### Task 7: CLI — config, gc, stats commands

**Files:**
- Create: `codeindex/crates/cli/src/commands/config_cmd.rs`
- Create: `codeindex/crates/cli/src/commands/gc.rs`
- Create: `codeindex/crates/cli/src/commands/stats.rs`
- Modify: `codeindex/crates/cli/src/commands/mod.rs`
- Modify: `codeindex/crates/cli/src/main.rs`
- Modify: `codeindex/crates/core/src/storage/sqlite.rs`

- [ ] **Step 1: Add helper methods to SqliteStorage**

Add to `crates/core/src/storage/sqlite.rs`:

```rust
    pub fn list_all_files(&self) -> Result<Vec<IndexedFile>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_id, path, content_hash, language, last_indexed_at FROM files ORDER BY path"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(IndexedFile {
                file_id: row.get(0)?,
                path: row.get(1)?,
                content_hash: row.get(2)?,
                language: row.get(3)?,
                last_indexed_at: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn delete_file(&self, file_id: i64) -> Result<()> {
        // Cascade: delete regions (which cascades to deps) and the file record
        self.conn.execute("DELETE FROM regions WHERE file_id = ?1", [file_id])?;
        self.conn.execute("DELETE FROM files WHERE file_id = ?1", [file_id])?;
        Ok(())
    }

    pub fn vacuum(&self) -> Result<()> {
        self.conn.execute_batch("VACUUM")?;
        Ok(())
    }
```

- [ ] **Step 2: Implement config_cmd.rs**

```rust
// crates/cli/src/commands/config_cmd.rs
use anyhow::Result;
use codeindex_core::config::Config;

pub fn run_show() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    println!("{}", serde_json::to_string_pretty(&config)?);
    Ok(())
}

pub fn run_set(key: &str, value: &str) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let mut config = Config::load(&project_root).unwrap_or_default();

    match key {
        "embedding.provider" => config.embedding.provider = value.to_string(),
        "embedding.model" => config.embedding.model = Some(value.to_string()),
        "summary.enabled" => config.summary.enabled = value.parse().map_err(|_| anyhow::anyhow!("expected true or false"))?,
        "summary.model" => config.summary.model = value.to_string(),
        "query.default_top" => config.query.default_top = value.parse()?,
        "query.default_depth" => config.query.default_depth = value.parse()?,
        "daemon.debounce_ms" => config.daemon.debounce_ms = value.parse()?,
        "index.git_hooks" => config.index.git_hooks = value.parse().map_err(|_| anyhow::anyhow!("expected true or false"))?,
        _ => anyhow::bail!("Unknown config key: {}", key),
    }

    config.save(&project_root)?;
    println!("Set {} = {}", key, value);
    Ok(())
}
```

- [ ] **Step 3: Implement gc.rs**

```rust
// crates/cli/src/commands/gc.rs
use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::storage::sqlite::SqliteStorage;

pub fn run() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);
    let db_path = index_dir.join("index.db");

    if !db_path.exists() {
        anyhow::bail!("No index found. Run `codeindex init && codeindex index` first.");
    }

    let storage = SqliteStorage::open(&db_path)
        .with_context(|| "Failed to open database")?;

    let files = storage.list_all_files()?;
    let mut removed = 0;

    for file in &files {
        let full_path = project_root.join(&file.path);
        if !full_path.exists() {
            storage.delete_file(file.file_id)?;
            removed += 1;
        }
    }

    storage.vacuum()?;

    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    println!("Removed {} orphaned file(s). Index size: {:.1} KB", removed, db_size as f64 / 1024.0);
    Ok(())
}
```

- [ ] **Step 4: Implement stats.rs**

```rust
// crates/cli/src/commands/stats.rs
use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::storage::sqlite::SqliteStorage;

pub fn run() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);
    let db_path = index_dir.join("index.db");

    if !db_path.exists() {
        anyhow::bail!("No index found.");
    }

    let storage = SqliteStorage::open(&db_path)
        .with_context(|| "Failed to open database")?;
    let conn = storage.conn();

    // Files by language
    println!("=== Files by Language ===");
    let mut stmt = conn.prepare("SELECT language, COUNT(*) FROM files GROUP BY language ORDER BY COUNT(*) DESC")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (lang, count) = row?;
        println!("  {}: {}", lang, count);
    }

    // Regions by kind
    println!("\n=== Regions by Kind ===");
    let mut stmt = conn.prepare("SELECT kind, COUNT(*) FROM regions GROUP BY kind ORDER BY COUNT(*) DESC")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (kind, count) = row?;
        println!("  {}: {}", kind, count);
    }

    // Dependencies by kind
    println!("\n=== Dependencies by Kind ===");
    let mut stmt = conn.prepare("SELECT kind, COUNT(*) FROM dependencies GROUP BY kind ORDER BY COUNT(*) DESC")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (kind, count) = row?;
        println!("  {}: {}", kind, count);
    }

    // Index size
    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    let vec_size = std::fs::metadata(index_dir.join("vectors.json")).map(|m| m.len()).unwrap_or(0);
    println!("\n=== Index Size ===");
    println!("  Database: {:.1} KB", db_size as f64 / 1024.0);
    println!("  Vectors:  {:.1} KB", vec_size as f64 / 1024.0);
    println!("  Total:    {:.1} KB", (db_size + vec_size) as f64 / 1024.0);

    // Last indexed
    let last: Option<i64> = conn.query_row(
        "SELECT MAX(last_indexed_at) FROM files", [], |row| row.get(0)
    ).ok().flatten();
    if let Some(ts) = last {
        println!("\nLast indexed: {} (unix timestamp)", ts);
    }

    Ok(())
}
```

- [ ] **Step 5: Update main.rs to wire commands**

Update the `Commands` enum in `main.rs`:

```rust
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
```

Add:
```rust
#[derive(Subcommand)]
enum ConfigAction {
    /// Set a config value
    Set { key: String, value: String },
}
```

Update the match arms:
```rust
        Commands::Config { action } => match action {
            Some(ConfigAction::Set { key, value }) => commands::config_cmd::run_set(&key, &value),
            None => commands::config_cmd::run_show(),
        },
        Commands::Gc => commands::gc::run(),
        Commands::Stats => commands::stats::run(),
```

Update `commands/mod.rs`:
```rust
pub mod config_cmd;
pub mod gc;
pub mod stats;
```

- [ ] **Step 6: Build and verify**

Run: `cd codeindex && cargo build -p codeindex`
Expected: compiles

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: implement config, gc, and stats CLI commands"
```

---

### Task 8: CLI — watch and mcp-server commands

**Files:**
- Create: `codeindex/crates/cli/src/commands/watch.rs`
- Create: `codeindex/crates/cli/src/commands/mcp.rs`
- Modify: `codeindex/crates/cli/Cargo.toml`
- Modify: `codeindex/crates/cli/src/main.rs`
- Modify: `codeindex/crates/cli/src/commands/mod.rs`

- [ ] **Step 1: Add daemon and mcp-server dependencies to CLI**

Add to `crates/cli/Cargo.toml`:
```toml
codeindex-daemon = { path = "../daemon" }
codeindex-mcp-server = { path = "../mcp-server" }
```

Note: the mcp-server crate needs a `lib.rs` exposing its tools module. Create `crates/mcp-server/src/lib.rs`:
```rust
pub mod tools;
```

- [ ] **Step 2: Implement watch.rs**

```rust
// crates/cli/src/commands/watch.rs
use anyhow::Result;
use codeindex_core::config::Config;
use codeindex_core::indexer::pipeline::IndexPipeline;
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_tree_sitter_langs::LanguageRegistry;
use std::path::PathBuf;
use std::sync::mpsc;

pub fn run(daemon: bool, stop: bool) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);
    let pid_path = index_dir.join("daemon.pid");

    if stop {
        return stop_daemon(&pid_path);
    }

    if !index_dir.exists() {
        anyhow::bail!("No index found. Run `codeindex init` first.");
    }

    if daemon {
        // Fork to background (simplified: just note the PID)
        let pid = std::process::id();
        std::fs::write(&pid_path, pid.to_string())?;
        println!("Daemon started with PID {} (PID file: {})", pid, pid_path.display());
    }

    println!("Watching for file changes in {} ...", project_root.display());

    let (tx, rx) = mpsc::channel::<PathBuf>();
    let registry = LanguageRegistry::new();
    let extensions: Vec<String> = registry.all_extensions();
    let ext_refs: Vec<&str> = extensions.iter().map(|s| s.as_str()).collect();

    let _watcher = codeindex_daemon::watcher::start_watcher(&project_root, tx, &ext_refs)?;
    let debouncer = codeindex_daemon::debounce::Debouncer::new(rx, config.daemon.debounce_ms);

    loop {
        match debouncer.next_batch() {
            Some(changed) => {
                println!("Re-indexing {} file(s)...", changed.len());
                let db_path = index_dir.join("index.db");
                let storage = SqliteStorage::open(&db_path)?;
                let registry = LanguageRegistry::new();
                let mut pipeline = IndexPipeline::new(storage, registry);
                for path in &changed {
                    match pipeline.index_file(path, &project_root) {
                        Ok(n) => println!("  {} — {} regions", path.display(), n),
                        Err(e) => eprintln!("  {} — error: {}", path.display(), e),
                    }
                }
            }
            None => break,
        }
    }

    Ok(())
}

fn stop_daemon(pid_path: &std::path::Path) -> Result<()> {
    if !pid_path.exists() {
        anyhow::bail!("No daemon PID file found.");
    }
    let pid: i32 = std::fs::read_to_string(pid_path)?.trim().parse()?;
    unsafe { libc::kill(pid, libc::SIGTERM); }
    std::fs::remove_file(pid_path)?;
    println!("Sent SIGTERM to daemon (PID {})", pid);
    Ok(())
}
```

Add `libc = "0.2"` to `crates/cli/Cargo.toml`.

- [ ] **Step 3: Implement mcp.rs**

```rust
// crates/cli/src/commands/mcp.rs
use anyhow::Result;

pub fn run() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let server = codeindex_mcp_server::tools::McpServer::new(&project_root)?;

    use std::io::{self, BufRead, Write};

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err = serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":e.to_string()}});
                writeln!(stdout, "{}", err)?;
                stdout.flush()?;
                continue;
            }
        };

        // Delegate to the MCP server's handler
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let response = match method {
            "initialize" => server.handle_initialize_value(&request),
            "tools/list" => server.handle_tools_list_value(&request),
            "tools/call" => server.handle_tools_call_value(&request),
            _ => serde_json::json!({"jsonrpc":"2.0","id":request.get("id"),"error":{"code":-32601,"message":"Method not found"}}),
        };

        writeln!(stdout, "{}", response)?;
        stdout.flush()?;
    }

    Ok(())
}
```

Note: the MCP server's `McpServer` needs methods that take/return `serde_json::Value` rather than the internal `JsonRpcRequest`/`JsonRpcResponse` types. Add thin wrappers in `tools.rs` or make the types public. The simplest approach: add `handle_*_value` methods that accept/return `serde_json::Value`.

- [ ] **Step 4: Wire into main.rs**

Add `Watch` to Commands enum:
```rust
    /// Watch for file changes and re-index
    Watch {
        /// Run as background daemon
        #[arg(long)]
        daemon: bool,
        /// Stop running daemon
        #[arg(long)]
        stop: bool,
    },
```

Add match arms:
```rust
        Commands::Watch { daemon, stop } => commands::watch::run(daemon, stop),
        Commands::McpServer => commands::mcp::run(),
```

Update `commands/mod.rs`:
```rust
pub mod watch;
pub mod mcp;
```

- [ ] **Step 5: Build**

Run: `cd codeindex && cargo build -p codeindex`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: implement watch and mcp-server CLI commands"
```

---

### Task 9: Publishing Infrastructure

**Files:**
- Modify: `codeindex/Cargo.toml`
- Create: `codeindex/LICENSE`
- Create: `codeindex/.github/workflows/ci.yml`
- Create: `codeindex/.github/workflows/release.yml`

- [ ] **Step 1: Update workspace Cargo.toml metadata**

```toml
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
authors = ["Jonathan James"]
repository = "https://github.com/rigbox-dev/codeindex"
homepage = "https://github.com/rigbox-dev/codeindex"
keywords = ["code-search", "indexing", "tree-sitter", "mcp", "ai-tools"]
categories = ["development-tools", "command-line-utilities"]
description = "Semantic code indexing and retrieval for AI agents"
```

Add `publish = false` to each non-CLI crate's `Cargo.toml` (core, tree-sitter-langs, daemon, mcp-server):
```toml
publish = false
```

Add `description` to the CLI crate:
```toml
description = "Semantic code indexing and retrieval for AI agents"
```

- [ ] **Step 2: Create LICENSE**

```
MIT License

Copyright (c) 2026 Jonathan James

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

- [ ] **Step 3: Create CI workflow**

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - name: Build
        run: cargo build --workspace
      - name: Test
        run: cargo test --workspace
      - name: Clippy
        run: cargo clippy --workspace -- -D warnings
```

- [ ] **Step 4: Create release workflow**

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

permissions:
  contents: write

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: Swatinem/rust-cache@v2
      - name: Build
        run: cargo build --release --target ${{ matrix.target }} -p codeindex
      - name: Package
        run: |
          mkdir -p dist
          cp target/${{ matrix.target }}/release/codeindex dist/
          cd dist && tar czf codeindex-${{ matrix.target }}.tar.gz codeindex
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: codeindex-${{ matrix.target }}
          path: dist/codeindex-${{ matrix.target }}.tar.gz

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          path: artifacts
      - name: Create release
        uses: softprops/action-gh-release@v2
        with:
          files: artifacts/**/*.tar.gz
          generate_release_notes: true
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: add publishing infrastructure (LICENSE, CI, release workflows)"
```

---

### Task 10: Final Verification

**Files:** none (verification only)

- [ ] **Step 1: Run full test suite**

Run: `cd codeindex && cargo test --workspace`
Expected: all tests pass

- [ ] **Step 2: Run clippy**

Run: `cd codeindex && cargo clippy --workspace -- -D warnings`
Expected: no warnings

- [ ] **Step 3: Test on mac-cpp**

```bash
cd /Users/jonathanjames/workspace/mac-cpp
codeindex init && codeindex index
codeindex query ":symbol Scanner"
codeindex stats
```

Expected: C/C++ files indexed, symbol lookup returns results.

- [ ] **Step 4: Clean up test project**

```bash
rm -rf /Users/jonathanjames/workspace/mac-cpp/.codeindex
```

- [ ] **Step 5: Push and verify CI**

```bash
git push origin main
```

- [ ] **Step 6: Commit any fixes**

```bash
git add -A && git commit -m "chore: final verification and fixes"
```
