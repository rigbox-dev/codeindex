use tree_sitter::{Language, Node, Tree};

use crate::{
    ExtractedDependency, ExtractedRegion, LanguagePlugin, RegionKind,
};

use super::c_lang;

pub struct CppPlugin;

impl LanguagePlugin for CppPlugin {
    fn language_id(&self) -> &str {
        "cpp"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".cpp", ".hpp", ".cc", ".cxx", ".hh"]
    }

    fn tree_sitter_language(&self) -> Language {
        tree_sitter_cpp::LANGUAGE.into()
    }

    fn extract_regions(&self, tree: &Tree, source: &[u8]) -> Vec<ExtractedRegion> {
        let mut regions = Vec::new();
        walk_node(tree.root_node(), source, &mut regions, None, false);
        regions
    }

    fn extract_dependencies(
        &self,
        tree: &Tree,
        source: &[u8],
        regions: &[ExtractedRegion],
    ) -> Vec<ExtractedDependency> {
        let mut deps = Vec::new();
        // C's dependency walker handles #include and call_expression — both
        // work identically in C++.
        c_lang::collect_dependencies(tree.root_node(), source, regions, &mut deps);
        deps
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn node_text<'a>(node: Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

fn signature_of(node: Node, source: &[u8]) -> String {
    let body_kinds = [
        "compound_statement",
        "field_declaration_list",
        "enumerator_list",
        "declaration_list",
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

/// Return true if any direct child of `node` has the given kind string.
fn has_child_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).any(|c| c.kind() == kind);
    result
}

/// Walk the C++ AST collecting regions.
///
/// `inside_class` is true when we are walking direct children of a
/// class/struct body — this causes `function_definition` to be tagged as
/// `Method` rather than `Function`.
fn walk_node(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
    inside_class: bool,
) {
    match node.kind() {
        "function_definition" => {
            let name = node
                .child_by_field_name("declarator")
                .map(|d| extract_cpp_declarator_name(d, source))
                .unwrap_or_default();
            let kind = if inside_class {
                RegionKind::Method
            } else {
                RegionKind::Function
            };
            let region = make_region(node, kind, name, source, parent_index);
            regions.push(region);
            // Don't recurse — function bodies don't contain further top-level defs.
        }
        "class_specifier" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let class_index = regions.len();
            let region = make_region(node, RegionKind::Class, name, source, parent_index);
            regions.push(region);
            // Recurse into the body to find methods.
            walk_class_body(node, source, regions, Some(class_index));
        }
        "struct_specifier" => {
            if has_child_kind(node, "field_declaration_list") {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source).to_string())
                    .unwrap_or_default();
                let struct_index = regions.len();
                let region = make_region(node, RegionKind::Struct, name, source, parent_index);
                regions.push(region);
                // Recurse into struct body for inline method definitions.
                walk_class_body(node, source, regions, Some(struct_index));
            } else {
                recurse_children(node, source, regions, parent_index, inside_class);
            }
        }
        "enum_specifier" => {
            if has_child_kind(node, "enumerator_list") {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source).to_string())
                    .unwrap_or_default();
                let region = make_region(node, RegionKind::Enum, name, source, parent_index);
                regions.push(region);
            } else {
                recurse_children(node, source, regions, parent_index, inside_class);
            }
        }
        "namespace_definition" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let ns_index = regions.len();
            let region = make_region(node, RegionKind::Module, name, source, parent_index);
            regions.push(region);
            // Recurse into namespace body. Functions inside are NOT methods.
            walk_namespace_body(node, source, regions, Some(ns_index));
        }
        _ => {
            recurse_children(node, source, regions, parent_index, inside_class);
        }
    }
}

/// Recurse into child nodes without changing the parent index or inside_class flag.
fn recurse_children(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
    inside_class: bool,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node(child, source, regions, parent_index, inside_class);
    }
}

/// Recurse into the `field_declaration_list` body of a class/struct, marking
/// function_definition children as Methods.
fn walk_class_body(
    parent: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    class_index: Option<usize>,
) {
    let mut cursor = parent.walk();
    for child in parent.children(&mut cursor) {
        if child.kind() == "field_declaration_list" {
            let mut inner = child.walk();
            for item in child.children(&mut inner) {
                walk_node(item, source, regions, class_index, true);
            }
        }
    }
}

/// Recurse into the `declaration_list` body of a namespace, keeping functions
/// as Function (not Method).
fn walk_namespace_body(
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
                walk_node(item, source, regions, parent_index, false);
            }
        }
    }
}

/// Extract a function/method name from a C++ declarator node.
///
/// C++ adds `qualified_identifier` (e.g. `ClassName::method`),
/// `reference_declarator`, and `field_identifier` (used for member function
/// names inside class bodies) on top of what C has.
fn extract_cpp_declarator_name(node: Node, source: &[u8]) -> String {
    match node.kind() {
        "identifier" | "field_identifier" => node_text(node, source).to_string(),
        "destructor_name" => node_text(node, source).to_string(),
        "qualified_identifier" => {
            // e.g. `Foo::bar` — the `name` field is the last segment.
            node.child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_else(|| node_text(node, source).to_string())
        }
        "function_declarator" | "pointer_declarator" | "reference_declarator" => {
            if let Some(inner) = node.child_by_field_name("declarator") {
                extract_cpp_declarator_name(inner, source)
            } else {
                String::new()
            }
        }
        _ => {
            // Try named `declarator` field first.
            if let Some(inner) = node.child_by_field_name("declarator") {
                let name = extract_cpp_declarator_name(inner, source);
                if !name.is_empty() {
                    return name;
                }
            }
            // Fallback to C helper.
            c_lang::extract_declarator_name(node, source)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_source, DependencyKind, RegionKind};

    fn parse(src: &str) -> (Tree, Vec<u8>) {
        let bytes = src.as_bytes().to_vec();
        let tree = parse_source(&CppPlugin, &bytes).expect("parse failed");
        (tree, bytes)
    }

    #[test]
    fn extracts_cpp_classes_and_methods() {
        let src = r#"
class Animal {
public:
    Animal(const char* name) {}
    void speak() {}
    virtual ~Animal() {}
};
"#;
        let (tree, bytes) = parse(src);
        let regions = CppPlugin.extract_regions(&tree, &bytes);

        let classes: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Class)
            .collect();
        let methods: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Method)
            .collect();

        assert_eq!(classes.len(), 1, "expected 1 class, got {}", classes.len());
        assert_eq!(classes[0].name, "Animal");

        assert!(
            methods.len() >= 2,
            "expected at least 2 methods, got {}",
            methods.len()
        );

        let method_names: Vec<&str> = methods.iter().map(|r| r.name.as_str()).collect();
        assert!(
            method_names.contains(&"speak"),
            "missing 'speak', got {:?}",
            method_names
        );

        // All methods should have the class as parent.
        let class_index = regions
            .iter()
            .position(|r| r.kind == RegionKind::Class)
            .expect("class not found");
        for m in &methods {
            assert_eq!(
                m.parent_index,
                Some(class_index),
                "method '{}' has wrong parent_index",
                m.name
            );
        }
    }

    #[test]
    fn extracts_cpp_namespaces() {
        let src = r#"
namespace auth {
    int validate(const char* token) {
        return 1;
    }
}
"#;
        let (tree, bytes) = parse(src);
        let regions = CppPlugin.extract_regions(&tree, &bytes);

        let modules: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Module)
            .collect();
        let fns: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Function)
            .collect();

        assert_eq!(modules.len(), 1, "expected 1 namespace, got {}", modules.len());
        assert_eq!(modules[0].name, "auth");

        assert_eq!(fns.len(), 1, "expected 1 function, got {}", fns.len());
        assert_eq!(fns[0].name, "validate");

        let ns_index = regions
            .iter()
            .position(|r| r.kind == RegionKind::Module)
            .expect("namespace not found");
        assert_eq!(
            fns[0].parent_index,
            Some(ns_index),
            "function inside namespace should have namespace as parent"
        );
    }

    #[test]
    fn extracts_cpp_standalone_functions() {
        let src = r#"
int add(int a, int b) {
    return a + b;
}

double square(double x) {
    return x * x;
}
"#;
        let (tree, bytes) = parse(src);
        let regions = CppPlugin.extract_regions(&tree, &bytes);

        let fns: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Function)
            .collect();

        assert_eq!(fns.len(), 2, "expected 2 functions, got {}", fns.len());
        let names: Vec<&str> = fns.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"add"), "missing 'add'");
        assert!(names.contains(&"square"), "missing 'square'");
    }

    #[test]
    fn extracts_cpp_includes() {
        let src = r#"
#include <iostream>
#include "auth.h"

int main() {
    return 0;
}
"#;
        let (tree, bytes) = parse(src);
        let regions = CppPlugin.extract_regions(&tree, &bytes);
        let deps = CppPlugin.extract_dependencies(&tree, &bytes, &regions);

        let imports: Vec<_> = deps
            .iter()
            .filter(|d| d.kind == DependencyKind::Imports)
            .collect();

        assert!(
            imports.len() >= 2,
            "expected at least 2 imports, got {}",
            imports.len()
        );

        let paths: Vec<&str> = imports
            .iter()
            .filter_map(|d| d.target_path.as_deref())
            .collect();
        assert!(paths.contains(&"iostream"), "missing 'iostream', got {:?}", paths);
        assert!(paths.contains(&"auth.h"), "missing 'auth.h'");
    }
}
