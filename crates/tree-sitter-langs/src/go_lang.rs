use tree_sitter::{Language, Node, Tree};

use crate::{
    find_enclosing_region, DependencyKind, ExtractedDependency, ExtractedRegion, LanguagePlugin,
    RegionKind,
};

pub struct GoPlugin;

impl LanguagePlugin for GoPlugin {
    fn language_id(&self) -> &str {
        "go"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".go"]
    }

    fn tree_sitter_language(&self) -> Language {
        tree_sitter_go::LANGUAGE.into()
    }

    fn extract_regions(&self, tree: &Tree, source: &[u8]) -> Vec<ExtractedRegion> {
        let mut regions = Vec::new();
        walk_node(tree.root_node(), source, &mut regions);
        regions
    }

    fn extract_dependencies(
        &self,
        tree: &Tree,
        source: &[u8],
        regions: &[ExtractedRegion],
    ) -> Vec<ExtractedDependency> {
        let mut deps = Vec::new();
        collect_dependencies(tree.root_node(), source, regions, &mut deps);
        deps
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn node_text<'a>(node: Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// Return the text up to (but not including) the body block.
fn signature_of(node: Node, source: &[u8]) -> String {
    let body_kinds = ["block", "field_declaration_list", "method_spec_list"];

    let end_byte = {
        let mut found = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if body_kinds.contains(&child.kind()) {
                found = Some(child.start_byte());
                break;
            }
        }
        found.unwrap_or_else(|| node.end_byte())
    };

    let slice = &source[node.start_byte()..end_byte];
    std::str::from_utf8(slice)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn make_region(
    node: Node,
    kind: RegionKind,
    name: String,
    source: &[u8],
    parent_index: Option<usize>,
) -> ExtractedRegion {
    ExtractedRegion {
        kind,
        name,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        start_byte: node.start_byte() as u32,
        end_byte: node.end_byte() as u32,
        signature: signature_of(node, source),
        parent_index,
    }
}

/// Recursively walk the AST collecting regions.
fn walk_node(node: Node, source: &[u8], regions: &mut Vec<ExtractedRegion>) {
    match node.kind() {
        "function_declaration" => {
            // func Name(...) ...
            let name = node
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let region = make_region(node, RegionKind::Function, name, source, None);
            regions.push(region);
        }
        "method_declaration" => {
            // func (recv Type) Name(...) ...
            let name = node
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let region = make_region(node, RegionKind::Method, name, source, None);
            regions.push(region);
        }
        "type_declaration" => {
            // type Foo struct { ... } / type Bar interface { ... }
            extract_type_declaration(node, source, regions);
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(child, source, regions);
            }
        }
    }
}

/// Handle a `type_declaration` node which may contain one or more `type_spec` nodes.
fn extract_type_declaration(node: Node, source: &[u8], regions: &mut Vec<ExtractedRegion>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_spec" {
            extract_type_spec(child, source, regions);
        }
    }
}

/// Handle a `type_spec` node.
fn extract_type_spec(node: Node, source: &[u8], regions: &mut Vec<ExtractedRegion>) {
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }

    // Determine the kind from the type field
    let type_node = match node.child_by_field_name("type") {
        Some(n) => n,
        None => return,
    };

    let kind = match type_node.kind() {
        "struct_type" => RegionKind::Struct,
        "interface_type" => RegionKind::Interface,
        _ => return, // Skip type aliases etc.
    };

    // The region should cover the type_spec node (not just the type_node)
    let region = make_region(node, kind, name, source, None);
    regions.push(region);
}

// ---------------------------------------------------------------------------
// Dependency extraction
// ---------------------------------------------------------------------------

fn collect_dependencies(
    node: Node,
    source: &[u8],
    regions: &[ExtractedRegion],
    deps: &mut Vec<ExtractedDependency>,
) {
    match node.kind() {
        "import_declaration" => {
            extract_import_declaration(node, source, regions, deps);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_dependencies(child, source, regions, deps);
    }
}

/// Extract import dependencies from an `import_declaration` node.
///
/// Handles both single imports:  `import "fmt"`
/// and grouped imports:          `import ( "fmt"\n "os" )`
fn extract_import_declaration(
    node: Node,
    source: &[u8],
    regions: &[ExtractedRegion],
    deps: &mut Vec<ExtractedDependency>,
) {
    let source_region_index = find_enclosing_region(node, regions).unwrap_or(0);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_spec" => {
                push_import_spec(child, source, source_region_index, deps);
            }
            "import_spec_list" => {
                let mut c2 = child.walk();
                for spec in child.children(&mut c2) {
                    if spec.kind() == "import_spec" {
                        push_import_spec(spec, source, source_region_index, deps);
                    }
                }
            }
            _ => {}
        }
    }
}

/// Push a single import_spec as a dependency.
fn push_import_spec(
    node: Node,
    source: &[u8],
    source_region_index: usize,
    deps: &mut Vec<ExtractedDependency>,
) {
    // The path is a `interpreted_string_literal` — strip quotes
    let path_node = match node.child_by_field_name("path") {
        Some(n) => n,
        None => return,
    };
    let full_path = node_text(path_node, source)
        .trim_matches('"')
        .trim_matches('`')
        .to_string();
    if full_path.is_empty() {
        return;
    }

    // Package name: either from explicit alias or last path segment
    let alias_node = node.child_by_field_name("name");
    let package_name = if let Some(alias) = alias_node {
        node_text(alias, source).to_string()
    } else {
        full_path
            .split('/')
            .last()
            .unwrap_or(&full_path)
            .to_string()
    };

    deps.push(ExtractedDependency {
        source_region_index,
        target_symbol: package_name,
        target_path: Some(full_path),
        kind: DependencyKind::Imports,
    });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_source, RegionKind};

    fn parse(src: &str) -> (Tree, Vec<u8>) {
        let bytes = src.as_bytes().to_vec();
        let tree = parse_source(&GoPlugin, &bytes).expect("parse failed");
        (tree, bytes)
    }

    #[test]
    fn extracts_go_structs_and_interfaces() {
        let src = r#"
package auth

type AuthRequest struct {
	Username string
	Password string
}

type AuthToken struct {
	Token     string
	ExpiresAt int64
}

type Authenticator interface {
	Authenticate(req AuthRequest) (AuthToken, error)
}
"#;
        let (tree, bytes) = parse(src);
        let regions = GoPlugin.extract_regions(&tree, &bytes);

        let structs: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Struct)
            .collect();
        let interfaces: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Interface)
            .collect();

        assert_eq!(structs.len(), 2, "expected 2 structs, got {}", structs.len());
        let struct_names: Vec<&str> = structs.iter().map(|r| r.name.as_str()).collect();
        assert!(struct_names.contains(&"AuthRequest"), "missing AuthRequest");
        assert!(struct_names.contains(&"AuthToken"), "missing AuthToken");

        assert_eq!(interfaces.len(), 1, "expected 1 interface, got {}", interfaces.len());
        assert_eq!(interfaces[0].name, "Authenticator");
    }

    #[test]
    fn extracts_go_functions_and_methods() {
        let src = r#"
package auth

type AuthHandler struct {
	db interface{}
}

func NewAuthHandler(db interface{}) *AuthHandler {
	return &AuthHandler{db: db}
}

func (h *AuthHandler) Authenticate(req AuthRequest) (AuthToken, error) {
	return AuthToken{}, nil
}
"#;
        let (tree, bytes) = parse(src);
        let regions = GoPlugin.extract_regions(&tree, &bytes);

        let fns: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Function)
            .collect();
        let methods: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Method)
            .collect();

        assert_eq!(fns.len(), 1, "expected 1 function, got {}", fns.len());
        assert_eq!(fns[0].name, "NewAuthHandler");

        assert_eq!(methods.len(), 1, "expected 1 method, got {}", methods.len());
        assert_eq!(methods[0].name, "Authenticate");
    }
}
