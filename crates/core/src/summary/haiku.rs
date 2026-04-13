use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;

use super::SummaryProvider;

pub struct HaikuSummaryProvider {
    api_key: String,
    model: String,
}

impl HaikuSummaryProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
        }
    }

    pub fn from_env(api_key_env: &str, model: impl Into<String>) -> Result<Self> {
        let api_key = std::env::var(api_key_env)
            .with_context(|| format!("Environment variable '{}' not set", api_key_env))?;
        Ok(Self::new(api_key, model))
    }
}

#[derive(Deserialize)]
struct ContentBlock {
    text: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ContentBlock>,
}

impl SummaryProvider for HaikuSummaryProvider {
    fn summarize_batch(&self, items: &[(String, String)]) -> Result<Vec<String>> {
        let client = reqwest::blocking::Client::new();
        let mut summaries = Vec::with_capacity(items.len());

        for (signature, code) in items {
            let prompt = format!(
                "Provide a 2-3 sentence summary of the following code.\n\nSignature: {}\n\nCode:\n{}",
                signature, code
            );

            let body = json!({
                "model": self.model,
                "max_tokens": 150,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ]
            });

            let response = client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .context("Failed to send request to Anthropic API")?;

            let status = response.status();
            if !status.is_success() {
                let text = response.text().unwrap_or_default();
                anyhow::bail!("Anthropic API returned {}: {}", status, text);
            }

            let api_response: ApiResponse = response
                .json()
                .context("Failed to parse Anthropic API response")?;

            let text = api_response
                .content
                .into_iter()
                .next()
                .map(|block| block.text)
                .unwrap_or_default();

            summaries.push(text);
        }

        Ok(summaries)
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}
