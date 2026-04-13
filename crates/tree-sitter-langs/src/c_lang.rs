use tree_sitter::{Language, Node, Tree};

use crate::{
    find_enclosing_region, DependencyKind, ExtractedDependency, ExtractedRegion, LanguagePlugin,
    RegionKind,
};

pub struct CPlugin;

impl LanguagePlugin for CPlugin {
    fn language_id(&self) -> &str {
        "c"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".c", ".h"]
    }

    fn tree_sitter_language(&self) -> Language {
        tree_sitter_c::LANGUAGE.into()
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

fn signature_of(node: Node, source: &[u8]) -> String {
    let body_kinds = ["compound_statement", "field_declaration_list", "enumerator_list"];

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

/// Recursively extract the function name from a declarator node.
///
/// In tree-sitter-c, a function definition's `declarator` field may be:
///   - `function_declarator`  →  name is in its own `declarator` field
///   - `pointer_declarator`   →  recurse into its `declarator` field
///   - `identifier`           →  this is the name itself
pub fn extract_declarator_name(node: Node, source: &[u8]) -> String {
    match node.kind() {
        "identifier" => node_text(node, source).to_string(),
        "function_declarator" | "pointer_declarator" | "abstract_pointer_declarator" => {
            if let Some(inner) = node.child_by_field_name("declarator") {
                extract_declarator_name(inner, source)
            } else {
                String::new()
            }
        }
        _ => {
            // Try the `declarator` named field first, then walk children.
            if let Some(inner) = node.child_by_field_name("declarator") {
                let name = extract_declarator_name(inner, source);
                if !name.is_empty() {
                    return name;
                }
            }
            // Fallback: walk children looking for an identifier.
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let name = extract_declarator_name(child, source);
                if !name.is_empty() {
                    return name;
                }
            }
            String::new()
        }
    }
}

/// Return true if any direct child of `node` has the given kind string.
fn has_child_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).any(|c| c.kind() == kind);
    result
}

/// Walk the AST collecting C regions (pub so C++ can reuse it).
pub fn walk_node(node: Node, source: &[u8], regions: &mut Vec<ExtractedRegion>) {
    walk_node_with_parent(node, source, regions, None);
}

/// Walk the AST with an optional parent index for nested constructs.
pub fn walk_node_with_parent(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
) {
    match node.kind() {
        "function_definition" => {
            let name = node
                .child_by_field_name("declarator")
                .map(|d| extract_declarator_name(d, source))
                .unwrap_or_default();
            let kind = if parent_index.is_some() {
                RegionKind::Method
            } else {
                RegionKind::Function
            };
            let region = make_region(node, kind, name, source, parent_index);
            regions.push(region);
            // Don't recurse further — nested function definitions are not valid C.
        }
        "struct_specifier" => {
            // Only treat it as a region if it has a body (field_declaration_list).
            if has_child_kind(node, "field_declaration_list") {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source).to_string())
                    .unwrap_or_default();
                let region = make_region(node, RegionKind::Struct, name, source, parent_index);
                regions.push(region);
            } else {
                recurse_children(node, source, regions, parent_index);
            }
        }
        "enum_specifier" => {
            // Only treat it as a region if it has a body (enumerator_list).
            if has_child_kind(node, "enumerator_list") {
                let name = node
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source).to_string())
                    .unwrap_or_default();
                let region = make_region(node, RegionKind::Enum, name, source, parent_index);
                regions.push(region);
            } else {
                recurse_children(node, source, regions, parent_index);
            }
        }
        _ => {
            recurse_children(node, source, regions, parent_index);
        }
    }
}

fn recurse_children(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_node_with_parent(child, source, regions, parent_index);
    }
}

// ---------------------------------------------------------------------------
// Dependency extraction (pub so C++ can reuse)
// ---------------------------------------------------------------------------

/// Recursively walk AST collecting C dependencies (pub so C++ can reuse).
pub fn collect_dependencies(
    node: Node,
    source: &[u8],
    regions: &[ExtractedRegion],
    deps: &mut Vec<ExtractedDependency>,
) {
    match node.kind() {
        "preproc_include" => {
            extract_include_dependency(node, source, regions, deps);
            return; // no need to recurse into an include directive
        }
        "call_expression" => {
            extract_call_dependency(node, source, regions, deps);
            // Fall through to recurse — may have nested calls.
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_dependencies(child, source, regions, deps);
    }
}

/// Extract a `#include` dependency.
fn extract_include_dependency(
    node: Node,
    source: &[u8],
    regions: &[ExtractedRegion],
    deps: &mut Vec<ExtractedDependency>,
) {
    // The `path` child holds the include path (either a string_literal or
    // system_lib_string like `<stdio.h>`).
    let path_node = match node.child_by_field_name("path") {
        Some(n) => n,
        None => return,
    };

    let raw = node_text(path_node, source);
    // Strip surrounding `"..."` or `<...>`.
    let path = raw
        .trim_matches('"')
        .trim_start_matches('<')
        .trim_end_matches('>')
        .to_string();

    if path.is_empty() {
        return;
    }

    // Target symbol is the last path component (e.g. "stdio.h" from "sys/stdio.h").
    let target_symbol = path
        .split('/')
        .next_back()
        .unwrap_or(&path)
        .to_string();

    let source_region_index = find_enclosing_region(node, regions).unwrap_or(0);

    deps.push(ExtractedDependency {
        source_region_index,
        target_symbol,
        target_path: Some(path),
        kind: DependencyKind::Imports,
    });
}

/// Extract a function call dependency from a `call_expression` node.
fn extract_call_dependency(
    node: Node,
    source: &[u8],
    regions: &[ExtractedRegion],
    deps: &mut Vec<ExtractedDependency>,
) {
    let function_node = match node.child_by_field_name("function") {
        Some(n) => n,
        None => return,
    };

    let target_symbol = match function_node.kind() {
        "identifier" => node_text(function_node, source).to_string(),
        "field_expression" => {
            // e.g. `obj->method(...)` or `obj.method(...)`
            function_node
                .child_by_field_name("field")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default()
        }
        _ => return,
    };

    if target_symbol.is_empty() {
        return;
    }

    let source_region_index = find_enclosing_region(node, regions).unwrap_or(0);

    deps.push(ExtractedDependency {
        source_region_index,
        target_symbol,
        target_path: None,
        kind: DependencyKind::Calls,
    });
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
        let tree = parse_source(&CPlugin, &bytes).expect("parse failed");
        (tree, bytes)
    }

    #[test]
    fn extracts_c_functions() {
        let src = r#"
int add(int a, int b) {
    return a + b;
}

void greet(const char* name) {
    printf("Hello, %s\n", name);
}
"#;
        let (tree, bytes) = parse(src);
        let regions = CPlugin.extract_regions(&tree, &bytes);

        let fns: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Function)
            .collect();

        assert_eq!(fns.len(), 2, "expected 2 functions, got {}", fns.len());
        let names: Vec<&str> = fns.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"add"), "missing 'add', got {:?}", names);
        assert!(names.contains(&"greet"), "missing 'greet', got {:?}", names);
    }

    #[test]
    fn extracts_c_structs() {
        let src = r#"
struct Point {
    int x;
    int y;
};

struct Color {
    unsigned char r;
    unsigned char g;
    unsigned char b;
};
"#;
        let (tree, bytes) = parse(src);
        let regions = CPlugin.extract_regions(&tree, &bytes);

        let structs: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Struct)
            .collect();

        assert_eq!(structs.len(), 2, "expected 2 structs, got {}", structs.len());
        let names: Vec<&str> = structs.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"Point"), "missing 'Point'");
        assert!(names.contains(&"Color"), "missing 'Color'");
    }

    #[test]
    fn extracts_c_enums() {
        let src = r#"
enum Direction {
    NORTH,
    SOUTH,
    EAST,
    WEST
};
"#;
        let (tree, bytes) = parse(src);
        let regions = CPlugin.extract_regions(&tree, &bytes);

        let enums: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Enum)
            .collect();

        assert_eq!(enums.len(), 1, "expected 1 enum, got {}", enums.len());
        assert_eq!(enums[0].name, "Direction");
    }

    #[test]
    fn extracts_c_includes() {
        let src = r#"
#include <stdio.h>
#include <stdlib.h>
#include "myheader.h"

int main() {
    return 0;
}
"#;
        let (tree, bytes) = parse(src);
        let regions = CPlugin.extract_regions(&tree, &bytes);
        let deps = CPlugin.extract_dependencies(&tree, &bytes, &regions);

        let imports: Vec<_> = deps
            .iter()
            .filter(|d| d.kind == DependencyKind::Imports)
            .collect();

        assert!(
            imports.len() >= 3,
            "expected at least 3 imports, got {}",
            imports.len()
        );

        let paths: Vec<&str> = imports
            .iter()
            .filter_map(|d| d.target_path.as_deref())
            .collect();
        assert!(paths.contains(&"stdio.h"), "missing stdio.h, got {:?}", paths);
        assert!(paths.contains(&"stdlib.h"), "missing stdlib.h");
        assert!(paths.contains(&"myheader.h"), "missing myheader.h");
    }
}
