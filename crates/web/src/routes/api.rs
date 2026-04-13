use axum::extract::State;
use axum::Json;
use serde::Serialize;
use std::fs;
use crate::state::SharedState;
use codeindex_core::storage::sqlite::SqliteStorage;

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
