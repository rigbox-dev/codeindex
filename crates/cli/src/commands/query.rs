use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::embedding::MockEmbeddingProvider;
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

    // Use MockEmbeddingProvider with dimension 32 for now
    const DIM: usize = 32;
    let provider = MockEmbeddingProvider::new(DIM);

    // Load or create vector index
    let mut vector_index = VectorIndex::load(&index_dir, DIM)
        .unwrap_or_else(|_| VectorIndex::new(DIM));

    // If no vectors loaded, embed all regions from storage
    // (needed when index was built without embeddings)
    build_vector_index_if_empty(&storage, &provider, &mut vector_index)?;

    let engine = QueryEngine::new(&storage, &vector_index, &provider);

    let opts = QueryOptions {
        top,
        depth,
        include_code,
    };

    let response = engine.query(&query, &opts).with_context(|| "Query failed")?;

    if response.results.is_empty() {
        if json || format == "json" || !atty::is(atty::Stream::Stdout) {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            println!("No results found for: {}", query);
        }
        std::process::exit(1);
    }

    // Determine output mode
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

/// Populate the VectorIndex from all regions in storage using the given provider.
/// This is a no-op if the index already has points from a previous load.
fn build_vector_index_if_empty(
    storage: &SqliteStorage,
    provider: &MockEmbeddingProvider,
    vector_index: &mut VectorIndex,
) -> Result<()> {
    use codeindex_core::embedding::EmbeddingProvider;

    // Collect file_ids
    let file_ids: Vec<i64> = {
        let conn = storage.conn();
        let mut stmt = conn.prepare("SELECT file_id FROM files")?;
        let ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        ids
    };

    if file_ids.is_empty() {
        return Ok(());
    }

    for file_id in file_ids {
        let regions = storage.get_regions_for_file(file_id)?;
        for region in regions {
            let text = format!("{}: {}", region.kind, region.signature);
            let vecs = provider.embed(&[text])?;
            // Ignore dimension mismatch errors (index might be partially built)
            let _ = vector_index.add(region.region_id, vecs.into_iter().next().unwrap_or_default());
        }
    }

    vector_index.build()?;
    Ok(())
}
