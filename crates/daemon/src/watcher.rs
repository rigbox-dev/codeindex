use anyhow::Result;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use notify::{recommended_watcher, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tracing::{debug, warn};

/// Start a filesystem watcher on `project_root`.
///
/// Events for created, modified, or removed files are filtered by:
/// 1. Must not be a directory.
/// 2. Must not be gitignored (if a `.gitignore` exists at `project_root`).
/// 3. Must have an extension present in `supported_extensions` (e.g. `[".rs"]`).
///
/// Matching paths are forwarded to `tx`.
pub fn start_watcher(
    project_root: &Path,
    tx: mpsc::Sender<PathBuf>,
    supported_extensions: Vec<String>,
) -> Result<RecommendedWatcher> {
    // Build a gitignore matcher for the project root.
    let gitignore = build_gitignore(project_root);

    let project_root = project_root.to_path_buf();

    let mut watcher = recommended_watcher(move |result: notify::Result<Event>| {
        match result {
            Ok(event) => {
                let is_relevant = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                );
                if !is_relevant {
                    return;
                }

                for path in event.paths {
                    // Skip directories.
                    if path.is_dir() {
                        continue;
                    }

                    // Skip gitignored paths.
                    if let Some(ref gi) = gitignore {
                        let is_dir = false;
                        if gi.matched(&path, is_dir).is_ignore() {
                            debug!("gitignored, skipping: {:?}", path);
                            continue;
                        }
                    }

                    // Check extension.
                    let ext = match path.extension() {
                        Some(e) => format!(".{}", e.to_string_lossy()),
                        None => continue,
                    };
                    if !supported_extensions.contains(&ext) {
                        continue;
                    }

                    debug!("watcher: sending {:?}", path);
                    if let Err(e) = tx.send(path) {
                        warn!("watcher: channel send failed: {}", e);
                    }
                }
            }
            Err(e) => {
                warn!("watcher error: {:?}", e);
            }
        }
    })?;

    watcher.watch(&project_root, RecursiveMode::Recursive)?;
    Ok(watcher)
}

fn build_gitignore(project_root: &Path) -> Option<Gitignore> {
    let gitignore_path = project_root.join(".gitignore");
    if !gitignore_path.exists() {
        return None;
    }

    let mut builder = GitignoreBuilder::new(project_root);
    if let Some(e) = builder.add(gitignore_path) {
        warn!("failed to load .gitignore: {}", e);
        return None;
    }

    match builder.build() {
        Ok(gi) => Some(gi),
        Err(e) => {
            warn!("failed to build gitignore matcher: {}", e);
            None
        }
    }
}
