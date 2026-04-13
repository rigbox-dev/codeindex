use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::indexer::pipeline::IndexPipeline;
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_tree_sitter_langs::LanguageRegistry;

pub fn run(incremental: bool, quiet: bool) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);

    if !index_dir.exists() {
        anyhow::bail!(
            "Index directory '{}' does not exist. Run `codeindex init` first.",
            index_dir.display()
        );
    }

    let db_path = index_dir.join("index.db");
    let storage = SqliteStorage::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

    let registry = LanguageRegistry::new();
    let mut pipeline = IndexPipeline::new(storage, registry);

    if !quiet {
        println!("Indexing project at {} ...", project_root.display());
        if incremental {
            println!("(incremental mode — skipping unchanged files)");
        }
    }

    let stats = pipeline
        .index_project(&project_root)
        .with_context(|| "Indexing failed")?;

    if !quiet {
        println!(
            "Done. Indexed {} file(s), {} region(s), {} dependency(s).",
            stats.files_indexed, stats.regions_extracted, stats.dependencies_extracted
        );
    }

    Ok(())
}
