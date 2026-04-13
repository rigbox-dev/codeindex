use anyhow::{Context, Result};
use ort::session::Session;
use ort::value::TensorRef;
use std::sync::Mutex;
use tokenizers::Tokenizer;

use super::EmbeddingProvider;

/// Embedding provider using a local ONNX sentence-transformer model.
pub struct LocalEmbeddingProvider {
    session: Mutex<Session>,
    tokenizer: Tokenizer,
    dimension: usize,
    model_name: String,
}

impl LocalEmbeddingProvider {
    /// Load model and tokenizer from a directory containing `model.onnx` and `tokenizer.json`.
    pub fn from_dir(dir: &std::path::Path) -> Result<Self> {
        let model_path = dir.join("model.onnx");
        let tokenizer_path = dir.join("tokenizer.json");

        let session = Session::builder()
            .context("failed to create ONNX session builder")?
            .commit_from_file(&model_path)
            .with_context(|| format!("failed to load model from {}", model_path.display()))?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {}", e))?;

        Ok(Self {
            session: Mutex::new(session),
            tokenizer,
            dimension: 384,
            model_name: "all-MiniLM-L6-v2".to_string(),
        })
    }
}

impl EmbeddingProvider for LocalEmbeddingProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut session = self
            .session
            .lock()
            .map_err(|e| anyhow::anyhow!("session mutex poisoned: {}", e))?;
        let mut results = Vec::with_capacity(texts.len());

        for text in texts {
            let encoding = self
                .tokenizer
                .encode(text.as_str(), true)
                .map_err(|e| anyhow::anyhow!("tokenization failed: {}", e))?;

            let token_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
            let attention_mask: Vec<i64> = encoding
                .get_attention_mask()
                .iter()
                .map(|&m| m as i64)
                .collect();
            let token_type_ids: Vec<i64> = encoding
                .get_type_ids()
                .iter()
                .map(|&t| t as i64)
                .collect();
            let seq_len = token_ids.len();

            // Use tuple (shape, data_slice) API to avoid ndarray version conflicts.
            let shape: &[i64] = &[1, seq_len as i64];
            let input_ids_ref =
                TensorRef::<i64>::from_array_view((shape, token_ids.as_slice()))
                    .context("failed to create input_ids tensor")?;
            let mask_ref =
                TensorRef::<i64>::from_array_view((shape, attention_mask.as_slice()))
                    .context("failed to create attention_mask tensor")?;
            let type_ids_ref =
                TensorRef::<i64>::from_array_view((shape, token_type_ids.as_slice()))
                    .context("failed to create token_type_ids tensor")?;

            let outputs = session
                .run(ort::inputs![
                    "input_ids" => input_ids_ref,
                    "attention_mask" => mask_ref,
                    "token_type_ids" => type_ids_ref,
                ])
                .context("ONNX inference failed")?;

            // try_extract_tensor returns (&Shape, &[T])
            let (shape_ref, data) = outputs[0]
                .try_extract_tensor::<f32>()
                .context("failed to extract output tensor")?;

            let shape_dims = shape_ref.as_ref();
            let embedding: Vec<f32> = match shape_dims.len() {
                2 => {
                    // Shape [1, hidden] — already pooled
                    data.iter().take(self.dimension).copied().collect()
                }
                3 => {
                    // Shape [1, seq_len, hidden] — mean-pool over seq_len
                    let seq = shape_dims[1] as usize;
                    let hidden = shape_dims[2] as usize;
                    (0..hidden.min(self.dimension))
                        .map(|h| {
                            let sum: f32 = (0..seq).map(|s| data[s * hidden + h]).sum();
                            sum / seq as f32
                        })
                        .collect()
                }
                _ => anyhow::bail!("unexpected output tensor rank {}", shape_dims.len()),
            };

            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            let normalized = if norm > 0.0 {
                embedding.iter().map(|x| x / norm).collect()
            } else {
                embedding
            };

            results.push(normalized);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::ensure_model;

    #[test]
    #[ignore = "downloads ~22MB model from HuggingFace"]
    fn local_provider_embeds_text() {
        let model_dir = ensure_model().expect("model download failed");
        let provider = LocalEmbeddingProvider::from_dir(&model_dir).expect("load failed");

        let texts = vec!["fn authenticate(user: &str) -> bool".to_string()];
        let vecs = provider.embed(&texts).expect("embed failed");

        assert_eq!(vecs.len(), 1);
        assert_eq!(vecs[0].len(), 384);

        let norm: f32 = vecs[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }
}
