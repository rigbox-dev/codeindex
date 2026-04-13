use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use super::EmbeddingProvider;

pub struct VoyageEmbeddingProvider {
    api_key: String,
    model: String,
    dimension: usize,
}

impl VoyageEmbeddingProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        let dimension = match model.as_str() {
            "voyage-code-3" => 1024,
            _ => 1024,
        };
        Self {
            api_key: api_key.into(),
            model,
            dimension,
        }
    }

    pub fn from_env(model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var("VOYAGE_API_KEY")
            .context("VOYAGE_API_KEY environment variable not set")?;
        Ok(Self::new(api_key, model))
    }
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
    input_type: &'a str,
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
}

impl EmbeddingProvider for VoyageEmbeddingProvider {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let client = reqwest::blocking::Client::new();
        let body = EmbedRequest {
            model: &self.model,
            input: texts,
            input_type: "document",
        };

        let response = client
            .post("https://api.voyageai.com/v1/embeddings")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .context("Failed to send request to Voyage API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(anyhow!("Voyage API returned {}: {}", status, text));
        }

        let parsed: EmbedResponse = response
            .json()
            .context("Failed to parse Voyage API response")?;

        Ok(parsed.data.into_iter().map(|d| d.embedding).collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn voyage_embeds_text() {
        let provider = VoyageEmbeddingProvider::from_env("voyage-code-3").unwrap();
        let vectors = provider.embed(&["fn main() {}".into()]).unwrap();
        assert_eq!(vectors.len(), 1);
        assert_eq!(vectors[0].len(), 1024);
    }
}
