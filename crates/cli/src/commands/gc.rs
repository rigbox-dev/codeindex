use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::storage::sqlite::SqliteStorage;

pub fn run() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let db_path = project_root.join(&config.index.path).join("index.db");
    if !db_path.exists() {
        anyhow::bail!("No index found. Run `codeindex init && codeindex index` first.");
    }
    let storage = SqliteStorage::open(&db_path).with_context(|| "Failed to open database")?;
    let files = storage.list_all_files()?;
    let mut removed = 0;
    for file in &files {
        if !project_root.join(&file.path).exists() {
            storage.delete_file(file.file_id)?;
            removed += 1;
        }
    }
    storage.vacuum()?;
    let size = std::fs::metadata(&db_path)
        .map(|m| m.len())
        .unwrap_or(0);
    println!(
        "Removed {} orphaned file(s). Index size: {:.1} KB",
        removed,
        size as f64 / 1024.0
    );
    Ok(())
}
