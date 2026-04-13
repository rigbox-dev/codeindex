use anyhow::Result;
use std::path::PathBuf;
use std::sync::mpsc;
use tracing::{error, info, warn};

use codeindex_core::{
    config::Config,
    indexer::pipeline::IndexPipeline,
    storage::sqlite::SqliteStorage,
};
use codeindex_daemon::debounce::Debouncer;
use codeindex_daemon::watcher::start_watcher;
use codeindex_tree_sitter_langs::LanguageRegistry;

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    // Determine the project root (current working directory).
    let project_root = std::env::current_dir()?;
    info!("codeindex-daemon starting in {:?}", project_root);

    // Load configuration.
    let config = Config::load(&project_root)?;
    info!("daemon debounce: {}ms", config.daemon.debounce_ms);

    // Collect supported extensions from all registered language plugins.
    let registry = LanguageRegistry::new();
    let supported_extensions: Vec<String> = registry.all_extensions();
    info!("watching extensions: {:?}", supported_extensions);

    // Create the mpsc channel used to forward paths from watcher → debouncer.
    let (tx, rx) = mpsc::channel::<PathBuf>();

    // Start the filesystem watcher (kept alive for the duration of the loop).
    let _watcher = start_watcher(&project_root, tx, supported_extensions.clone())?;
    info!("filesystem watcher started");

    // Build the debouncer.
    let debouncer = Debouncer::new(rx, config.daemon.debounce_ms);

    // Determine the SQLite database path.
    let db_path = config.index_dir(&project_root).join("index.db");

    info!("watching for file changes — press Ctrl-C to stop");

    loop {
        let batch = match debouncer.next_batch() {
            Some(b) => b,
            None => {
                info!("watcher channel closed, shutting down");
                break;
            }
        };

        info!("processing batch of {} changed file(s)", batch.len());

        // Open storage and build pipeline for this batch.
        let storage = match SqliteStorage::open(&db_path) {
            Ok(s) => s,
            Err(e) => {
                error!("failed to open storage at {:?}: {}", db_path, e);
                continue;
            }
        };
        let registry = LanguageRegistry::new();
        let mut pipeline = IndexPipeline::new(storage, registry);

        for path in &batch {
            // Skip files that no longer exist (e.g. deletions).
            if !path.exists() {
                info!("skipping deleted/moved path: {:?}", path);
                continue;
            }

            match pipeline.index_file(path, &project_root) {
                Ok(region_count) => {
                    info!("indexed {:?} ({} region(s))", path, region_count);
                }
                Err(e) => {
                    warn!("failed to index {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(())
}
