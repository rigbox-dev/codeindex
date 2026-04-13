use tree_sitter::{Language, Node, Tree};

use crate::{
    find_enclosing_region, DependencyKind, ExtractedDependency, ExtractedRegion, LanguagePlugin,
    RegionKind,
};

pub struct PythonPlugin;

impl LanguagePlugin for PythonPlugin {
    fn language_id(&self) -> &str {
        "python"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".py"]
    }

    fn tree_sitter_language(&self) -> Language {
        tree_sitter_python::LANGUAGE.into()
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

/// Return the first line of a node, stripped of trailing ':'.
fn signature_of(node: Node, source: &[u8]) -> String {
    let full = std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("");
    let first_line = full.lines().next().unwrap_or("").trim_end_matches(':').trim().to_string();
    first_line
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

/// Recursively walk the AST, collecting regions.
///
/// `inside_class` is true when we are directly inside a class body, so that
/// `function_definition` nodes are treated as Methods.
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
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let kind = if inside_class {
                RegionKind::Method
            } else {
                RegionKind::Function
            };
            let region = make_region(node, kind, name, source, parent_index);
            regions.push(region);
            // Don't recurse further (nested functions inside methods are uncommon)
        }
        "class_definition" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let class_index = regions.len();
            let region = make_region(node, RegionKind::Class, name, source, parent_index);
            regions.push(region);
            // Recurse into the class body
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "block" {
                    walk_class_body(child, source, regions, Some(class_index));
                }
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(child, source, regions, parent_index, inside_class);
            }
        }
    }
}

/// Walk the body of a class, extracting function_definition nodes as Methods.
fn walk_class_body(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_definition" {
            let name = child
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let region = make_region(child, RegionKind::Method, name, source, parent_index);
            regions.push(region);
        }
        // Ignore decorated_definition wrapping — handle it specially
        if child.kind() == "decorated_definition" {
            // Find the inner function_definition
            let mut c2 = child.walk();
            for sub in child.children(&mut c2) {
                if sub.kind() == "function_definition" {
                    let name = sub
                        .child_by_field_name("name")
                        .map(|n| node_text(n, source).to_string())
                        .unwrap_or_default();
                    let region = make_region(sub, RegionKind::Method, name, source, parent_index);
                    regions.push(region);
                }
            }
        }
    }
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
        "import_from_statement" => {
            extract_import_from_dependency(node, source, regions, deps);
            return;
        }
        "import_statement" => {
            extract_import_dependency(node, source, regions, deps);
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_dependencies(child, source, regions, deps);
    }
}

/// Extract `from X import Y, Z` dependencies.
fn extract_import_from_dependency(
    node: Node,
    source: &[u8],
    regions: &[ExtractedRegion],
    deps: &mut Vec<ExtractedDependency>,
) {
    // module_name is the module being imported from
    let module_name = node
        .child_by_field_name("module_name")
        .map(|n| node_text(n, source).to_string());

    let source_region_index = find_enclosing_region(node, regions).unwrap_or(0);

    // Collect imported names
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" | "aliased_import" => {
                // These appear in: `from module import name` or `from module import name as alias`
                let name = if child.kind() == "aliased_import" {
                    child
                        .child_by_field_name("name")
                        .map(|n| node_text(n, source).to_string())
                        .unwrap_or_default()
                } else {
                    node_text(child, source).to_string()
                };
                if !name.is_empty() && child.start_byte() > node.start_byte() {
                    // Skip the module_name node itself — only emit imported names
                    // Check this isn't the module_name field
                    let is_module = node
                        .child_by_field_name("module_name")
                        .map(|n| n.start_byte() == child.start_byte())
                        .unwrap_or(false);
                    if !is_module {
                        deps.push(ExtractedDependency {
                            source_region_index,
                            target_symbol: name,
                            target_path: module_name.clone(),
                            kind: DependencyKind::Imports,
                        });
                    }
                }
            }
            "identifier" => {
                // Simple: from module import Name
                let name = node_text(child, source).to_string();
                if name == "import" || name == "from" {
                    continue;
                }
                let is_module = node
                    .child_by_field_name("module_name")
                    .map(|n| n.start_byte() == child.start_byte())
                    .unwrap_or(false);
                if !is_module && !name.is_empty() {
                    deps.push(ExtractedDependency {
                        source_region_index,
                        target_symbol: name,
                        target_path: module_name.clone(),
                        kind: DependencyKind::Imports,
                    });
                }
            }
            _ => {}
        }
    }
}

/// Extract `import X` dependencies.
fn extract_import_dependency(
    node: Node,
    source: &[u8],
    regions: &[ExtractedRegion],
    deps: &mut Vec<ExtractedDependency>,
) {
    let source_region_index = find_enclosing_region(node, regions).unwrap_or(0);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "dotted_name" => {
                let name = node_text(child, source).to_string();
                deps.push(ExtractedDependency {
                    source_region_index,
                    target_symbol: name.clone(),
                    target_path: Some(name),
                    kind: DependencyKind::Imports,
                });
            }
            "aliased_import" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source).to_string())
                    .unwrap_or_default();
                if !name.is_empty() {
                    deps.push(ExtractedDependency {
                        source_region_index,
                        target_symbol: name.clone(),
                        target_path: Some(name),
                        kind: DependencyKind::Imports,
                    });
                }
            }
            _ => {}
        }
    }
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
        let tree = parse_source(&PythonPlugin, &bytes).expect("parse failed");
        (tree, bytes)
    }

    #[test]
    fn extracts_python_classes_and_methods() {
        let src = r#"
class AuthHandler:
    def __init__(self, db):
        self.db = db

    def authenticate(self, req):
        return None
"#;
        let (tree, bytes) = parse(src);
        let regions = PythonPlugin.extract_regions(&tree, &bytes);

        let classes: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Class)
            .collect();
        let methods: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Method)
            .collect();

        assert_eq!(classes.len(), 1, "expected 1 class, got {}", classes.len());
        assert_eq!(classes[0].name, "AuthHandler");

        assert_eq!(methods.len(), 2, "expected 2 methods, got {}", methods.len());
        let method_names: Vec<&str> = methods.iter().map(|r| r.name.as_str()).collect();
        assert!(method_names.contains(&"__init__"), "missing __init__");
        assert!(method_names.contains(&"authenticate"), "missing authenticate");

        // Methods should point to the class
        let class_index = regions
            .iter()
            .position(|r| r.kind == RegionKind::Class)
            .expect("class not found");
        for m in &methods {
            assert_eq!(
                m.parent_index,
                Some(class_index),
                "method '{}' should have class as parent",
                m.name
            );
        }
    }

    #[test]
    fn extracts_python_functions() {
        let src = r#"
def verify_password(password: str, hash: str) -> bool:
    return password == hash

def helper():
    pass
"#;
        let (tree, bytes) = parse(src);
        let regions = PythonPlugin.extract_regions(&tree, &bytes);

        let fns: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Function)
            .collect();

        assert_eq!(fns.len(), 2, "expected 2 functions, got {}", fns.len());
        let names: Vec<&str> = fns.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"verify_password"), "missing verify_password");
        assert!(names.contains(&"helper"), "missing helper");

        // Signature should be the first line, stripped of ':'
        let vp = fns.iter().find(|r| r.name == "verify_password").unwrap();
        assert!(
            vp.signature.contains("verify_password"),
            "unexpected signature: {}",
            vp.signature
        );
        assert!(
            !vp.signature.ends_with(':'),
            "signature should not end with ':', got: {}",
            vp.signature
        );
    }
}
