use anyhow::Result;
use codeindex_core::config::Config;
use codeindex_core::indexer::pipeline::IndexPipeline;
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_tree_sitter_langs::LanguageRegistry;
use std::path::PathBuf;
use std::sync::mpsc;

pub fn run(stop: bool) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);
    let pid_path = index_dir.join("daemon.pid");

    if stop {
        if !pid_path.exists() {
            anyhow::bail!("No daemon PID file found.");
        }
        let pid: i32 = std::fs::read_to_string(&pid_path)?.trim().parse()?;
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        std::fs::remove_file(&pid_path)?;
        println!("Sent SIGTERM to daemon (PID {})", pid);
        return Ok(());
    }

    if !index_dir.exists() {
        anyhow::bail!("No index found. Run `codeindex init` first.");
    }

    println!(
        "Watching for file changes in {} ...",
        project_root.display()
    );

    let (tx, rx) = mpsc::channel::<PathBuf>();
    let registry = LanguageRegistry::new();
    let extensions = registry.all_extensions();

    let _watcher =
        codeindex_daemon::watcher::start_watcher(&project_root, tx, extensions)?;
    let debouncer = codeindex_daemon::debounce::Debouncer::new(rx, config.daemon.debounce_ms);

    while let Some(changed) = debouncer.next_batch() {
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
    Ok(())
}
