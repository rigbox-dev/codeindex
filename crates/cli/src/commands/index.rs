use anyhow::{Context, Result};
use codeindex_core::config::Config;
use codeindex_core::embedding::MockEmbeddingProvider;
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

    // Build an embedding provider based on config.
    let provider = build_embedding_provider(&config);

    let mut pipeline = IndexPipeline::new(storage, registry)
        .with_embedding_provider(provider);

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

/// Build a boxed `EmbeddingProvider` according to the config.
///
/// Priority:
/// 1. If `embedding.provider` == "voyage", try `VoyageEmbeddingProvider::from_env()`.
/// 2. Try `LocalEmbeddingProvider` via `ensure_model()` + `from_dir()`.
/// 3. Fall back to `MockEmbeddingProvider::new(384)`.
pub(crate) fn build_embedding_provider(
    config: &codeindex_core::config::Config,
) -> Box<dyn codeindex_core::embedding::EmbeddingProvider> {
    use codeindex_core::embedding::{
        ensure_model,
        local::LocalEmbeddingProvider,
        voyage::VoyageEmbeddingProvider,
    };

    // 1. Voyage (only if explicitly configured)
    if config.embedding.provider == "voyage" {
        let model = config
            .embedding
            .model
            .clone()
            .unwrap_or_else(|| "voyage-code-3".to_string());
        match VoyageEmbeddingProvider::from_env(model) {
            Ok(p) => {
                eprintln!("Using Voyage embedding provider.");
                return Box::new(p);
            }
            Err(e) => {
                eprintln!("Voyage provider unavailable ({}), falling back to local.", e);
            }
        }
    }

    // 2. Local ONNX model
    match ensure_model().and_then(|dir| LocalEmbeddingProvider::from_dir(&dir)) {
        Ok(p) => {
            eprintln!("Using local ONNX embedding provider (all-MiniLM-L6-v2).");
            return Box::new(p);
        }
        Err(e) => {
            eprintln!(
                "Local embedding provider unavailable ({}), falling back to mock.",
                e
            );
        }
    }

    // 3. Mock fallback
    eprintln!("Using mock embedding provider (deterministic, not semantic).");
    Box::new(MockEmbeddingProvider::new(384))
}
