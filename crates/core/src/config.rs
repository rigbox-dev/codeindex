use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn default_languages() -> Vec<String> {
    vec![
        "typescript".to_string(),
        "python".to_string(),
        "rust".to_string(),
        "go".to_string(),
    ]
}

fn default_embedding_provider() -> String {
    "local".to_string()
}

fn default_summary_provider() -> String {
    "anthropic".to_string()
}

fn default_summary_model() -> String {
    "claude-haiku-4-5-20251001".to_string()
}

fn default_summary_api_key_env() -> String {
    "ANTHROPIC_API_KEY".to_string()
}

fn default_summary_batch_size() -> usize {
    20
}

fn default_summary_interval_seconds() -> u64 {
    30
}

fn default_query_default_top() -> usize {
    5
}

fn default_query_default_depth() -> usize {
    1
}

fn default_daemon_debounce_ms() -> u64 {
    500
}

fn default_daemon_max_concurrent_embeds() -> usize {
    4
}

fn default_daemon_respect_gitignore() -> bool {
    true
}

fn default_index_path() -> String {
    ".codeindex".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingConfig {
    #[serde(default = "default_embedding_provider")]
    pub provider: String,

    #[serde(default)]
    pub model: Option<String>,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        serde_json::from_str("{}").expect("EmbeddingConfig default deserialize failed")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SummaryConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_summary_provider")]
    pub provider: String,

    #[serde(default = "default_summary_model")]
    pub model: String,

    #[serde(default = "default_summary_api_key_env")]
    pub api_key_env: String,

    #[serde(default = "default_summary_batch_size")]
    pub batch_size: usize,

    #[serde(default = "default_summary_interval_seconds")]
    pub interval_seconds: u64,
}

impl Default for SummaryConfig {
    fn default() -> Self {
        serde_json::from_str("{}").expect("SummaryConfig default deserialize failed")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryConfig {
    #[serde(default = "default_query_default_top")]
    pub default_top: usize,

    #[serde(default = "default_query_default_depth")]
    pub default_depth: usize,

    #[serde(default)]
    pub enhance: bool,
}

impl Default for QueryConfig {
    fn default() -> Self {
        serde_json::from_str("{}").expect("QueryConfig default deserialize failed")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonConfig {
    #[serde(default = "default_daemon_debounce_ms")]
    pub debounce_ms: u64,

    #[serde(default = "default_daemon_max_concurrent_embeds")]
    pub max_concurrent_embeds: usize,

    #[serde(default = "default_daemon_respect_gitignore")]
    pub respect_gitignore: bool,

    #[serde(default)]
    pub extra_ignore: Vec<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        serde_json::from_str("{}").expect("DaemonConfig default deserialize failed")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexConfig {
    #[serde(default = "default_index_path")]
    pub path: String,

    #[serde(default)]
    pub git_hooks: bool,
}

impl Default for IndexConfig {
    fn default() -> Self {
        serde_json::from_str("{}").expect("IndexConfig default deserialize failed")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default = "default_languages")]
    pub languages: Vec<String>,

    #[serde(default)]
    pub embedding: EmbeddingConfig,

    #[serde(default)]
    pub summary: SummaryConfig,

    #[serde(default)]
    pub query: QueryConfig,

    #[serde(default)]
    pub daemon: DaemonConfig,

    #[serde(default)]
    pub index: IndexConfig,
}

impl Default for Config {
    fn default() -> Self {
        serde_json::from_str("{}").expect("Config default deserialize failed")
    }
}

impl Config {
    /// Load configuration with precedence: global config < project config.
    /// Global config lives at `dirs::config_dir()/codeindex/config.json`.
    /// Project config lives at `<project_root>/<index.path>/config.json`.
    pub fn load(project_root: &Path) -> Result<Self> {
        // Start with defaults
        let mut config = Config::default();

        // Load global config if it exists
        if let Some(config_dir) = dirs::config_dir() {
            let global_config_path = config_dir.join("codeindex").join("config.json");
            if global_config_path.exists() {
                let contents = std::fs::read_to_string(&global_config_path)?;
                let global: Config = serde_json::from_str(&contents)?;
                config = global;
            }
        }

        // Determine index path from current config to find project config
        let index_path = config.index.path.clone();
        let project_config_path = project_root.join(&index_path).join("config.json");

        if project_config_path.exists() {
            let contents = std::fs::read_to_string(&project_config_path)?;
            let project: Config = serde_json::from_str(&contents)?;
            config = project;
        }

        Ok(config)
    }

    /// Save configuration to `<project_root>/<index.path>/config.json`.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let config_dir = project_root.join(&self.index.path);
        std::fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.json");
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, contents)?;
        Ok(())
    }

    /// Returns the absolute path to the index directory given a project root.
    pub fn index_dir(&self, project_root: &Path) -> PathBuf {
        project_root.join(&self.index.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_config_has_correct_values() {
        let config = Config::default();

        // languages
        assert_eq!(
            config.languages,
            vec!["typescript", "python", "rust", "go"]
        );

        // embedding
        assert_eq!(config.embedding.provider, "local");
        assert_eq!(config.embedding.model, None);

        // summary
        assert!(!config.summary.enabled);
        assert_eq!(config.summary.provider, "anthropic");
        assert_eq!(config.summary.model, "claude-haiku-4-5-20251001");
        assert_eq!(config.summary.api_key_env, "ANTHROPIC_API_KEY");
        assert_eq!(config.summary.batch_size, 20);
        assert_eq!(config.summary.interval_seconds, 30);

        // query
        assert_eq!(config.query.default_top, 5);
        assert_eq!(config.query.default_depth, 1);
        assert!(!config.query.enhance);

        // daemon
        assert_eq!(config.daemon.debounce_ms, 500);
        assert_eq!(config.daemon.max_concurrent_embeds, 4);
        assert!(config.daemon.respect_gitignore);
        assert!(config.daemon.extra_ignore.is_empty());

        // index
        assert_eq!(config.index.path, ".codeindex");
        assert!(!config.index.git_hooks);
    }

    #[test]
    fn config_round_trips_through_json() {
        let original = Config::default();
        let json = serde_json::to_string(&original).expect("serialize failed");
        let restored: Config = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(original, restored);
    }

    #[test]
    fn save_and_load() {
        let tmp = TempDir::new().expect("tempdir failed");
        let project_root = tmp.path();

        let config = Config::default();
        config.save(project_root).expect("save failed");

        let loaded = Config::load(project_root).expect("load failed");
        assert_eq!(config, loaded);

        // Verify specific values survive the round-trip
        assert_eq!(loaded.languages, vec!["typescript", "python", "rust", "go"]);
        assert_eq!(loaded.embedding.provider, "local");
        assert_eq!(loaded.index.path, ".codeindex");
    }
}
