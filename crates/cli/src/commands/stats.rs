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
    let storage = SqliteStorage::open(&db_path).with_context(|| "Failed to open database")?;
    let conn = storage.conn();

    println!("=== Files by Language ===");
    let mut stmt =
        conn.prepare("SELECT language, COUNT(*) FROM files GROUP BY language ORDER BY COUNT(*) DESC")?;
    let rows =
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (lang, count) = row?;
        println!("  {}: {}", lang, count);
    }

    println!("\n=== Regions by Kind ===");
    let mut stmt =
        conn.prepare("SELECT kind, COUNT(*) FROM regions GROUP BY kind ORDER BY COUNT(*) DESC")?;
    let rows =
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (kind, count) = row?;
        println!("  {}: {}", kind, count);
    }

    println!("\n=== Dependencies by Kind ===");
    let mut stmt = conn.prepare(
        "SELECT kind, COUNT(*) FROM dependencies GROUP BY kind ORDER BY COUNT(*) DESC",
    )?;
    let rows =
        stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (kind, count) = row?;
        println!("  {}: {}", kind, count);
    }

    let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
    let vec_size = std::fs::metadata(index_dir.join("vectors.json"))
        .map(|m| m.len())
        .unwrap_or(0);
    println!("\n=== Index Size ===");
    println!("  Database: {:.1} KB", db_size as f64 / 1024.0);
    println!("  Vectors:  {:.1} KB", vec_size as f64 / 1024.0);
    println!(
        "  Total:    {:.1} KB",
        (db_size + vec_size) as f64 / 1024.0
    );

    Ok(())
}
