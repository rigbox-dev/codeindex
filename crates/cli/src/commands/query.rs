use anyhow::{Context, Result};
use codeindex_core::config::Config;
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

    // Build the embedding provider using the same logic as the index command.
    let provider = super::index::build_embedding_provider(&config);
    let dim = provider.dimension();

    // Load the vector index built during indexing.
    let vector_index = VectorIndex::load(&index_dir, dim)
        .unwrap_or_else(|_| VectorIndex::new(dim));

    let engine = QueryEngine::new(&storage, &vector_index, provider.as_ref(), &project_root);

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
