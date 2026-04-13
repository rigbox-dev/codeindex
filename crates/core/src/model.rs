use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionKind {
    Function,
    Method,
    Class,
    Struct,
    Module,
    Interface,
    ImplBlock,
    Enum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Calls,
    Imports,
    TypeReference,
    Inherits,
    Implements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedFile {
    pub file_id: i64,
    pub path: String,
    pub content_hash: String,
    pub language: String,
    pub last_indexed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub region_id: i64,
    pub file_id: i64,
    pub parent_region_id: Option<i64>,
    pub kind: RegionKind,
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub start_byte: u32,
    pub end_byte: u32,
    pub signature: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub source_region_id: i64,
    pub target_region_id: Option<i64>,
    pub target_symbol: String,
    pub target_path: Option<String>,
    pub kind: DependencyKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub region_id: i64,
    pub summary_text: String,
    pub generated_at: i64,
    pub model_version: String,
}

/// The response returned to callers after a query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResponse {
    pub query: String,
    pub results: Vec<QueryResult>,
    pub graph_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub region_id: i64,
    pub file: String,
    pub kind: RegionKind,
    pub name: String,
    pub lines: [u32; 2],
    pub signature: String,
    pub summary: Option<String>,
    pub relevance: f64,
    pub code: Option<String>,
    pub dependencies: DependencyInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DependencyInfo {
    pub calls: Vec<DependencyRef>,
    pub called_by: Vec<DependencyRef>,
    pub type_references: Vec<DependencyRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyRef {
    pub name: String,
    pub file: String,
    pub lines: [u32; 2],
}

impl From<codeindex_tree_sitter_langs::RegionKind> for RegionKind {
    fn from(k: codeindex_tree_sitter_langs::RegionKind) -> Self {
        use codeindex_tree_sitter_langs::RegionKind as R;
        match k {
            R::Function => Self::Function,
            R::Method => Self::Method,
            R::Class => Self::Class,
            R::Struct => Self::Struct,
            R::Module => Self::Module,
            R::Interface => Self::Interface,
            R::ImplBlock => Self::ImplBlock,
            R::Enum => Self::Enum,
        }
    }
}

impl From<codeindex_tree_sitter_langs::DependencyKind> for DependencyKind {
    fn from(k: codeindex_tree_sitter_langs::DependencyKind) -> Self {
        use codeindex_tree_sitter_langs::DependencyKind as D;
        match k {
            D::Calls => Self::Calls,
            D::Imports => Self::Imports,
            D::TypeReference => Self::TypeReference,
            D::Inherits => Self::Inherits,
            D::Implements => Self::Implements,
        }
    }
}

impl RegionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Module => "module",
            Self::Interface => "interface",
            Self::ImplBlock => "impl_block",
            Self::Enum => "enum",
        }
    }
}

impl std::fmt::Display for RegionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl DependencyKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Calls => "calls",
            Self::Imports => "imports",
            Self::TypeReference => "type_reference",
            Self::Inherits => "inherits",
            Self::Implements => "implements",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_kind_serializes_to_snake_case() {
        let json = serde_json::to_string(&RegionKind::ImplBlock).unwrap();
        assert_eq!(json, "\"impl_block\"");
    }

    #[test]
    fn region_kind_deserializes_from_snake_case() {
        let kind: RegionKind = serde_json::from_str("\"impl_block\"").unwrap();
        assert_eq!(kind, RegionKind::ImplBlock);
    }

    #[test]
    fn dependency_kind_round_trips() {
        for kind in [
            DependencyKind::Calls,
            DependencyKind::Imports,
            DependencyKind::TypeReference,
            DependencyKind::Inherits,
            DependencyKind::Implements,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: DependencyKind = serde_json::from_str(&json).unwrap();
            assert_eq!(kind, back);
        }
    }

    #[test]
    fn query_response_serializes_to_spec_format() {
        let response = QueryResponse {
            query: "test".into(),
            results: vec![QueryResult {
                region_id: 1,
                file: "src/main.rs".into(),
                kind: RegionKind::Function,
                name: "main".into(),
                lines: [1, 10],
                signature: "fn main()".into(),
                summary: None,
                relevance: 0.95,
                code: Some("fn main() {}".into()),
                dependencies: DependencyInfo::default(),
            }],
            graph_context: None,
        };
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["results"][0]["kind"], "function");
        assert_eq!(json["results"][0]["lines"], serde_json::json!([1, 10]));
    }
}
