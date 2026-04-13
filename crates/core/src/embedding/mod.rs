use anyhow::Result;
use std::path::PathBuf;

use crate::model::Region;

pub mod local;
pub mod voyage;

/// Trait for embedding providers that convert text into dense vectors.
pub trait EmbeddingProvider: Send + Sync {
    /// Embed a batch of texts, returning one vector per input.
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Dimensionality of the vectors returned by this provider.
    fn dimension(&self) -> usize;

    /// Stable identifier for the model (used in storage metadata).
    fn model_id(&self) -> &str;
}

/// Build the text that will be embedded for a given region.
///
/// Format:
/// ```text
/// [{language}] {kind}: {signature}
/// {summary OR first 20 lines of source body}
/// Dependencies: {dep1, dep2, …}
/// ```
pub fn compose_embedding_text(
    language: &str,
    region: &Region,
    summary: Option<&str>,
    source: &str,
    dep_symbols: &[String],
) -> String {
    let mut parts = Vec::new();

    // Header line
    parts.push(format!("[{}] {}: {}", language, region.kind, region.signature));

    // Body: prefer summary, fall back to first 20 source lines
    if let Some(s) = summary {
        if !s.trim().is_empty() {
            parts.push(s.trim().to_string());
        }
    } else {
        let body: String = source
            .lines()
            .take(20)
            .collect::<Vec<_>>()
            .join("\n");
        if !body.trim().is_empty() {
            parts.push(body);
        }
    }

    // Dependencies
    if !dep_symbols.is_empty() {
        parts.push(format!("Dependencies: {}", dep_symbols.join(", ")));
    }

    parts.join("\n")
}

/// A deterministic mock embedding provider suitable for tests.
///
/// Produces vectors whose components are derived from the blake3 hash of
/// the input text, so the same input always yields the same vector.
pub struct MockEmbeddingProvider {
    dimension: usize,
}

impl MockEmbeddingProvider {
    pub fn new(dimension: usize) -> Self {
        assert!(dimension > 0, "dimension must be > 0");
        Self { dimension }
    }

    fn hash_to_vec(text: &str, dimension: usize) -> Vec<f32> {
        // Use the XOF (extended output) feature of blake3 to fill `dimension` floats.
        let mut output = vec![0u8; dimension * 4];
        let mut hasher = blake3::Hasher::new();
        hasher.update(text.as_bytes());
        let mut reader = hasher.finalize_xof();
        reader.fill(&mut output);

        let floats: Vec<f32> = output
            .chunks_exact(4)
            .map(|b| {
                let arr = [b[0], b[1], b[2], b[3]];
                // Map the raw u32 to [-1, 1]
                let u = u32::from_le_bytes(arr) as f32;
                u / (u32::MAX as f32) * 2.0 - 1.0
            })
            .collect();

        // L2-normalise so cosine distance is meaningful
        let norm: f32 = floats.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            floats.iter().map(|x| x / norm).collect()
        } else {
            floats
        }
    }
}

impl EmbeddingProvider for MockEmbeddingProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|t| Self::hash_to_vec(t, self.dimension))
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_id(&self) -> &str {
        "mock-blake3"
    }
}

const MODEL_DIR: &str = "codeindex/models";
const MODEL_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
const TOKENIZER_URL: &str = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";

/// Return the path to the model directory, downloading model files if needed.
pub fn ensure_model() -> Result<PathBuf> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(MODEL_DIR);
    std::fs::create_dir_all(&cache_dir)?;

    let model_path = cache_dir.join("model.onnx");
    let tokenizer_path = cache_dir.join("tokenizer.json");

    if !model_path.exists() {
        eprintln!("Downloading embedding model (all-MiniLM-L6-v2)...");
        download_file(MODEL_URL, &model_path)?;
        eprintln!("Model downloaded to {}", model_path.display());
    }
    if !tokenizer_path.exists() {
        eprintln!("Downloading tokenizer...");
        download_file(TOKENIZER_URL, &tokenizer_path)?;
    }

    Ok(cache_dir)
}

fn download_file(url: &str, dest: &std::path::Path) -> Result<()> {
    let response = reqwest::blocking::get(url)?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download {}: HTTP {}", url, response.status());
    }
    let bytes = response.bytes()?;
    std::fs::write(dest, &bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RegionKind;

    fn make_region() -> Region {
        Region {
            region_id: 1,
            file_id: 1,
            parent_region_id: None,
            kind: RegionKind::Function,
            name: "do_work".to_string(),
            start_line: 10,
            end_line: 20,
            start_byte: 100,
            end_byte: 300,
            signature: "fn do_work(x: i32) -> i32".to_string(),
            content_hash: "abc123".to_string(),
        }
    }

    #[test]
    fn compose_text_includes_all_signals() {
        let region = make_region();
        let summary = "Computes the result by doubling x.";
        let deps = vec!["helper".to_string(), "Logger".to_string()];

        let text = compose_embedding_text("rust", &region, Some(summary), "", &deps);

        assert!(text.contains("rust"), "should contain language");
        assert!(text.contains("function"), "should contain kind");
        assert!(text.contains("fn do_work(x: i32) -> i32"), "should contain signature");
        assert!(text.contains(summary), "should contain summary");
        assert!(text.contains("helper"), "should contain dep 'helper'");
        assert!(text.contains("Logger"), "should contain dep 'Logger'");
        assert!(text.contains("Dependencies:"), "should have Dependencies label");
    }

    #[test]
    fn compose_text_falls_back_to_source_when_no_summary() {
        let region = make_region();
        let source = "fn do_work(x: i32) -> i32 {\n    x * 2\n}";
        let text = compose_embedding_text("rust", &region, None, source, &[]);
        assert!(text.contains("x * 2"), "should contain source body");
    }

    #[test]
    fn mock_provider_returns_correct_dimensions() {
        let provider = MockEmbeddingProvider::new(128);
        let texts = vec!["hello world".to_string(), "foo bar".to_string()];
        let vecs = provider.embed(&texts).unwrap();

        assert_eq!(vecs.len(), 2, "one vector per text");
        for v in &vecs {
            assert_eq!(v.len(), 128, "each vector has correct dimension");
        }
    }

    #[test]
    fn mock_provider_is_deterministic() {
        let provider = MockEmbeddingProvider::new(64);
        let texts = vec!["deterministic input".to_string()];
        let v1 = provider.embed(&texts).unwrap();
        let v2 = provider.embed(&texts).unwrap();
        assert_eq!(v1, v2, "same input must produce same vector");
    }

    #[test]
    fn mock_provider_different_inputs_differ() {
        let provider = MockEmbeddingProvider::new(64);
        let v1 = provider.embed(&["text A".to_string()]).unwrap();
        let v2 = provider.embed(&["text B".to_string()]).unwrap();
        assert_ne!(v1[0], v2[0], "different inputs should produce different vectors");
    }
}
