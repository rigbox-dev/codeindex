use axum::extract::{State, Query, Path};
use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;
use std::fs;
use crate::state::SharedState;
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_core::storage::vectors::VectorIndex;
use codeindex_core::embedding::MockEmbeddingProvider;
use codeindex_core::query::{QueryEngine, QueryOptions};
use codeindex_core::model::{ActivityEntry, QueryResponse};
use codeindex_core::config::Config;

#[derive(serde::Deserialize)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_top")]
    pub top: usize,
    #[serde(default = "default_depth")]
    pub depth: usize,
}
fn default_top() -> usize { 10 }
fn default_depth() -> usize { 1 }

pub async fn search(
    State(state): State<SharedState>,
    Query(params): Query<SearchParams>,
) -> Json<QueryResponse> {
    let empty = QueryResponse {
        query: params.q.clone(),
        results: Vec::new(),
        graph_context: None,
    };

    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(empty),
    };

    let provider = MockEmbeddingProvider::new(384);
    let vectors = VectorIndex::load(&state.index_dir, 384).unwrap_or_else(|_| VectorIndex::new(384));

    let engine = QueryEngine::new(&storage, &vectors, &provider, &state.project_root);
    let opts = QueryOptions {
        top: params.top,
        depth: params.depth,
        include_code: true,
    };

    match engine.query(&params.q, &opts) {
        Ok(response) => Json(response),
        Err(_) => Json(empty),
    }
}

#[derive(Serialize)]
#[derive(Default)]
pub struct StatsResponse {
    pub total_files: i64,
    pub total_regions: i64,
    pub total_dependencies: i64,
    pub languages: Vec<LabelCount>,
    pub region_kinds: Vec<LabelCount>,
    pub dep_kinds: Vec<LabelCount>,
    pub db_size_bytes: u64,
    pub vector_size_bytes: u64,
    pub last_indexed_at: Option<i64>,
}


#[derive(Serialize)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

pub async fn stats(State(state): State<SharedState>) -> Json<StatsResponse> {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(StatsResponse::default()),
    };
    let conn = storage.conn();

    // Total files
    let total_files: i64 = conn
        .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
        .unwrap_or(0);

    // Total regions
    let total_regions: i64 = conn
        .query_row("SELECT COUNT(*) FROM regions", [], |row| row.get(0))
        .unwrap_or(0);

    // Total dependencies
    let total_dependencies: i64 = conn
        .query_row("SELECT COUNT(*) FROM dependencies", [], |row| row.get(0))
        .unwrap_or(0);

    // Language breakdown
    let languages = {
        let mut stmt = conn
            .prepare(
                "SELECT language, COUNT(*) as cnt FROM files \
                 GROUP BY language ORDER BY cnt DESC LIMIT 20",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok(LabelCount {
                label: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>()
    };

    // Region kind breakdown
    let region_kinds = {
        let mut stmt = conn
            .prepare(
                "SELECT kind, COUNT(*) as cnt FROM regions \
                 GROUP BY kind ORDER BY cnt DESC",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok(LabelCount {
                label: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>()
    };

    // Dependency kind breakdown
    let dep_kinds = {
        let mut stmt = conn
            .prepare(
                "SELECT kind, COUNT(*) as cnt FROM dependencies \
                 GROUP BY kind ORDER BY cnt DESC",
            )
            .unwrap();
        stmt.query_map([], |row| {
            Ok(LabelCount {
                label: row.get(0)?,
                count: row.get(1)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>()
    };

    // Last indexed timestamp
    let last_indexed_at: Option<i64> = conn
        .query_row(
            "SELECT MAX(last_indexed_at) FROM files",
            [],
            |row| row.get(0),
        )
        .unwrap_or(None);

    // DB file size
    let db_size_bytes = fs::metadata(&state.db_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // Vector store size (walk the index_dir for .bin / .idx files)
    let vector_size_bytes = fs::read_dir(&state.index_dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name();
                    let s = name.to_string_lossy();
                    s.ends_with(".bin") || s.ends_with(".idx") || s.ends_with(".vec")
                })
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum::<u64>()
        })
        .unwrap_or(0);

    Json(StatsResponse {
        total_files,
        total_regions,
        total_dependencies,
        languages,
        region_kinds,
        dep_kinds,
        db_size_bytes,
        vector_size_bytes,
        last_indexed_at,
    })
}

// ── Dependency Graph API ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<serde_json::Value>,
    pub edges: Vec<serde_json::Value>,
}

#[derive(serde::Deserialize)]
pub struct GraphParams {
    #[serde(default = "default_graph_limit")]
    pub limit: usize,
}
fn default_graph_limit() -> usize { 200 }

#[derive(Serialize)]
pub struct NodeDetail {
    pub region_id: i64,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub lines: [u32; 2],
    pub signature: String,
    pub outgoing: Vec<DepEdge>,
    pub incoming: Vec<DepEdge>,
}

#[derive(Serialize)]
pub struct DepEdge {
    pub name: String,
    pub file: String,
    pub kind: String,
}

pub async fn graph_data(
    State(state): State<SharedState>,
    Query(params): Query<GraphParams>,
) -> impl IntoResponse {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Json(GraphData { nodes: vec![], edges: vec![] }).into_response(),
    };

    let files = match storage.list_all_files() {
        Ok(f) => f,
        Err(_) => return Json(GraphData { nodes: vec![], edges: vec![] }).into_response(),
    };

    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let mut edges: Vec<serde_json::Value> = Vec::new();
    let mut region_count = 0usize;

    for file in &files {
        if region_count >= params.limit {
            break;
        }

        let regions = match storage.get_regions_for_file(file.file_id) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if regions.is_empty() {
            continue;
        }

        // Short path label for file compound node
        let short_path = file.path.rfind('/').map(|p| &file.path[p+1..]).unwrap_or(&file.path);
        nodes.push(serde_json::json!({
            "data": {
                "id": format!("f_{}", file.file_id),
                "label": short_path,
            }
        }));

        for region in &regions {
            if region_count >= params.limit {
                break;
            }
            nodes.push(serde_json::json!({
                "data": {
                    "id": format!("r_{}", region.region_id),
                    "label": region.name,
                    "kind": region.kind.as_str(),
                    "file": file.path,
                    "parent": format!("f_{}", file.file_id),
                }
            }));
            region_count += 1;

            // Collect edges for this region
            let deps = match storage.get_dependencies_from(region.region_id) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for dep in deps {
                if let Some(target_id) = dep.target_region_id {
                    edges.push(serde_json::json!({
                        "data": {
                            "source": format!("r_{}", region.region_id),
                            "target": format!("r_{}", target_id),
                            "kind": dep.kind.as_str(),
                        }
                    }));
                }
            }
        }
    }

    Json(GraphData { nodes, edges }).into_response()
}

pub async fn graph_node_detail(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let region = match storage.get_region(id) {
        Ok(Some(r)) => r,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let file = match storage.get_file(region.file_id) {
        Ok(Some(f)) => f,
        _ => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // Outgoing deps
    let outgoing_deps = storage.get_dependencies_from(id).unwrap_or_default();
    let mut outgoing: Vec<DepEdge> = Vec::new();
    for dep in outgoing_deps {
        if let Some(target_id) = dep.target_region_id {
            if let Ok(Some(target_region)) = storage.get_region(target_id) {
                let target_file = storage.get_file(target_region.file_id)
                    .ok().flatten()
                    .map(|f| f.path)
                    .unwrap_or_default();
                outgoing.push(DepEdge {
                    name: target_region.name,
                    file: target_file,
                    kind: dep.kind.as_str().to_string(),
                });
            }
        }
    }

    // Incoming deps
    let incoming_deps = storage.get_dependencies_to(id).unwrap_or_default();
    let mut incoming: Vec<DepEdge> = Vec::new();
    for dep in incoming_deps {
        if let Ok(Some(src_region)) = storage.get_region(dep.source_region_id) {
            let src_file = storage.get_file(src_region.file_id)
                .ok().flatten()
                .map(|f| f.path)
                .unwrap_or_default();
            incoming.push(DepEdge {
                name: src_region.name,
                file: src_file,
                kind: dep.kind.as_str().to_string(),
            });
        }
    }

    let detail = NodeDetail {
        region_id: region.region_id,
        name: region.name,
        kind: region.kind.as_str().to_string(),
        file: file.path,
        lines: [region.start_line, region.end_line],
        signature: region.signature,
        outgoing,
        incoming,
    };

    Json(detail).into_response()
}

// ── File Explorer API ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct FileInfo {
    pub file_id: i64,
    pub path: String,
    pub language: String,
    pub region_count: i64,
    pub last_indexed_at: i64,
}

#[derive(Serialize)]
pub struct FileDetail {
    pub file_id: i64,
    pub path: String,
    pub language: String,
    pub last_indexed_at: i64,
    pub regions: Vec<RegionInfo>,
}

#[derive(Serialize)]
pub struct RegionInfo {
    pub region_id: i64,
    pub kind: String,
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub signature: String,
}

#[derive(Serialize)]
pub struct FileSource {
    pub path: String,
    pub content: String,
    pub language: String,
}

pub async fn list_files(State(state): State<SharedState>) -> impl IntoResponse {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<FileInfo>::new())).into_response(),
    };
    let conn = storage.conn();

    let mut stmt = match conn.prepare(
        "SELECT f.file_id, f.path, f.language, f.last_indexed_at, COUNT(r.region_id) as region_count \
         FROM files f LEFT JOIN regions r ON f.file_id = r.file_id \
         GROUP BY f.file_id ORDER BY f.path",
    ) {
        Ok(s) => s,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<FileInfo>::new())).into_response(),
    };

    let files: Vec<FileInfo> = stmt
        .query_map([], |row| {
            Ok(FileInfo {
                file_id: row.get(0)?,
                path: row.get(1)?,
                language: row.get(2)?,
                last_indexed_at: row.get(3)?,
                region_count: row.get(4)?,
            })
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();

    Json(files).into_response()
}

pub async fn get_file_detail(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let file = match storage.get_file(id) {
        Ok(Some(f)) => f,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let regions = storage.get_regions_for_file(id).unwrap_or_default();
    let region_infos: Vec<RegionInfo> = regions
        .into_iter()
        .map(|r| RegionInfo {
            region_id: r.region_id,
            kind: r.kind.as_str().to_string(),
            name: r.name,
            start_line: r.start_line,
            end_line: r.end_line,
            signature: r.signature,
        })
        .collect();

    let detail = FileDetail {
        file_id: file.file_id,
        path: file.path,
        language: file.language,
        last_indexed_at: file.last_indexed_at,
        regions: region_infos,
    };

    Json(detail).into_response()
}

pub async fn get_file_source(
    State(state): State<SharedState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let file = match storage.get_file(id) {
        Ok(Some(f)) => f,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    let full_path = state.project_root.join(&file.path);
    let content = match fs::read_to_string(&full_path) {
        Ok(c) => c,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };

    let source = FileSource {
        path: file.path,
        language: file.language,
        content,
    };

    Json(source).into_response()
}

// ── Config API ───────────────────────────────────────────────────────────────

pub async fn get_config(State(state): State<SharedState>) -> impl IntoResponse {
    match Config::load(&state.project_root) {
        Ok(config) => Json(config).into_response(),
        Err(_) => Json(Config::default()).into_response(),
    }
}

pub async fn update_config(
    State(state): State<SharedState>,
    Json(config): Json<Config>,
) -> impl IntoResponse {
    match config.save(&state.project_root) {
        Ok(()) => (
            StatusCode::OK,
            r#"<span style="color:#3fb950;">&#10003; Settings saved</span>"#,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(r#"<span style="color:#f85149;">&#10007; Save failed: {}</span>"#, e),
        )
            .into_response(),
    }
}

// ── Activity API ─────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct ActivityParams {
    #[serde(default = "default_activity_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
}
fn default_activity_limit() -> usize { 50 }

pub async fn list_activity(
    State(state): State<SharedState>,
    Query(params): Query<ActivityParams>,
) -> impl IntoResponse {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<ActivityEntry>::new())).into_response(),
    };
    match storage.list_activity(params.limit, params.offset) {
        Ok(entries) => Json(entries).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<ActivityEntry>::new())).into_response(),
    }
}
