//! Local ONNX-based embedding provider.
//!
//! This module is only compiled when the `local-embeddings` feature is enabled.
//! It uses the `ort` crate (ONNX Runtime bindings) to run inference on a locally
//! stored ONNX model file without any network calls.
//!
//! # Usage
//!
//! Enable the feature in your `Cargo.toml`:
//!
//! ```toml
//! codeindex-core = { path = "...", features = ["local-embeddings"] }
//! ```
//!
//! Then construct a provider pointing at a downloaded ONNX model:
//!
//! ```no_run
//! # #[cfg(feature = "local-embeddings")]
//! # {
//! use codeindex_core::embedding::local::LocalEmbeddingProvider;
//! let provider = LocalEmbeddingProvider::from_path("model.onnx", 384, "all-MiniLM-L6-v2").unwrap();
//! # }
//! ```

#[cfg(feature = "local-embeddings")]
use anyhow::{Context, Result};
#[cfg(feature = "local-embeddings")]
use ort::{inputs, Session};

#[cfg(feature = "local-embeddings")]
use super::EmbeddingProvider;

/// An embedding provider that runs a local ONNX model via ONNX Runtime.
///
/// Requires the `local-embeddings` feature flag.
#[cfg(feature = "local-embeddings")]
pub struct LocalEmbeddingProvider {
    session: Session,
    dimension: usize,
    model_name: String,
}

#[cfg(feature = "local-embeddings")]
impl LocalEmbeddingProvider {
    /// Load an ONNX model from `model_path` and prepare it for inference.
    ///
    /// # Parameters
    /// - `model_path`: Path to the `.onnx` model file on disk.
    /// - `dimension`: The output embedding dimensionality of the model.
    /// - `model_name`: A stable human-readable identifier stored in metadata.
    pub fn from_path(
        model_path: impl AsRef<std::path::Path>,
        dimension: usize,
        model_name: impl Into<String>,
    ) -> Result<Self> {
        let path = model_path.as_ref();
        let session = Session::builder()
            .context("failed to create ONNX Runtime session builder")?
            .commit_from_file(path)
            .with_context(|| format!("failed to load ONNX model from {}", path.display()))?;

        Ok(Self {
            session,
            dimension,
            model_name: model_name.into(),
        })
    }

    /// Tokenise `text` into a flat sequence of token-id `i64` values.
    ///
    /// This is a placeholder implementation that maps each byte to its value.
    /// Replace this with a real tokeniser (e.g. `tokenizers` crate) before
    /// using in production.
    fn simple_tokenize(text: &str) -> Vec<i64> {
        text.bytes().map(|b| b as i64).collect()
    }
}

#[cfg(feature = "local-embeddings")]
impl EmbeddingProvider for LocalEmbeddingProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());

        for text in texts {
            let token_ids = Self::simple_tokenize(text);
            let seq_len = token_ids.len();

            // Build input tensors: input_ids [1, seq_len] and attention_mask [1, seq_len].
            let input_ids: ndarray::Array2<i64> =
                ndarray::Array2::from_shape_vec((1, seq_len), token_ids)
                    .context("failed to build input_ids tensor")?;
            let attention_mask: ndarray::Array2<i64> =
                ndarray::Array2::ones((1, seq_len));

            let outputs = self
                .session
                .run(inputs![
                    "input_ids" => input_ids.view(),
                    "attention_mask" => attention_mask.view(),
                ]?)
                .context("ONNX inference failed")?;

            // Most sentence-transformer ONNX exports place the pooled embedding
            // in the first output tensor under the key "last_hidden_state" or
            // "sentence_embedding".  We take the first output and mean-pool it.
            let output_tensor = outputs[0]
                .try_extract_tensor::<f32>()
                .context("failed to extract f32 tensor from model output")?;

            // Shape is typically [1, seq_len, hidden] or [1, hidden].
            // Mean-pool over the sequence dimension if present.
            let shape = output_tensor.shape().to_vec();
            let embedding: Vec<f32> = match shape.len() {
                2 => {
                    // [1, hidden] — already pooled
                    output_tensor.iter().take(self.dimension).copied().collect()
                }
                3 => {
                    // [1, seq_len, hidden] — mean-pool over seq_len
                    let hidden = shape[2];
                    let seq = shape[1];
                    let flat: Vec<f32> = output_tensor.iter().copied().collect();
                    (0..hidden)
                        .map(|h| {
                            let sum: f32 = (0..seq).map(|s| flat[s * hidden + h]).sum();
                            sum / seq as f32
                        })
                        .take(self.dimension)
                        .collect()
                }
                _ => anyhow::bail!("unexpected output tensor rank {}", shape.len()),
            };

            // L2-normalise so cosine similarity works out of the box.
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            let normalised = if norm > 0.0 {
                embedding.iter().map(|x| x / norm).collect()
            } else {
                embedding
            };

            results.push(normalised);
        }

        Ok(results)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_id(&self) -> &str {
        &self.model_name
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[cfg(feature = "local-embeddings")]
mod tests {
    use super::*;

    /// This test requires a real ONNX model on disk and is therefore ignored by
    /// default.  To run it, place a valid sentence-transformer ONNX model at
    /// `/tmp/test_model.onnx` and execute:
    ///
    /// ```bash
    /// cargo test -p codeindex-core --features local-embeddings -- local::tests --ignored
    /// ```
    #[test]
    #[ignore = "requires a real ONNX model on disk at /tmp/test_model.onnx"]
    fn local_provider_returns_correct_dimension() {
        let provider =
            LocalEmbeddingProvider::from_path("/tmp/test_model.onnx", 384, "test-model")
                .expect("failed to load model");

        let texts = vec!["hello world".to_string()];
        let vecs = provider.embed(&texts).expect("inference failed");

        assert_eq!(vecs.len(), 1);
        assert_eq!(vecs[0].len(), 384);

        // Verify L2 norm ≈ 1.0
        let norm: f32 = vecs[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "embedding should be unit-normalised");
    }
}
