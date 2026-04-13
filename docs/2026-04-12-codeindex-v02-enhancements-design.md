# codeindex v0.2 Enhancements

**Date**: 2026-04-12
**Status**: Design approved, pending implementation
**Builds on**: `docs/superpowers/specs/2026-04-12-codeindex-design.md`

## Overview

Five independent enhancements to make codeindex production-ready: real semantic search via a bundled embedding model, source code in query results, complete CLI commands, C/C++ language support, and publishing infrastructure.

---

## Enhancement 1: Local Embedding Model

### Problem

The indexing pipeline doesn't call any embedding provider — embeddings are created lazily at query time using a `MockEmbeddingProvider` that generates random vectors from blake3 hashes. Natural language queries return low-relevance results.

### Solution

Bundle `all-MiniLM-L6-v2` (22MB ONNX, 384-dimensional output) as the default local embedding model. Download it at first run to `~/.cache/codeindex/models/`. Integrate embeddings into the indexing pipeline so vectors are pre-built.

### Changes

**Model management** (`crates/core/src/embedding/local.rs`):
- `download_model()` — fetches `all-MiniLM-L6-v2.onnx` and `tokenizer.json` from HuggingFace to `~/.cache/codeindex/models/` if not already present. Uses `reqwest::blocking`.
- `ensure_model() -> Result<PathBuf>` — returns path to model, downloading if needed.

**LocalEmbeddingProvider rewrite** (`crates/core/src/embedding/local.rs`):
- Use `tokenizers` crate (HuggingFace tokenizer library) to properly tokenize input text with the model's vocabulary, instead of the placeholder byte-level tokenizer.
- Load ONNX model with `ort::Session`, run inference, mean-pool the output over the sequence dimension, L2-normalize.
- Dimension: 384 (from model output shape).

**Indexing pipeline** (`crates/core/src/indexer/pipeline.rs`):
- `IndexPipeline::new()` gains an `Option<Box<dyn EmbeddingProvider>>` parameter.
- `index_file()` after inserting regions: if embedding provider is set, compose embedding text for each region, call `embed()`, store vectors in the HNSW index.
- `index_project()` calls `vectors.build()` at the end and saves the vector index to disk.
- `IndexPipeline` gains a `VectorIndex` field alongside storage.

**CLI index command** (`crates/cli/src/commands/index.rs`):
- Creates `LocalEmbeddingProvider` (triggering download if needed) and passes it to the pipeline.
- If `local-embeddings` feature is not enabled, falls back to `MockEmbeddingProvider` (same as today).

**Config defaults**:
- `embedding.provider` stays "local" but now means the real ONNX model.
- Remove the hardcoded `dim = 32` in the CLI query command — read dimension from the provider.

**Voyage wiring** (`crates/cli/src/commands/index.rs`, `query.rs`):
- If `embedding.provider` is "voyage", create `VoyageEmbeddingProvider` from env.
- If `VOYAGE_API_KEY` is not set, warn and fall back to local.

### Dependencies

Add to `crates/core/Cargo.toml`:
```toml
tokenizers = { version = "0.20", default-features = false, features = ["onig"] }
```

The `ort` and `reqwest` dependencies already exist.

Remove the `local-embeddings` feature flag — `ort` and `tokenizers` become required dependencies. The model binary is downloaded on demand (not compiled in), so binary size impact is only the ONNX runtime.

---

## Enhancement 2: Source Code in Query Results

### Problem

The `code` field in `QueryResult` is always `None`. Agents consuming query results must make separate file reads to see the actual code.

### Solution

When `include_code` is true, read the source file and extract the region's byte range.

### Changes

**`query/mod.rs` — `build_results`**:
- Add `project_root: &Path` parameter.
- Maintain a `HashMap<String, Vec<u8>>` file cache within the function — multiple regions from the same file share one read.
- When `include_code` is true: read the file (or use cache), slice `source[start_byte as usize..end_byte as usize]`, convert to `String`, set as the `code` field.
- When `include_code` is false: leave as `None`.

**Thread `project_root` through**:
- `QueryEngine::new()` takes `project_root: &Path` and stores it.
- CLI `query` command passes `std::env::current_dir()`.
- MCP server passes its `project_root`.

---

## Enhancement 3: Finish CLI Stubs

### 3a: `config` command

**`codeindex config`** — prints current merged config as pretty JSON.

**`codeindex config set <key> <value>`** — parses dotted key paths (e.g. `embedding.provider`), updates the config, saves to `.codeindex/config.json`.

Implementation: load config, match on key path to set the appropriate field, save. Limited set of known keys — error on unknown keys.

### 3b: `gc` command

**`codeindex gc`** — garbage collect the index:
1. Walk all indexed file paths, check if file still exists on disk.
2. Delete regions/dependencies/FTS entries for files that no longer exist.
3. Run `VACUUM` on SQLite to reclaim space.
4. Report: "Removed N orphaned files, reclaimed Xkb".

Add a `SqliteStorage::delete_file(file_id)` method that cascades to regions/dependencies.
Add a `SqliteStorage::list_all_files() -> Vec<IndexedFile>` method.

### 3c: `stats` command

**`codeindex stats`** — detailed breakdown:
- Files by language (e.g. "go: 475, rust: 12")
- Regions by kind (e.g. "function: 3200, method: 1800, struct: 400")
- Dependencies by kind (e.g. "calls: 1500, imports: 400")
- Index size on disk (size of `index.db` + `vectors.bin`)
- Last indexed timestamp (most recent `last_indexed_at` across files)

All data from SQL aggregate queries.

### 3d: `watch` command

**`codeindex watch`** — runs the file watcher in-process (not delegating to the daemon binary). Imports `codeindex-daemon`'s `watcher` and `debounce` modules directly.

- `--daemon` — fork to background, write PID to `.codeindex/daemon.pid`.
- `--stop` — read PID file, send SIGTERM, clean up PID file.
- Default (no flags) — foreground, logs to stdout.

Add `codeindex-daemon` as a dependency of the CLI crate.

### 3e: `mcp-server` command

**`codeindex mcp-server`** — runs the MCP server in-process. Import `codeindex-mcp-server`'s tools module and run the stdio loop directly rather than exec'ing a separate binary.

Add `codeindex-mcp-server` as a dependency of the CLI crate (or extract the shared logic into core).

---

## Enhancement 4: C/C++ Language Plugin

### Two plugins sharing helpers

**`CPlugin`** — language_id "c", extensions [".c", ".h"]
**`CppPlugin`** — language_id "cpp", extensions [".cpp", ".hpp", ".cc", ".cxx", ".hh"]

Both use shared helper functions for common extraction logic.

### Region extraction

| RegionKind | C nodes | C++ additional nodes |
|---|---|---|
| Function | `function_definition` | same |
| Struct | `struct_specifier` (with body) | same |
| Enum | `enum_specifier` | same |
| Class | N/A | `class_specifier` |
| Method | N/A | `function_definition` inside class body |
| Module | N/A | `namespace_definition` |

### Dependency extraction

- `preproc_include` → Imports (extract the path from `#include "..."` or `#include <...>`)
- `call_expression` → Calls (same pattern as Rust/Go — look at `function` child)

### Tree-sitter grammars

Add to `crates/tree-sitter-langs/Cargo.toml`:
```toml
tree-sitter-c = "0.23"
tree-sitter-cpp = "0.23"
```

### Test fixtures

Create `tests/fixtures/cpp-project/` with a few `.cpp`/`.h` files mirroring the auth handler pattern used in other fixture projects. Can also verify against the real `mac-cpp` project.

---

## Enhancement 5: Publishing

### crates.io

Update workspace `Cargo.toml` metadata:
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
```

Set `publish = false` on internal crates (core, tree-sitter-langs, daemon, mcp-server). Only the `cli` crate publishes as `codeindex`.

Add a `LICENSE` file (MIT).

### GitHub Actions CI

`.github/workflows/ci.yml`:
- Trigger: push to main, PRs
- Matrix: macOS (latest), Ubuntu (latest)
- Steps: checkout, setup Rust (stable), `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`

### GitHub Actions Release

`.github/workflows/release.yml`:
- Trigger: push tag `v*`
- Matrix build: macOS arm64, macOS x86_64, Linux x86_64
- Steps: checkout, setup Rust, `cargo build --release -p codeindex`, package binary as `.tar.gz`, create GitHub release, upload artifacts
- Use `cross` for Linux cross-compilation if building on macOS runners.

### Homebrew

Create `rigbox-dev/homebrew-tap` repo (or add to existing tap) with:
- `Formula/codeindex.rb` — downloads the release binary for the current platform, installs to bin.
- Updated automatically by the release workflow via `dawidd6/action-homebrew-bump-formula` or a simple sed script that updates the URL and SHA in the formula.

---

## Testing

Each enhancement has its own tests:

1. **Embedding**: integration test that indexes a fixture with real embeddings, queries "authentication", verifies results are semantically relevant (not random). Mock tests for download logic.
2. **Source code**: unit test that `build_results` with `include_code: true` returns non-None `code` fields matching expected source.
3. **CLI stubs**: `config set` round-trip test, `gc` test with deleted fixture file, `stats` output parsing.
4. **C/C++ plugin**: snapshot tests like other languages — extract regions from fixture, verify kinds/names/line numbers.
5. **Publishing**: CI workflow runs tests (validates itself). No code tests needed.
