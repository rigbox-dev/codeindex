pub mod rust_lang;

use anyhow::Result;
use codeindex_core::model::{DependencyKind, RegionKind};
use tree_sitter::{Language, Tree};

pub use rust_lang::RustPlugin;

/// A region extracted from a source file during AST analysis.
pub struct ExtractedRegion {
    pub kind: RegionKind,
    pub name: String,
    pub start_line: u32,      // 1-based
    pub end_line: u32,        // 1-based
    pub start_byte: u32,
    pub end_byte: u32,
    pub signature: String,
    pub parent_index: Option<usize>, // index into Vec<ExtractedRegion>
}

/// A dependency extracted from a source file.
pub struct ExtractedDependency {
    pub source_region_index: usize,
    pub target_symbol: String,
    pub target_path: Option<String>,
    pub kind: DependencyKind,
}

/// Trait that each language plugin must implement.
pub trait LanguagePlugin: Send + Sync {
    fn language_id(&self) -> &str;
    fn file_extensions(&self) -> &[&str];
    fn tree_sitter_language(&self) -> Language;
    fn extract_regions(&self, tree: &Tree, source: &[u8]) -> Vec<ExtractedRegion>;
    fn extract_dependencies(
        &self,
        tree: &Tree,
        source: &[u8],
        regions: &[ExtractedRegion],
    ) -> Vec<ExtractedDependency>;
}

/// Parse source bytes using the given plugin's language.
pub fn parse_source(plugin: &dyn LanguagePlugin, source: &[u8]) -> Result<Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&plugin.tree_sitter_language())?;
    parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter failed to parse source"))
}

/// Find the index of the smallest enclosing region (by byte range) for a given node.
pub fn find_enclosing_region(
    node: tree_sitter::Node,
    regions: &[ExtractedRegion],
) -> Option<usize> {
    let node_start = node.start_byte() as u32;
    let node_end = node.end_byte() as u32;

    let mut best: Option<usize> = None;
    let mut best_span = u32::MAX;

    for (i, region) in regions.iter().enumerate() {
        if region.start_byte <= node_start && node_end <= region.end_byte {
            let span = region.end_byte - region.start_byte;
            if span < best_span {
                best_span = span;
                best = Some(i);
            }
        }
    }
    best
}

/// Registry that maps file extensions and language IDs to plugins.
pub struct LanguageRegistry {
    plugins: Vec<Box<dyn LanguagePlugin>>,
}

impl LanguageRegistry {
    /// Creates a new registry pre-registered with the built-in Rust plugin.
    pub fn new() -> Self {
        let mut registry = Self {
            plugins: Vec::new(),
        };
        registry.register(Box::new(RustPlugin));
        registry
    }

    /// Register an additional language plugin.
    pub fn register(&mut self, plugin: Box<dyn LanguagePlugin>) {
        self.plugins.push(plugin);
    }

    /// Return the plugin that handles the given file extension (e.g. ".rs").
    pub fn plugin_for_extension(&self, ext: &str) -> Option<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .find(|p| p.file_extensions().contains(&ext))
            .map(|p| p.as_ref())
    }

    /// Return the plugin for the given language id (e.g. "rust").
    pub fn plugin_for_language(&self, lang: &str) -> Option<&dyn LanguagePlugin> {
        self.plugins
            .iter()
            .find(|p| p.language_id() == lang)
            .map(|p| p.as_ref())
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}
