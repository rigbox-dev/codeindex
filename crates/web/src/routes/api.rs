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
use codeindex_core::model::QueryResponse;

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

impl Default for StatsResponse {
    fn default() -> Self {
        Self {
            total_files: 0,
            total_regions: 0,
            total_dependencies: 0,
            languages: Vec::new(),
            region_kinds: Vec::new(),
            dep_kinds: Vec::new(),
            db_size_bytes: 0,
            vector_size_bytes: 0,
            last_indexed_at: None,
        }
    }
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
