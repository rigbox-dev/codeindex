use codeindex_core::model::RegionKind;
use tree_sitter::{Language, Node, Tree};

use crate::{ExtractedDependency, ExtractedRegion, LanguagePlugin};

pub struct RustPlugin;

impl LanguagePlugin for RustPlugin {
    fn language_id(&self) -> &str {
        "rust"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".rs"]
    }

    fn tree_sitter_language(&self) -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    fn extract_regions(&self, tree: &Tree, source: &[u8]) -> Vec<ExtractedRegion> {
        let mut regions = Vec::new();
        walk_node(tree.root_node(), source, &mut regions, None, false);
        regions
    }

    fn extract_dependencies(
        &self,
        _tree: &Tree,
        _source: &[u8],
        _regions: &[ExtractedRegion],
    ) -> Vec<ExtractedDependency> {
        // Task 5 will add dependency extraction.
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the UTF-8 text for a node, or an empty string on error.
fn node_text<'a>(node: Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// Extract the "signature" of a node: the text from the node start up to (but
/// not including) the body block, trimmed.
fn signature_of(node: Node, source: &[u8]) -> String {
    // Body nodes in Rust grammar are called "block" or "declaration_list" or
    // "enum_variant_list" / "field_declaration_list".
    let body_kinds = [
        "block",
        "declaration_list",
        "enum_variant_list",
        "field_declaration_list",
    ];

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

/// Recursively walk the AST, collecting regions.
///
/// `inside_impl_or_trait` indicates we are directly inside an impl/trait body,
/// so `function_item` nodes should be treated as Methods.
fn walk_node(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_impl_index: Option<usize>,
    inside_impl_or_trait: bool,
) {
    match node.kind() {
        "function_item" => {
            let name = child_text_by_field(node, "name", source);
            let kind = if inside_impl_or_trait {
                RegionKind::Method
            } else {
                RegionKind::Function
            };
            let region = make_region(node, kind, name, source, parent_impl_index);
            regions.push(region);
        }
        "struct_item" => {
            let name = child_text_by_field(node, "name", source);
            let region = make_region(node, RegionKind::Struct, name, source, None);
            regions.push(region);
        }
        "enum_item" => {
            let name = child_text_by_field(node, "name", source);
            let region = make_region(node, RegionKind::Enum, name, source, None);
            regions.push(region);
        }
        "trait_item" => {
            let name = child_text_by_field(node, "name", source);
            let trait_index = regions.len();
            let region = make_region(node, RegionKind::Interface, name, source, None);
            regions.push(region);
            // Recurse into trait body for method declarations.
            walk_children_for_methods(node, source, regions, Some(trait_index));
        }
        "impl_item" => {
            // Build a display name for the impl block.
            let name = impl_name(node, source);
            let impl_index = regions.len();
            let region = make_region(node, RegionKind::ImplBlock, name, source, None);
            regions.push(region);
            // Recurse into declaration_list for methods.
            walk_children_for_methods(node, source, regions, Some(impl_index));
        }
        _ => {
            // For any other node, recurse into its children.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(child, source, regions, parent_impl_index, inside_impl_or_trait);
            }
        }
    }
}

/// Recurse into the body of an impl or trait, tagging functions as Methods.
fn walk_children_for_methods(
    parent: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
) {
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if child.kind() == "declaration_list" {
            let mut inner = child.walk();
            for item in child.children(&mut inner) {
                walk_node(item, source, regions, parent_index, true);
            }
        }
    }
}

/// Build a human-readable name for an impl block, e.g. "AuthHandler" or
/// "Authenticator for AuthHandler".
fn impl_name(node: Node, source: &[u8]) -> String {
    // Look for children: type_identifier and optional `for <type>`
    let mut cursor = node.walk();
    let mut parts: Vec<&str> = Vec::new();
    let mut found_for = false;
    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" | "generic_type" | "scoped_type_identifier" => {
                parts.push(node_text(child, source));
            }
            "for" => {
                found_for = true;
            }
            _ => {}
        }
    }
    if found_for && parts.len() == 2 {
        format!("{} for {}", parts[0], parts[1])
    } else {
        parts.join("")
    }
}

/// Get the text of a named field child (tree-sitter field accessor).
fn child_text_by_field<'a>(node: Node, field: &str, source: &'a [u8]) -> String {
    node.child_by_field_name(field)
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default()
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_source;
    use codeindex_core::model::RegionKind;

    fn parse(src: &str) -> (Tree, Vec<u8>) {
        let bytes = src.as_bytes().to_vec();
        let tree = parse_source(&RustPlugin, &bytes).expect("parse failed");
        (tree, bytes)
    }

    #[test]
    fn extracts_functions() {
        let src = r#"fn hello() { let x = 1; }
pub fn world(x: i32) -> String { String::new() }"#;
        let (tree, bytes) = parse(src);
        let regions = RustPlugin.extract_regions(&tree, &bytes);

        let fns: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Function)
            .collect();

        assert_eq!(fns.len(), 2, "expected 2 functions, got {:?}", fns.len());
        assert_eq!(fns[0].name, "hello");
        assert_eq!(fns[1].name, "world");

        // Signatures should not include the body.
        assert!(
            fns[0].signature.contains("fn hello()"),
            "unexpected signature: {}",
            fns[0].signature
        );
        assert!(
            fns[1].signature.contains("fn world(x: i32) -> String"),
            "unexpected signature: {}",
            fns[1].signature
        );

        // Lines are 1-based.
        assert_eq!(fns[0].start_line, 1);
        assert_eq!(fns[1].start_line, 2);
    }

    #[test]
    fn extracts_structs_and_impl_blocks() {
        let src = r#"pub struct Foo {
    x: i32,
}

impl Foo {
    pub fn bar(&self) -> i32 {
        self.x
    }
}
"#;
        let (tree, bytes) = parse(src);
        let regions = RustPlugin.extract_regions(&tree, &bytes);

        let structs: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Struct)
            .collect();
        let impls: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::ImplBlock)
            .collect();
        let methods: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Method)
            .collect();

        assert_eq!(structs.len(), 1, "expected 1 struct");
        assert_eq!(structs[0].name, "Foo");

        assert_eq!(impls.len(), 1, "expected 1 impl block");

        assert_eq!(methods.len(), 1, "expected 1 method");
        assert_eq!(methods[0].name, "bar");

        // The method's parent_index should point to the impl block.
        let impl_index = regions
            .iter()
            .position(|r| r.kind == RegionKind::ImplBlock)
            .expect("impl block not found");
        assert_eq!(
            methods[0].parent_index,
            Some(impl_index),
            "method's parent_index should point to the impl block"
        );
    }

    #[test]
    fn extracts_traits() {
        let src = r#"pub trait Greeter {
    fn greet(&self) -> String;
}
"#;
        let (tree, bytes) = parse(src);
        let regions = RustPlugin.extract_regions(&tree, &bytes);

        let traits: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Interface)
            .collect();

        assert_eq!(traits.len(), 1, "expected 1 trait");
        assert_eq!(traits[0].name, "Greeter");
    }

    #[test]
    fn extracts_enums() {
        let src = r#"pub enum Color {
    Red,
    Green,
    Blue,
}
"#;
        let (tree, bytes) = parse(src);
        let regions = RustPlugin.extract_regions(&tree, &bytes);

        let enums: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Enum)
            .collect();

        assert_eq!(enums.len(), 1, "expected 1 enum");
        assert_eq!(enums[0].name, "Color");
    }
}
