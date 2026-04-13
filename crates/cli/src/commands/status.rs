use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::storage::sqlite::SqliteStorage;

pub fn run() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);

    if !index_dir.exists() {
        println!("No index found. Run `codeindex init` to initialize.");
        std::process::exit(2);
    }

    let db_path = index_dir.join("index.db");
    if !db_path.exists() {
        println!("Index directory exists but no database found. Run `codeindex index` to build.");
        std::process::exit(2);
    }

    let storage = SqliteStorage::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

    let conn = storage.conn();

    let file_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .unwrap_or(0);

    let region_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM regions", [], |row| row.get(0))
        .unwrap_or(0);

    let dep_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM dependencies", [], |row| row.get(0))
        .unwrap_or(0);

    println!("Index path:        {}", index_dir.display());
    println!("Files indexed:     {}", file_count);
    println!("Regions indexed:   {}", region_count);
    println!("Dependencies:      {}", dep_count);
    println!("Embedding provider: {}", config.embedding.provider);
    println!(
        "Summaries enabled: {}",
        if config.summary.enabled { "yes" } else { "no" }
    );

    Ok(())
}
