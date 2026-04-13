use anyhow::Result;
use codeindex_core::config::Config;

pub fn run() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::default();
    let index_dir = project_root.join(&config.index.path);
    if index_dir.exists() {
        println!("Index already initialized at {}", index_dir.display());
        return Ok(());
    }
    std::fs::create_dir_all(&index_dir)?;
    config.save(&project_root)?;
    println!("Initialized codeindex at {}", index_dir.display());
    println!("Run `codeindex index` to index the project.");
    Ok(())
}
