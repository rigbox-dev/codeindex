use std::path::PathBuf;

use codeindex_core::embedding::{EmbeddingProvider, MockEmbeddingProvider};
use codeindex_core::indexer::pipeline::IndexPipeline;
use codeindex_core::query::{QueryEngine, QueryOptions};
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_core::storage::vectors::VectorIndex;
use codeindex_tree_sitter_langs::LanguageRegistry;

/// Returns the path to a named fixture directory, relative to the workspace root.
fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

/// Index a project at the given path and return (storage, stats).
fn index_project(
    project_root: &PathBuf,
) -> (
    SqliteStorage,
    codeindex_core::indexer::pipeline::IndexStats,
) {
    let storage = SqliteStorage::open_in_memory().unwrap();
    let registry = LanguageRegistry::new();
    let mut pipeline = IndexPipeline::new(storage, registry);
    let stats = pipeline.index_project(project_root).unwrap();
    let storage = pipeline.into_storage();
    (storage, stats)
}

/// Build a VectorIndex by embedding all regions in the storage using MockEmbeddingProvider(dim).
fn build_vector_index(storage: &SqliteStorage, dim: usize) -> VectorIndex {
    let provider = MockEmbeddingProvider::new(dim);
    let mut vector_index = VectorIndex::new(dim);

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
            vector_index
                .add(region.region_id, vecs.into_iter().next().unwrap())
                .unwrap();
        }
    }

    vector_index.build().unwrap();
    vector_index
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_rust_project() {
    let root = fixture_path("rust-project");
    let (storage, stats) = index_project(&root);

    assert!(
        stats.files_indexed >= 4,
        "expected >= 4 files indexed, got {}",
        stats.files_indexed
    );
    assert!(
        stats.regions_extracted > 5,
        "expected > 5 regions, got {}",
        stats.regions_extracted
    );

    const DIM: usize = 32;
    let provider = MockEmbeddingProvider::new(DIM);
    let vector_index = build_vector_index(&storage, DIM);

    let engine = QueryEngine::new(&storage, &vector_index, &provider, &root);
    let opts = QueryOptions {
        top: 10,
        depth: 1,
        include_code: false,
    };

    // Test :symbol AuthRequest
    let response = engine.query(":symbol AuthRequest", &opts).unwrap();
    assert!(
        !response.results.is_empty(),
        ":symbol AuthRequest should return at least one result"
    );
    assert_eq!(
        response.results[0].name, "AuthRequest",
        "first result name should be 'AuthRequest', got: {:?}",
        response.results.iter().map(|r| &r.name).collect::<Vec<_>>()
    );

    // Test :file src/auth/types.rs
    let file_opts = QueryOptions {
        top: 50,
        depth: 0,
        include_code: false,
    };
    let response = engine.query(":file src/auth/types.rs", &file_opts).unwrap();
    assert!(
        response.results.len() >= 4,
        ":file src/auth/types.rs should return >= 4 regions, got {}",
        response.results.len()
    );

    // Test natural language query: "authenticate"
    let nl_response = engine.query("authenticate", &opts).unwrap();
    assert!(
        !nl_response.results.is_empty(),
        "natural language query 'authenticate' should return non-empty results"
    );
}

#[test]
fn end_to_end_typescript_project() {
    let root = fixture_path("typescript-project");
    let (_, stats) = index_project(&root);

    assert!(
        stats.files_indexed >= 3,
        "expected >= 3 TypeScript files indexed, got {}",
        stats.files_indexed
    );
    assert!(
        stats.regions_extracted > 3,
        "expected > 3 regions, got {}",
        stats.regions_extracted
    );
}

#[test]
fn end_to_end_python_project() {
    let root = fixture_path("python-project");
    let (_, stats) = index_project(&root);

    assert!(
        stats.files_indexed >= 2,
        "expected >= 2 Python files indexed, got {}",
        stats.files_indexed
    );
    assert!(
        stats.regions_extracted > 2,
        "expected > 2 regions, got {}",
        stats.regions_extracted
    );
}

#[test]
fn incremental_indexing_skips_unchanged() {
    let root = fixture_path("rust-project");

    let storage = SqliteStorage::open_in_memory().unwrap();
    let registry = LanguageRegistry::new();
    let mut pipeline = IndexPipeline::new(storage, registry);

    // First indexing pass.
    let stats1 = pipeline.index_project(&root).unwrap();

    // Second indexing pass (same files, no changes).
    let stats2 = pipeline.index_project(&root).unwrap();

    assert_eq!(
        stats1.files_indexed, stats2.files_indexed,
        "second pass should index the same number of files: first={}, second={}",
        stats1.files_indexed, stats2.files_indexed
    );
    assert_eq!(
        stats1.regions_extracted, stats2.regions_extracted,
        "second pass should return the same region count: first={}, second={}",
        stats1.regions_extracted, stats2.regions_extracted
    );
}
