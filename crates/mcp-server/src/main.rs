mod tools;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use tools::McpServer;

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    // Determine project root: use first argument or CWD.
    let project_root: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().expect("cannot determine cwd"));

    let server = McpServer::new(project_root);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = handle_line(&server, &line);

        let serialized = serde_json::to_string(&response)?;
        out.write_all(serialized.as_bytes())?;
        out.write_all(b"\n")?;
        out.flush()?;
    }

    Ok(())
}

fn handle_line(server: &McpServer, line: &str) -> JsonRpcResponse {
    // Parse the JSON-RPC request.
    let req: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            return JsonRpcResponse::error(
                Value::Null,
                -32700,
                format!("Parse error: {}", e),
            );
        }
    };

    let id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => handle_initialize(id),
        "tools/list" => handle_tools_list(server, id),
        "tools/call" => handle_tools_call(server, id, &req.params),
        _ => JsonRpcResponse::error(id, -32601, format!("Method not found: {}", req.method)),
    }
}

// ---------------------------------------------------------------------------
// Method handlers
// ---------------------------------------------------------------------------

fn handle_initialize(id: Value) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "codeindex",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "tools": {}
            }
        }),
    )
}

fn handle_tools_list(server: &McpServer, id: Value) -> JsonRpcResponse {
    JsonRpcResponse::success(id, server.handle_tools_list())
}

fn handle_tools_call(server: &McpServer, id: Value, params: &Value) -> JsonRpcResponse {
    let tool_name = match params["name"].as_str() {
        Some(n) => n,
        None => {
            return JsonRpcResponse::error(id, -32602, "Missing 'name' in tools/call params");
        }
    };

    let args = &params["arguments"];

    let result = match tool_name {
        "codeindex_query" => server.call_query(args),
        "codeindex_deps" => server.call_deps(args),
        "codeindex_status" => server.call_status(),
        _ => {
            return JsonRpcResponse::error(
                id,
                -32601,
                format!("Unknown tool: {}", tool_name),
            );
        }
    };

    match result {
        Ok(value) => JsonRpcResponse::success(
            id,
            json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string_pretty(&value).unwrap_or_default()
                    }
                ]
            }),
        ),
        Err(e) => JsonRpcResponse::error(id, -32603, format!("Tool error: {}", e)),
    }
}
