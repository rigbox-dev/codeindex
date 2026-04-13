use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{Dependency, DependencyKind, IndexedFile, Region, RegionKind};

/// SQLite-backed storage for codeindex.
pub struct SqliteStorage {
    conn: Connection,
}

impl SqliteStorage {
    /// Open (or create) a database at the given path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let storage = Self { conn };
        storage.initialize()?;
        Ok(storage)
    }

    /// Open an in-memory database (useful for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self { conn };
        storage.initialize()?;
        Ok(storage)
    }

    /// Create all tables, indexes, and FTS5 virtual table.
    fn initialize(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS files (
                file_id          INTEGER PRIMARY KEY AUTOINCREMENT,
                path             TEXT NOT NULL UNIQUE,
                content_hash     TEXT NOT NULL,
                language         TEXT NOT NULL,
                last_indexed_at  INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS regions (
                region_id        INTEGER PRIMARY KEY AUTOINCREMENT,
                file_id          INTEGER NOT NULL REFERENCES files(file_id) ON DELETE CASCADE,
                parent_region_id INTEGER REFERENCES regions(region_id) ON DELETE SET NULL,
                kind             TEXT NOT NULL,
                name             TEXT NOT NULL,
                start_line       INTEGER NOT NULL,
                end_line         INTEGER NOT NULL,
                start_byte       INTEGER NOT NULL,
                end_byte         INTEGER NOT NULL,
                signature        TEXT NOT NULL,
                content_hash     TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dependencies (
                id               INTEGER PRIMARY KEY AUTOINCREMENT,
                source_region_id INTEGER NOT NULL REFERENCES regions(region_id) ON DELETE CASCADE,
                target_region_id INTEGER REFERENCES regions(region_id) ON DELETE SET NULL,
                target_symbol    TEXT NOT NULL,
                target_path      TEXT,
                kind             TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS summaries (
                region_id        INTEGER PRIMARY KEY REFERENCES regions(region_id) ON DELETE CASCADE,
                summary_text     TEXT NOT NULL,
                generated_at     INTEGER NOT NULL,
                model_version    TEXT NOT NULL
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS regions_fts USING fts5(
                name,
                signature,
                summary_text,
                content='',
                tokenize='porter ascii'
            );

            CREATE INDEX IF NOT EXISTS idx_regions_file_id
                ON regions(file_id);

            CREATE INDEX IF NOT EXISTS idx_dependencies_source
                ON dependencies(source_region_id);

            CREATE INDEX IF NOT EXISTS idx_dependencies_target
                ON dependencies(target_region_id);

            CREATE INDEX IF NOT EXISTS idx_dependencies_symbol
                ON dependencies(target_symbol);
            ",
        )?;

        Ok(())
    }

    // -------------------------------------------------------------------------
    // Files
    // -------------------------------------------------------------------------

    /// Upsert a file record, returning the file_id.
    pub fn upsert_file(&self, path: &str, content_hash: &str, language: &str) -> Result<i64> {
        let now = now_unix();
        self.conn.execute(
            "INSERT INTO files (path, content_hash, language, last_indexed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET
                 content_hash    = excluded.content_hash,
                 language        = excluded.language,
                 last_indexed_at = excluded.last_indexed_at",
            params![path, content_hash, language, now],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT file_id FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    /// Retrieve a file by its primary key.
    pub fn get_file(&self, file_id: i64) -> Result<Option<IndexedFile>> {
        let result = self
            .conn
            .query_row(
                "SELECT file_id, path, content_hash, language, last_indexed_at
                 FROM files WHERE file_id = ?1",
                params![file_id],
                row_to_indexed_file,
            )
            .optional()?;
        Ok(result)
    }

    /// Retrieve a file by its path.
    pub fn get_file_by_path(&self, path: &str) -> Result<Option<IndexedFile>> {
        let result = self
            .conn
            .query_row(
                "SELECT file_id, path, content_hash, language, last_indexed_at
                 FROM files WHERE path = ?1",
                params![path],
                row_to_indexed_file,
            )
            .optional()?;
        Ok(result)
    }

    // -------------------------------------------------------------------------
    // Regions
    // -------------------------------------------------------------------------

    /// Insert a region and return the newly assigned region_id (rowid).
    pub fn insert_region(&self, region: &Region) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO regions
                (file_id, parent_region_id, kind, name, start_line, end_line,
                 start_byte, end_byte, signature, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                region.file_id,
                region.parent_region_id,
                region.kind.as_str(),
                region.name,
                region.start_line,
                region.end_line,
                region.start_byte,
                region.end_byte,
                region.signature,
                region.content_hash,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Retrieve all regions for a file, ordered by start_line.
    pub fn get_regions_for_file(&self, file_id: i64) -> Result<Vec<Region>> {
        let mut stmt = self.conn.prepare(
            "SELECT region_id, file_id, parent_region_id, kind, name,
                    start_line, end_line, start_byte, end_byte, signature, content_hash
             FROM regions
             WHERE file_id = ?1
             ORDER BY start_line",
        )?;
        let rows = stmt.query_map(params![file_id], row_to_region)?;
        let mut regions = Vec::new();
        for r in rows {
            regions.push(r?);
        }
        Ok(regions)
    }

    /// Retrieve a single region by its primary key.
    pub fn get_region(&self, region_id: i64) -> Result<Option<Region>> {
        let result = self
            .conn
            .query_row(
                "SELECT region_id, file_id, parent_region_id, kind, name,
                         start_line, end_line, start_byte, end_byte, signature, content_hash
                  FROM regions WHERE region_id = ?1",
                params![region_id],
                row_to_region,
            )
            .optional()?;
        Ok(result)
    }

    /// Delete all regions belonging to a file.
    pub fn delete_regions_for_file(&self, file_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM regions WHERE file_id = ?1",
            params![file_id],
        )?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Dependencies
    // -------------------------------------------------------------------------

    /// Insert a dependency edge.
    pub fn insert_dependency(&self, dep: &Dependency) -> Result<()> {
        self.conn.execute(
            "INSERT INTO dependencies
                (source_region_id, target_region_id, target_symbol, target_path, kind)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                dep.source_region_id,
                dep.target_region_id,
                dep.target_symbol,
                dep.target_path,
                dep.kind.as_str(),
            ],
        )?;
        Ok(())
    }

    /// All dependencies whose source is the given region.
    pub fn get_dependencies_from(&self, region_id: i64) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_region_id, target_region_id, target_symbol, target_path, kind
             FROM dependencies
             WHERE source_region_id = ?1",
        )?;
        let rows = stmt.query_map(params![region_id], row_to_dependency)?;
        let mut deps = Vec::new();
        for d in rows {
            deps.push(d?);
        }
        Ok(deps)
    }

    /// All dependencies whose target is the given region.
    pub fn get_dependencies_to(&self, region_id: i64) -> Result<Vec<Dependency>> {
        let mut stmt = self.conn.prepare(
            "SELECT source_region_id, target_region_id, target_symbol, target_path, kind
             FROM dependencies
             WHERE target_region_id = ?1",
        )?;
        let rows = stmt.query_map(params![region_id], row_to_dependency)?;
        let mut deps = Vec::new();
        for d in rows {
            deps.push(d?);
        }
        Ok(deps)
    }

    /// Delete all dependency rows originating from a region.
    pub fn delete_dependencies_for_region(&self, region_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM dependencies WHERE source_region_id = ?1",
            params![region_id],
        )?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // FTS5
    // -------------------------------------------------------------------------

    /// Add or replace a region's entry in the FTS index.
    pub fn index_region_fts(
        &self,
        region_id: i64,
        name: &str,
        signature: &str,
        summary: Option<&str>,
    ) -> Result<()> {
        // Remove any stale entry first (external-content FTS requires manual management).
        self.delete_region_fts(region_id)?;
        self.conn.execute(
            "INSERT INTO regions_fts(rowid, name, signature, summary_text)
             VALUES (?1, ?2, ?3, ?4)",
            params![region_id, name, signature, summary.unwrap_or("")],
        )?;
        Ok(())
    }

    /// Remove a region from the FTS index.
    pub fn delete_region_fts(&self, region_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM regions_fts WHERE rowid = ?1",
            params![region_id],
        )?;
        Ok(())
    }

    /// Full-text search; returns region_ids ranked by FTS5 rank, up to 100.
    pub fn search_fts(&self, query: &str) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT rowid FROM regions_fts
             WHERE regions_fts MATCH ?1
             ORDER BY rank
             LIMIT 100",
        )?;
        let rows = stmt.query_map(params![query], |row| row.get::<_, i64>(0))?;
        let mut ids = Vec::new();
        for id in rows {
            ids.push(id?);
        }
        Ok(ids)
    }
}

// -------------------------------------------------------------------------
// Row-mapping helpers
// -------------------------------------------------------------------------

fn row_to_indexed_file(row: &rusqlite::Row<'_>) -> rusqlite::Result<IndexedFile> {
    Ok(IndexedFile {
        file_id: row.get(0)?,
        path: row.get(1)?,
        content_hash: row.get(2)?,
        language: row.get(3)?,
        last_indexed_at: row.get(4)?,
    })
}

fn row_to_region(row: &rusqlite::Row<'_>) -> rusqlite::Result<Region> {
    let kind_str: String = row.get(3)?;
    let kind: RegionKind =
        serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(RegionKind::Function);
    Ok(Region {
        region_id: row.get(0)?,
        file_id: row.get(1)?,
        parent_region_id: row.get(2)?,
        kind,
        name: row.get(4)?,
        start_line: row.get(5)?,
        end_line: row.get(6)?,
        start_byte: row.get(7)?,
        end_byte: row.get(8)?,
        signature: row.get(9)?,
        content_hash: row.get(10)?,
    })
}

fn row_to_dependency(row: &rusqlite::Row<'_>) -> rusqlite::Result<Dependency> {
    let kind_str: String = row.get(4)?;
    let kind: DependencyKind =
        serde_json::from_str(&format!("\"{}\"", kind_str)).unwrap_or(DependencyKind::Calls);
    Ok(Dependency {
        source_region_id: row.get(0)?,
        target_region_id: row.get(1)?,
        target_symbol: row.get(2)?,
        target_path: row.get(3)?,
        kind,
    })
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// -------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_region(file_id: i64, name: &str) -> Region {
        Region {
            region_id: 0, // will be assigned by DB
            file_id,
            parent_region_id: None,
            kind: RegionKind::Function,
            name: name.to_string(),
            start_line: 1,
            end_line: 10,
            start_byte: 0,
            end_byte: 200,
            signature: format!("fn {}()", name),
            content_hash: "abc123".to_string(),
        }
    }

    #[test]
    fn open_in_memory_creates_tables() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let count: i64 = storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name IN ('files','regions','dependencies','summaries')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 4, "expected 4 tables, found {}", count);
    }

    #[test]
    fn insert_and_get_file() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let id = storage
            .upsert_file("src/main.rs", "hash1", "rust")
            .unwrap();
        let file = storage.get_file(id).unwrap().expect("file should exist");
        assert_eq!(file.path, "src/main.rs");
        assert_eq!(file.content_hash, "hash1");
        assert_eq!(file.language, "rust");
    }

    #[test]
    fn upsert_file_updates_existing() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let id1 = storage
            .upsert_file("src/lib.rs", "hashA", "rust")
            .unwrap();
        let id2 = storage
            .upsert_file("src/lib.rs", "hashB", "rust")
            .unwrap();
        assert_eq!(id1, id2, "upsert must return the same id");
        let file = storage.get_file(id1).unwrap().unwrap();
        assert_eq!(file.content_hash, "hashB", "hash should be updated");
    }

    #[test]
    fn insert_and_get_regions() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let file_id = storage.upsert_file("src/a.rs", "h", "rust").unwrap();
        let mut region = make_region(file_id, "my_func");
        region.start_line = 5;
        let rid = storage.insert_region(&region).unwrap();
        assert!(rid > 0);

        let regions = storage.get_regions_for_file(file_id).unwrap();
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].name, "my_func");
        assert_eq!(regions[0].kind, RegionKind::Function);
        assert_eq!(regions[0].region_id, rid);
    }

    #[test]
    fn insert_and_query_dependencies() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let fid = storage.upsert_file("src/b.rs", "h", "rust").unwrap();

        let r1 = make_region(fid, "caller");
        let rid1 = storage.insert_region(&r1).unwrap();

        let r2 = make_region(fid, "callee");
        let rid2 = storage.insert_region(&r2).unwrap();

        let dep = Dependency {
            source_region_id: rid1,
            target_region_id: Some(rid2),
            target_symbol: "callee".to_string(),
            target_path: None,
            kind: DependencyKind::Calls,
        };
        storage.insert_dependency(&dep).unwrap();

        let from = storage.get_dependencies_from(rid1).unwrap();
        assert_eq!(from.len(), 1);
        assert_eq!(from[0].target_symbol, "callee");
        assert_eq!(from[0].kind, DependencyKind::Calls);

        let to = storage.get_dependencies_to(rid2).unwrap();
        assert_eq!(to.len(), 1);
        assert_eq!(to[0].source_region_id, rid1);
    }

    #[test]
    fn fts_search_finds_by_name() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        let fid = storage.upsert_file("src/c.rs", "h", "rust").unwrap();
        let region = make_region(fid, "parse_tokens");
        let rid = storage.insert_region(&region).unwrap();
        storage
            .index_region_fts(rid, "parse_tokens", "fn parse_tokens()", None)
            .unwrap();

        let results = storage.search_fts("parse_tokens").unwrap();
        assert!(
            results.contains(&rid),
            "expected region_id {} in FTS results: {:?}",
            rid,
            results
        );
    }
}
