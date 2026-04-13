pub mod haiku;

use anyhow::Result;

pub trait SummaryProvider: Send + Sync {
    fn summarize_batch(&self, items: &[(String, String)]) -> Result<Vec<String>>;
    fn model_id(&self) -> &str;
}

pub struct MockSummaryProvider;

impl SummaryProvider for MockSummaryProvider {
    fn summarize_batch(&self, items: &[(String, String)]) -> Result<Vec<String>> {
        Ok(items
            .iter()
            .map(|(sig, _code)| format!("Summary of {}", sig))
            .collect())
    }

    fn model_id(&self) -> &str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_provider_returns_summaries() {
        let provider = MockSummaryProvider;
        let items = vec![
            ("fn foo()".to_string(), "fn foo() { }".to_string()),
            ("fn bar()".to_string(), "fn bar() { }".to_string()),
        ];
        let summaries = provider.summarize_batch(&items).unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0], "Summary of fn foo()");
        assert_eq!(summaries[1], "Summary of fn bar()");
    }
}
