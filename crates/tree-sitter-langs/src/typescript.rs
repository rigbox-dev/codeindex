use tree_sitter::{Language, Node, Tree};

use crate::{
    find_enclosing_region, DependencyKind, ExtractedDependency, ExtractedRegion, LanguagePlugin,
    RegionKind,
};

pub struct TypeScriptPlugin;

impl LanguagePlugin for TypeScriptPlugin {
    fn language_id(&self) -> &str {
        "typescript"
    }

    fn file_extensions(&self) -> &[&str] {
        &[".ts", ".tsx", ".js", ".jsx"]
    }

    fn tree_sitter_language(&self) -> Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }

    fn extract_regions(&self, tree: &Tree, source: &[u8]) -> Vec<ExtractedRegion> {
        let mut regions = Vec::new();
        walk_node(tree.root_node(), source, &mut regions, None);
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

/// Extract the signature of a node: text up to (but not including) the body block.
fn signature_of(node: Node, source: &[u8]) -> String {
    let body_kinds = ["statement_block", "class_body"];

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
fn walk_node(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
) {
    match node.kind() {
        "function_declaration" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let region = make_region(node, RegionKind::Function, name, source, parent_index);
            regions.push(region);
            // Don't recurse further — no nested named functions expected at top level
        }
        "class_declaration" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let class_index = regions.len();
            let region = make_region(node, RegionKind::Class, name, source, parent_index);
            regions.push(region);
            // Recurse into class_body for method_definition nodes
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "class_body" {
                    walk_class_body(child, source, regions, Some(class_index));
                }
            }
        }
        "interface_declaration" => {
            let name = node
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let region = make_region(node, RegionKind::Interface, name, source, parent_index);
            regions.push(region);
            // Don't recurse into interface body (method signatures, not bodies)
        }
        "lexical_declaration" | "variable_declaration" => {
            // Look for arrow functions assigned to variables: `const foo = () => ...`
            extract_named_arrow_functions(node, source, regions, parent_index);
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                walk_node(child, source, regions, parent_index);
            }
        }
    }
}

/// Walk the body of a class, extracting method_definition nodes.
fn walk_class_body(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "method_definition" {
            let name = child
                .child_by_field_name("name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();
            let region = make_region(child, RegionKind::Method, name, source, parent_index);
            regions.push(region);
        }
    }
}

/// Look for named arrow functions in a variable declaration:
/// `const foo = (x) => x + 1;`
fn extract_named_arrow_functions(
    node: Node,
    source: &[u8],
    regions: &mut Vec<ExtractedRegion>,
    parent_index: Option<usize>,
) {
    // Expect: lexical_declaration → variable_declarator (name, value)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            let name_node = child.child_by_field_name("name");
            let value_node = child.child_by_field_name("value");
            if let (Some(name_n), Some(value_n)) = (name_node, value_node) {
                if value_n.kind() == "arrow_function" {
                    let name = node_text(name_n, source).to_string();
                    if !name.is_empty() {
                        let region =
                            make_region(value_n, RegionKind::Function, name, source, parent_index);
                        regions.push(region);
                    }
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
    if node.kind() == "import_statement" {
        extract_import_dependency(node, source, regions, deps);
        return; // Don't recurse into import statements
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_dependencies(child, source, regions, deps);
    }
}

/// Extract import dependencies from an `import_statement` node.
///
/// Handles:
///   import { Foo, Bar } from './module';
///   import Foo from './module';
fn extract_import_dependency(
    node: Node,
    source: &[u8],
    regions: &[ExtractedRegion],
    deps: &mut Vec<ExtractedDependency>,
) {
    // Find the source (string) node for target_path
    let target_path = {
        let mut path = None;
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "string" {
                let text = node_text(child, source).trim_matches('"').trim_matches('\'').to_string();
                path = Some(text);
                break;
            }
        }
        path
    };

    let source_region_index = find_enclosing_region(node, regions).unwrap_or(0);

    // Collect named imports from import_specifier nodes
    let mut found_any = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "import_clause" {
            // May have named_imports or a default identifier
            let mut c2 = child.walk();
            for sub in child.children(&mut c2) {
                match sub.kind() {
                    "identifier" => {
                        // default import: import Foo from '...'
                        let name = node_text(sub, source).to_string();
                        if !name.is_empty() {
                            deps.push(ExtractedDependency {
                                source_region_index,
                                target_symbol: name,
                                target_path: target_path.clone(),
                                kind: DependencyKind::Imports,
                            });
                            found_any = true;
                        }
                    }
                    "named_imports" => {
                        let mut c3 = sub.walk();
                        for spec in sub.children(&mut c3) {
                            if spec.kind() == "import_specifier" {
                                // The first identifier child is the imported name
                                let name = spec
                                    .child_by_field_name("name")
                                    .map(|n| node_text(n, source).to_string())
                                    .unwrap_or_default();
                                if !name.is_empty() {
                                    deps.push(ExtractedDependency {
                                        source_region_index,
                                        target_symbol: name,
                                        target_path: target_path.clone(),
                                        kind: DependencyKind::Imports,
                                    });
                                    found_any = true;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // If no named imports were found but we have a path, emit the path itself
    if !found_any {
        if let Some(path) = &target_path {
            deps.push(ExtractedDependency {
                source_region_index,
                target_symbol: path.clone(),
                target_path: target_path.clone(),
                kind: DependencyKind::Imports,
            });
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
        let tree = parse_source(&TypeScriptPlugin, &bytes).expect("parse failed");
        (tree, bytes)
    }

    #[test]
    fn extracts_ts_interfaces() {
        let src = r#"
export interface AuthRequest {
  username: string;
  password: string;
}

export interface AuthToken {
  token: string;
  expiresAt: number;
}
"#;
        let (tree, bytes) = parse(src);
        let regions = TypeScriptPlugin.extract_regions(&tree, &bytes);

        let interfaces: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Interface)
            .collect();

        assert_eq!(interfaces.len(), 2, "expected 2 interfaces, got {}", interfaces.len());
        let names: Vec<&str> = interfaces.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"AuthRequest"), "missing AuthRequest interface");
        assert!(names.contains(&"AuthToken"), "missing AuthToken interface");
    }

    #[test]
    fn extracts_ts_classes_and_methods() {
        let src = r#"
export class AuthHandler {
  constructor(private db: any) {}

  async authenticate(req: any): Promise<any> {
    return null;
  }
}
"#;
        let (tree, bytes) = parse(src);
        let regions = TypeScriptPlugin.extract_regions(&tree, &bytes);

        let classes: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Class)
            .collect();
        let methods: Vec<_> = regions
            .iter()
            .filter(|r| r.kind == RegionKind::Method)
            .collect();

        assert_eq!(classes.len(), 1, "expected 1 class");
        assert_eq!(classes[0].name, "AuthHandler");

        assert!(!methods.is_empty(), "expected at least one method");
        let method_names: Vec<&str> = methods.iter().map(|r| r.name.as_str()).collect();
        assert!(
            method_names.contains(&"authenticate"),
            "expected 'authenticate' method, got {:?}",
            method_names
        );

        // Methods should have parent_index pointing to the class
        let class_index = regions
            .iter()
            .position(|r| r.kind == RegionKind::Class)
            .expect("class not found");
        for m in &methods {
            assert_eq!(
                m.parent_index,
                Some(class_index),
                "method '{}' should have parent_index pointing to class",
                m.name
            );
        }
    }

    #[test]
    fn extracts_ts_imports() {
        let src = r#"
import { AuthRequest, AuthToken, AuthError } from './types';
import { Database } from '../db';
"#;
        let (tree, bytes) = parse(src);
        let regions = TypeScriptPlugin.extract_regions(&tree, &bytes);
        let deps = TypeScriptPlugin.extract_dependencies(&tree, &bytes, &regions);

        let imports: Vec<_> = deps
            .iter()
            .filter(|d| d.kind == DependencyKind::Imports)
            .collect();

        assert!(
            imports.len() >= 4,
            "expected at least 4 import deps, got {}",
            imports.len()
        );

        let symbols: Vec<&str> = imports.iter().map(|d| d.target_symbol.as_str()).collect();
        assert!(symbols.contains(&"AuthRequest"), "missing AuthRequest");
        assert!(symbols.contains(&"AuthToken"), "missing AuthToken");
        assert!(symbols.contains(&"AuthError"), "missing AuthError");
        assert!(symbols.contains(&"Database"), "missing Database");
    }
}
