use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;

use codeindex_core::{
    config::Config,
    embedding::MockEmbeddingProvider,
    query::{QueryEngine, QueryOptions},
    storage::{sqlite::SqliteStorage, vectors::VectorIndex},
};

/// MCP server that serves codeindex tools.
pub struct McpServer {
    pub project_root: PathBuf,
}

impl McpServer {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Handle `tools/list` — return schemas for all 3 tools.
    pub fn handle_tools_list(&self) -> Value {
        json!({
            "tools": [
                {
                    "name": "codeindex_query",
                    "description": "Search the codeindex for code regions matching a natural language query or symbol/file/dependency lookup.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The search query. Supports natural language, :symbol <name>, :file <path>, and :deps <file>::<symbol> prefixes."
                            },
                            "top": {
                                "type": "number",
                                "description": "Maximum number of results to return (default: 5)."
                            },
                            "depth": {
                                "type": "number",
                                "description": "Dependency expansion depth (default: 1)."
                            },
                            "enhance": {
                                "type": "boolean",
                                "description": "Whether to enhance results with LLM summaries."
                            },
                            "include_code": {
                                "type": "boolean",
                                "description": "Whether to include raw source code in results."
                            }
                        },
                        "required": ["query"]
                    }
                },
                {
                    "name": "codeindex_deps",
                    "description": "Look up dependencies for a specific symbol in a file.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "file": {
                                "type": "string",
                                "description": "Relative path to the source file."
                            },
                            "symbol": {
                                "type": "string",
                                "description": "Name of the symbol (function, struct, etc.) to look up."
                            },
                            "depth": {
                                "type": "number",
                                "description": "Dependency expansion depth (default: 2)."
                            },
                            "direction": {
                                "type": "string",
                                "enum": ["outgoing", "incoming", "both"],
                                "description": "Which direction of dependencies to return."
                            }
                        },
                        "required": ["file", "symbol"]
                    }
                },
                {
                    "name": "codeindex_status",
                    "description": "Check the status of the codeindex for the current project.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                }
            ]
        })
    }

    /// Handle `codeindex_query` tool call.
    pub fn call_query(&self, args: &Value) -> Result<Value> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("'query' argument is required"))?
            .to_string();

        let top = args["top"].as_u64().unwrap_or(5) as usize;
        let depth = args["depth"].as_u64().unwrap_or(1) as usize;
        let include_code = args["include_code"].as_bool().unwrap_or(false);

        let (storage, vectors, provider) = self.open_index()?;

        let engine = QueryEngine::new(&storage, &vectors, &provider, &self.project_root);
        let opts = QueryOptions {
            top,
            depth,
            include_code,
        };

        let response = engine.query(&query, &opts)?;
        Ok(serde_json::to_value(&response)?)
    }

    /// Handle `codeindex_deps` tool call.
    pub fn call_deps(&self, args: &Value) -> Result<Value> {
        let file = args["file"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("'file' argument is required"))?
            .to_string();
        let symbol = args["symbol"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("'symbol' argument is required"))?
            .to_string();
        let depth = args["depth"].as_u64().unwrap_or(2) as usize;

        // Format as a dependency query: :deps file::symbol
        let query = format!(":deps {}::{}", file, symbol);

        let (storage, vectors, provider) = self.open_index()?;

        let engine = QueryEngine::new(&storage, &vectors, &provider, &self.project_root);
        let opts = QueryOptions {
            top: 20,
            depth,
            include_code: false,
        };

        let response = engine.query(&query, &opts)?;
        Ok(serde_json::to_value(&response)?)
    }

    /// Handle `codeindex_status` tool call.
    pub fn call_status(&self) -> Result<Value> {
        let config = Config::load(&self.project_root).unwrap_or_default();
        let index_dir = config.index_dir(&self.project_root);
        let db_path = index_dir.join("index.db");

        if !db_path.exists() {
            return Ok(json!({
                "indexed": false,
                "project_root": self.project_root.display().to_string(),
                "message": "No index found. Run `codeindex index` to create one."
            }));
        }

        // Open the DB and count files/regions.
        match SqliteStorage::open(&db_path) {
            Ok(storage) => {
                let file_count: i64 = storage
                    .conn()
                    .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
                    .unwrap_or(0);
                let region_count: i64 = storage
                    .conn()
                    .query_row("SELECT COUNT(*) FROM regions", [], |row| row.get(0))
                    .unwrap_or(0);

                Ok(json!({
                    "indexed": true,
                    "project_root": self.project_root.display().to_string(),
                    "file_count": file_count,
                    "region_count": region_count,
                    "index_path": db_path.display().to_string()
                }))
            }
            Err(e) => Ok(json!({
                "indexed": false,
                "project_root": self.project_root.display().to_string(),
                "error": e.to_string()
            })),
        }
    }

    // -------------------------------------------------------------------------
    // Helpers
    // -------------------------------------------------------------------------

    fn open_index(&self) -> Result<(SqliteStorage, VectorIndex, MockEmbeddingProvider)> {
        let config = Config::load(&self.project_root).unwrap_or_default();
        let index_dir = config.index_dir(&self.project_root);
        let db_path = index_dir.join("index.db");

        if !db_path.exists() {
            anyhow::bail!(
                "No index found at {}. Run `codeindex index` to create one.",
                db_path.display()
            );
        }

        let storage = SqliteStorage::open(&db_path)?;

        // Use a fixed dimension for mock embeddings (matches what the indexer uses).
        // In a real deployment this would load a real embedding provider.
        let dimension = 64;
        let vectors = VectorIndex::load(&index_dir, dimension)?;
        let provider = MockEmbeddingProvider::new(dimension);

        Ok((storage, vectors, provider))
    }
}
