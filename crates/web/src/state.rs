use std::path::PathBuf;
use std::sync::Arc;

pub type SharedState = Arc<AppState>;

pub struct AppState {
    pub project_root: PathBuf,
    pub db_path: PathBuf,
    pub index_dir: PathBuf,
}

impl AppState {
    pub fn new(project_root: PathBuf) -> anyhow::Result<Self> {
        let config = codeindex_core::config::Config::load(&project_root).unwrap_or_default();
        let index_dir = project_root.join(&config.index.path);
        let db_path = index_dir.join("index.db");
        Ok(Self { project_root, db_path, index_dir })
    }
}
