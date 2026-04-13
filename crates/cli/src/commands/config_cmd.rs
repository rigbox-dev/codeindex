use anyhow::Result;
use codeindex_core::config::Config;
use codeindex_core::storage::sqlite::SqliteStorage;

pub fn run_show() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    println!("{}", serde_json::to_string_pretty(&config)?);
    Ok(())
}

pub fn run_set(key: &str, value: &str) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let mut config = Config::load(&project_root).unwrap_or_default();
    match key {
        "embedding.provider" => config.embedding.provider = value.to_string(),
        "embedding.model" => config.embedding.model = Some(value.to_string()),
        "summary.enabled" => {
            config.summary.enabled = value
                .parse()
                .map_err(|_| anyhow::anyhow!("expected true or false"))?
        }
        "summary.model" => config.summary.model = value.to_string(),
        "query.default_top" => config.query.default_top = value.parse()?,
        "query.default_depth" => config.query.default_depth = value.parse()?,
        "daemon.debounce_ms" => config.daemon.debounce_ms = value.parse()?,
        "index.git_hooks" => {
            config.index.git_hooks = value
                .parse()
                .map_err(|_| anyhow::anyhow!("expected true or false"))?
        }
        _ => anyhow::bail!("Unknown config key: {}", key),
    }
    config.save(&project_root)?;
    // Log the config change to the activity log if a database exists.
    let index_dir = project_root.join(&config.index.path);
    if let Ok(storage) = SqliteStorage::open(&index_dir.join("index.db")) {
        let _ = storage.insert_activity(
            "config_change",
            &format!(r#"{{"key":"{}","value":"{}"}}"#, key, value),
            "cli",
        );
    }
    println!("Set {} = {}", key, value);
    Ok(())
}
