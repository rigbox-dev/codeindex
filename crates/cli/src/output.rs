use codeindex_core::model::QueryResponse;

use colored::Colorize;

/// Human-readable multi-line output for each result.
pub fn print_human(response: &QueryResponse) {
    println!(
        "{}  {}",
        "Query:".bold(),
        response.query.cyan()
    );
    println!(
        "{} {}",
        "Results:".bold(),
        response.results.len()
    );
    println!();

    for (i, result) in response.results.iter().enumerate() {
        let n = i + 1;
        let header = format!(
            "#{} {} ({}) {}:{}-{}",
            n,
            result.name.bold(),
            result.kind.to_string().yellow(),
            result.file,
            result.lines[0],
            result.lines[1],
        );
        println!("{}", header);

        // Signature
        if !result.signature.is_empty() {
            println!("  {}", result.signature.dimmed());
        }

        // Summary
        if let Some(ref s) = result.summary {
            println!("  {}", s.italic());
        }

        // Relevance bar
        let score = result.relevance;
        let bar_len = (score * 20.0).round() as usize;
        let bar = "█".repeat(bar_len.min(20));
        let empty = "░".repeat(20 - bar_len.min(20));
        println!(
            "  Relevance: {:.3}  {}{}",
            score,
            bar.green(),
            empty.dimmed()
        );

        // Dependencies
        let dep_names: Vec<&str> = result
            .dependencies
            .calls
            .iter()
            .map(|d| d.name.as_str())
            .chain(result.dependencies.type_references.iter().map(|d| d.name.as_str()))
            .collect();

        if !dep_names.is_empty() {
            println!("  Deps: {}", dep_names.join(", ").blue());
        }

        println!();
    }
}

/// Compact one-line-per-result output.
pub fn print_compact(response: &QueryResponse) {
    for result in &response.results {
        let summary_or_sig = result
            .summary
            .as_deref()
            .unwrap_or(&result.signature);

        println!(
            "{:.3} | {}:{}-{} | {} | {}",
            result.relevance,
            result.file,
            result.lines[0],
            result.lines[1],
            result.name,
            summary_or_sig,
        );
    }
}
