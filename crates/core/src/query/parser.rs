/// A parsed representation of a user query.
#[derive(Debug, PartialEq)]
pub enum ParsedQuery {
    NaturalLanguage(String),
    SymbolLookup(String),
    DependencyQuery { file: String, symbol: String },
    FileScope(String),
}

/// Parse the raw user input string into a structured `ParsedQuery`.
///
/// Dispatch rules (checked in order):
/// - `:symbol <name>` → `SymbolLookup`
/// - `:deps <file>::<symbol>` → `DependencyQuery`
/// - `:file <path>` → `FileScope`
/// - Anything else → `NaturalLanguage`
pub fn parse_query(input: &str) -> ParsedQuery {
    let trimmed = input.trim();

    if let Some(rest) = trimmed.strip_prefix(":symbol ") {
        return ParsedQuery::SymbolLookup(rest.trim().to_string());
    }

    if let Some(rest) = trimmed.strip_prefix(":deps ") {
        let rest = rest.trim();
        // Split on `::` to separate file from symbol
        if let Some(sep) = rest.find("::") {
            let file = rest[..sep].trim().to_string();
            let symbol = rest[sep + 2..].trim().to_string();
            return ParsedQuery::DependencyQuery { file, symbol };
        }
    }

    if let Some(rest) = trimmed.strip_prefix(":file ") {
        return ParsedQuery::FileScope(rest.trim().to_string());
    }

    ParsedQuery::NaturalLanguage(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_natural_language() {
        let q = parse_query("how does authentication work");
        assert_eq!(
            q,
            ParsedQuery::NaturalLanguage("how does authentication work".to_string())
        );
    }

    #[test]
    fn parses_symbol_lookup() {
        let q = parse_query(":symbol AuthRequest");
        assert_eq!(q, ParsedQuery::SymbolLookup("AuthRequest".to_string()));
    }

    #[test]
    fn parses_dependency_query() {
        let q = parse_query(":deps src/auth/types.rs::AuthRequest");
        assert_eq!(
            q,
            ParsedQuery::DependencyQuery {
                file: "src/auth/types.rs".to_string(),
                symbol: "AuthRequest".to_string(),
            }
        );
    }

    #[test]
    fn parses_file_scope() {
        let q = parse_query(":file src/auth/types.rs");
        assert_eq!(
            q,
            ParsedQuery::FileScope("src/auth/types.rs".to_string())
        );
    }
}
