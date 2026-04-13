# codeindex Web Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a local web dashboard to codeindex that visualizes index metrics, dependency graphs, code search, file browsing, settings, and activity logs.

**Architecture:** New `crates/web/` crate using Axum + Askama templates + HTMX for server-rendered interactivity, with Cytoscape.js for dependency graph visualization. Static assets embedded via rust-embed. CLI gets `codeindex ui` command. Dashboard reads from the same SQLite DB as CLI/daemon.

**Tech Stack:** axum 0.8, askama, htmx.js, cytoscape.js, chart.js, highlight.js, rust-embed, tailwindcss (pre-built CSS), webbrowser crate

**Spec:** `docs/2026-04-12-codeindex-v02-enhancements-design.md` + approved plan at `.claude/plans/twinkling-brewing-papert.md`

---

## File Structure

```
crates/web/
├── Cargo.toml
├── src/
│   ├── lib.rs                  # start_server() public API
│   ├── state.rs                # AppState struct
│   ├── server.rs               # Axum router + static handler
│   ├── routes/
│   │   ├── mod.rs
│   │   ├── dashboard.rs        # GET /
│   │   ├── search.rs           # GET /search
│   │   ├── files.rs            # GET /files
│   │   ├── graph.rs            # GET /graph
│   │   ├── settings.rs         # GET /settings
│   │   ├── activity.rs         # GET /activity
│   │   └── api.rs              # /api/* JSON endpoints
│   ├── templates/
│   │   ├── base.html
│   │   ├── dashboard.html
│   │   ├── search.html
│   │   ├── search_results.html
│   │   ├── files.html
│   │   ├── file_detail.html
│   │   ├── graph.html
│   │   ├── settings.html
│   │   └── activity.html
│   └── assets/
│       ├── css/style.css
│       ├── js/htmx.min.js
│       ├── js/chart.min.js
│       ├── js/cytoscape.min.js
│       ├── js/highlight.min.js
│       ├── js/dashboard.js
│       └── js/graph.js
```

---

### Task 1: Scaffold Web Crate + Base Layout

**Files:**
- Create: `codeindex/crates/web/Cargo.toml`
- Create: `codeindex/crates/web/src/lib.rs`
- Create: `codeindex/crates/web/src/state.rs`
- Create: `codeindex/crates/web/src/server.rs`
- Create: `codeindex/crates/web/src/routes/mod.rs`
- Create: `codeindex/crates/web/src/routes/dashboard.rs`
- Create: `codeindex/crates/web/src/templates/base.html`
- Create: `codeindex/crates/web/src/templates/dashboard.html`
- Create: `codeindex/crates/web/src/assets/css/style.css`
- Create: `codeindex/crates/web/src/assets/js/htmx.min.js`
- Modify: `codeindex/Cargo.toml` (add web to workspace members)

- [ ] **Step 1: Add web crate to workspace**

Add `"crates/web"` to the `members` list in `codeindex/Cargo.toml`.

- [ ] **Step 2: Create `crates/web/Cargo.toml`**

```toml
[package]
name = "codeindex-web"
version.workspace = true
edition.workspace = true
publish = false

[dependencies]
codeindex-core = { path = "../core" }
anyhow = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
axum = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "net"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors"] }
rust-embed = "8"
askama = "0.12"
mime_guess = "2"
```

- [ ] **Step 3: Create `src/state.rs`**

```rust
use std::path::PathBuf;
use std::sync::Arc;

pub type SharedState = Arc<AppState>;

pub struct AppState {
    pub project_root: PathBuf,
    pub db_path: PathBuf,
    pub index_dir: PathBuf,
}

impl AppState {
    pub fn new(project_root: PathBuf) -> anyhow::Result<Self> {
        let config = codeindex_core::config::Config::load(&project_root).unwrap_or_default();
        let index_dir = project_root.join(&config.index.path);
        let db_path = index_dir.join("index.db");
        Ok(Self { project_root, db_path, index_dir })
    }
}
```

- [ ] **Step 4: Create `src/assets/css/style.css`**

Write a self-contained dark-theme CSS file (~200 lines) with:
- Dark background (#0d1117), card backgrounds (#161b22), borders (#30363d)
- Sidebar navigation (fixed left, 220px)
- Main content area with padding
- Stat cards (grid of 4), tables, forms
- Code blocks with monospace font
- Color accents: blue (#58a6ff), green (#3fb950), purple (#bc8cff), yellow (#d29922)
- Responsive: sidebar collapses on narrow screens
- Utility classes for badges, relevance bars, charts

Do NOT use TailwindCSS build tooling — write plain CSS that covers what we need. This keeps the build simple.

- [ ] **Step 5: Download htmx.min.js**

Download htmx 2.0 minified JS from CDN and save to `src/assets/js/htmx.min.js`. Use:
```bash
curl -o crates/web/src/assets/js/htmx.min.js https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js
```

- [ ] **Step 6: Create base.html Askama template**

```html
<!-- crates/web/src/templates/base.html -->
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% block title %}codeindex{% endblock %}</title>
    <link rel="stylesheet" href="/assets/css/style.css">
    <script src="/assets/js/htmx.min.js"></script>
    {% block head %}{% endblock %}
</head>
<body>
    <nav class="sidebar">
        <div class="sidebar-header">
            <h1 class="logo">codeindex</h1>
        </div>
        <ul class="nav-links">
            <li><a href="/" class="{% if active_page == "dashboard" %}active{% endif %}">Dashboard</a></li>
            <li><a href="/search" class="{% if active_page == "search" %}active{% endif %}">Search</a></li>
            <li><a href="/files" class="{% if active_page == "files" %}active{% endif %}">Files</a></li>
            <li><a href="/graph" class="{% if active_page == "graph" %}active{% endif %}">Graph</a></li>
            <li><a href="/activity" class="{% if active_page == "activity" %}active{% endif %}">Activity</a></li>
            <li><a href="/settings" class="{% if active_page == "settings" %}active{% endif %}">Settings</a></li>
        </ul>
    </nav>
    <main class="content">
        {% block content %}{% endblock %}
    </main>
    {% block scripts %}{% endblock %}
</body>
</html>
```

- [ ] **Step 7: Create dashboard.html template (placeholder)**

```html
<!-- crates/web/src/templates/dashboard.html -->
{% extends "base.html" %}
{% block title %}Dashboard — codeindex{% endblock %}
{% block content %}
<h2>Dashboard</h2>
<p>Index at {{ index_path }}</p>
{% endblock %}
```

- [ ] **Step 8: Create `src/routes/dashboard.rs`**

```rust
use askama::Template;
use axum::extract::State;
use axum::response::Html;
use crate::state::SharedState;

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    active_page: String,
    index_path: String,
}

pub async fn index(State(state): State<SharedState>) -> Html<String> {
    let tmpl = DashboardTemplate {
        active_page: "dashboard".into(),
        index_path: state.index_dir.display().to_string(),
    };
    Html(tmpl.render().unwrap_or_else(|e| format!("Template error: {}", e)))
}
```

- [ ] **Step 9: Create `src/routes/mod.rs`**

```rust
pub mod dashboard;
```

- [ ] **Step 10: Create `src/server.rs`**

```rust
use axum::{Router, routing::get, response::IntoResponse, http::{header, StatusCode}};
use rust_embed::Embed;
use std::sync::Arc;
use crate::state::AppState;
use crate::routes;

#[derive(Embed)]
#[folder = "src/assets/"]
struct Assets;

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches("/assets/");
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_text_plain();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.to_string())],
                content.data.to_vec(),
            ).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(routes::dashboard::index))
        .route("/assets/{*path}", get(static_handler))
        .with_state(state)
}
```

- [ ] **Step 11: Create `src/lib.rs`**

```rust
mod state;
mod server;
mod routes;

use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;

pub async fn start_server(project_root: PathBuf, port: u16) -> Result<()> {
    let state = Arc::new(state::AppState::new(project_root)?);
    let app = server::build_router(state);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Dashboard at http://127.0.0.1:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
```

- [ ] **Step 12: Verify build**

Run: `cd codeindex && cargo build -p codeindex-web`
Expected: compiles

- [ ] **Step 13: Commit**

```bash
git add -A && git commit -m "feat: scaffold web dashboard crate with base layout and Axum server"
```

---

### Task 2: CLI `ui` Command + Browser Open

**Files:**
- Create: `codeindex/crates/cli/src/commands/ui.rs`
- Modify: `codeindex/crates/cli/Cargo.toml`
- Modify: `codeindex/crates/cli/src/main.rs`
- Modify: `codeindex/crates/cli/src/commands/mod.rs`

- [ ] **Step 1: Add dependencies to CLI Cargo.toml**

Add to `crates/cli/Cargo.toml`:
```toml
codeindex-web = { path = "../web" }
webbrowser = "1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

- [ ] **Step 2: Create `src/commands/ui.rs`**

```rust
use anyhow::Result;
use codeindex_core::config::Config;

pub fn run(port: u16, no_open: bool) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let config = Config::load(&project_root).unwrap_or_default();
    let index_dir = project_root.join(&config.index.path);

    if !index_dir.join("index.db").exists() {
        anyhow::bail!("No index found. Run `codeindex init && codeindex index` first.");
    }

    println!("Starting codeindex dashboard at http://127.0.0.1:{}", port);

    if !no_open {
        let url = format!("http://127.0.0.1:{}", port);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = webbrowser::open(&url);
        });
    }

    tokio::runtime::Runtime::new()?
        .block_on(codeindex_web::start_server(project_root, port))
}
```

- [ ] **Step 3: Add Ui command to main.rs**

Add to the Commands enum:
```rust
    /// Launch the web dashboard
    Ui {
        /// Port to serve on
        #[arg(long, default_value = "3742")]
        port: u16,
        /// Don't auto-open browser
        #[arg(long)]
        no_open: bool,
    },
```

Add match arm:
```rust
Commands::Ui { port, no_open } => commands::ui::run(port, no_open),
```

- [ ] **Step 4: Update commands/mod.rs**

Add `pub mod ui;`

- [ ] **Step 5: Verify build and test manually**

Run: `cd codeindex && cargo build -p codeindex`
Then: `cd /tmp && mkdir test-dash && cd test-dash && codeindex init && codeindex index`
Then: `codeindex ui --no-open`
Then open http://127.0.0.1:3742 in browser — should see the placeholder dashboard.
Clean up: Ctrl+C, `rm -rf /tmp/test-dash`

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: add codeindex ui command to launch web dashboard"
```

---

### Task 3: Dashboard Page with Real Metrics

**Files:**
- Create: `codeindex/crates/web/src/routes/api.rs`
- Modify: `codeindex/crates/web/src/routes/mod.rs`
- Modify: `codeindex/crates/web/src/routes/dashboard.rs`
- Modify: `codeindex/crates/web/src/templates/dashboard.html`
- Modify: `codeindex/crates/web/src/server.rs`
- Create: `codeindex/crates/web/src/assets/js/dashboard.js`
- Download: `codeindex/crates/web/src/assets/js/chart.min.js`

- [ ] **Step 1: Download Chart.js**

```bash
curl -o crates/web/src/assets/js/chart.min.js https://cdn.jsdelivr.net/npm/chart.js@4.4.7/dist/chart.umd.min.js
```

- [ ] **Step 2: Implement `/api/stats` endpoint in `routes/api.rs`**

```rust
use axum::extract::State;
use axum::Json;
use serde::Serialize;
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

#[derive(Serialize)]
pub struct LabelCount {
    pub label: String,
    pub count: i64,
}

pub async fn stats(State(state): State<SharedState>) -> Json<StatsResponse> {
    let storage = SqliteStorage::open(&state.db_path).unwrap();
    let conn = storage.conn();

    let total_files: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap_or(0);
    let total_regions: i64 = conn.query_row("SELECT COUNT(*) FROM regions", [], |r| r.get(0)).unwrap_or(0);
    let total_dependencies: i64 = conn.query_row("SELECT COUNT(*) FROM dependencies", [], |r| r.get(0)).unwrap_or(0);

    let languages = query_label_counts(conn, "SELECT language, COUNT(*) FROM files GROUP BY language ORDER BY COUNT(*) DESC");
    let region_kinds = query_label_counts(conn, "SELECT kind, COUNT(*) FROM regions GROUP BY kind ORDER BY COUNT(*) DESC");
    let dep_kinds = query_label_counts(conn, "SELECT kind, COUNT(*) FROM dependencies GROUP BY kind ORDER BY COUNT(*) DESC");

    let db_size_bytes = std::fs::metadata(&state.db_path).map(|m| m.len()).unwrap_or(0);
    let vector_size_bytes = std::fs::metadata(state.index_dir.join("vectors.json")).map(|m| m.len()).unwrap_or(0);
    let last_indexed_at: Option<i64> = conn.query_row("SELECT MAX(last_indexed_at) FROM files", [], |r| r.get(0)).ok().flatten();

    Json(StatsResponse {
        total_files, total_regions, total_dependencies,
        languages, region_kinds, dep_kinds,
        db_size_bytes, vector_size_bytes, last_indexed_at,
    })
}

fn query_label_counts(conn: &rusqlite::Connection, sql: &str) -> Vec<LabelCount> {
    let mut stmt = conn.prepare(sql).unwrap();
    stmt.query_map([], |row| Ok(LabelCount { label: row.get(0)?, count: row.get(1)? }))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
}
```

- [ ] **Step 3: Update dashboard.html with real metrics layout**

Replace `crates/web/src/templates/dashboard.html` with a full dashboard template that:
- Loads stats from `/api/stats` via HTMX on page load (`hx-get="/api/stats" hx-trigger="load"`)
- Shows 4 stat cards (files, regions, deps, index size)
- Has chart containers for language breakdown (bar) and region kinds (doughnut)
- Shows last indexed timestamp
- Includes Chart.js and dashboard.js scripts

- [ ] **Step 4: Create `dashboard.js`**

Script that fetches `/api/stats`, renders Chart.js bar chart for languages and doughnut for region kinds. Updates stat card numbers.

- [ ] **Step 5: Wire routes in server.rs**

Add to router:
```rust
.route("/api/stats", get(routes::api::stats))
```

Update `routes/mod.rs` to include `pub mod api;`

- [ ] **Step 6: Update dashboard.rs to pass stats data**

The dashboard template should render with server-side data (stats fetched in the handler) OR use HTMX to load stats client-side. Choose server-side for initial render speed:

```rust
pub async fn index(State(state): State<SharedState>) -> Html<String> {
    let storage = SqliteStorage::open(&state.db_path).ok();
    let (files, regions, deps) = match &storage {
        Some(s) => {
            let c = s.conn();
            (
                c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_,i64>(0)).unwrap_or(0),
                c.query_row("SELECT COUNT(*) FROM regions", [], |r| r.get::<_,i64>(0)).unwrap_or(0),
                c.query_row("SELECT COUNT(*) FROM dependencies", [], |r| r.get::<_,i64>(0)).unwrap_or(0),
            )
        }
        None => (0, 0, 0),
    };
    // Pass to template...
}
```

- [ ] **Step 7: Test dashboard**

Index rigbox-sidecar: `cd rigbox-sidecar && codeindex index`
Run: `codeindex ui --no-open` and open http://127.0.0.1:3742
Verify: stat cards show real numbers, charts render.
Clean up: `rm -rf rigbox-sidecar/.codeindex`

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: dashboard page with metrics, charts, and index stats"
```

---

### Task 4: Search Page

**Files:**
- Create: `codeindex/crates/web/src/routes/search.rs`
- Create: `codeindex/crates/web/src/templates/search.html`
- Create: `codeindex/crates/web/src/templates/search_results.html`
- Modify: `codeindex/crates/web/src/routes/api.rs`
- Modify: `codeindex/crates/web/src/routes/mod.rs`
- Modify: `codeindex/crates/web/src/server.rs`
- Download: `codeindex/crates/web/src/assets/js/highlight.min.js`

- [ ] **Step 1: Download highlight.js**

```bash
curl -o crates/web/src/assets/js/highlight.min.js https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.11.1/highlight.min.js
```

Also download a dark theme CSS:
```bash
curl -o crates/web/src/assets/css/hljs-dark.css https://cdnjs.cloudflare.com/ajax/libs/highlight.js/11.11.1/styles/github-dark.min.css
```

- [ ] **Step 2: Add `/api/search` endpoint**

In `routes/api.rs`, add a search handler that:
- Accepts query params: `q` (string), `top` (usize, default 10), `depth` (usize, default 1)
- Opens SqliteStorage, creates MockEmbeddingProvider (or loads real one), creates VectorIndex, creates QueryEngine
- Calls `engine.query(&q, &opts)`
- Returns `Json<QueryResponse>`

For the embedding provider: since the dashboard is read-only, use MockEmbeddingProvider with the same dimension as the stored vectors. FTS5 search works without real embeddings.

- [ ] **Step 3: Create `search.html` template**

Search page with:
- Large search input with placeholder text showing query syntax hints
- `hx-get="/search/results" hx-trigger="keyup changed delay:300ms" hx-target="#results"`
- Results div (`#results`) initially empty
- Includes highlight.js for code syntax highlighting

- [ ] **Step 4: Create `search_results.html` partial**

HTMX partial that renders a list of result cards:
- Each card shows: region name (bold), kind badge (colored), file path, line range
- Signature in monospace
- Relevance bar (CSS width based on score)
- Collapsible code preview with syntax highlighting
- Dependency links

- [ ] **Step 5: Create `routes/search.rs`**

Page handler for `/search` (full page) and `/search/results` (HTMX partial that calls the API and renders the partial template).

- [ ] **Step 6: Wire routes**

Add to server.rs router:
```rust
.route("/search", get(routes::search::page))
.route("/search/results", get(routes::search::results))
.route("/api/search", get(routes::api::search))
```

- [ ] **Step 7: Test search**

Index rigbox-sidecar, run `codeindex ui`, search for "Plugin", verify results appear with code previews.

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: search page with live results and syntax highlighting"
```

---

### Task 5: File Explorer

**Files:**
- Create: `codeindex/crates/web/src/routes/files.rs`
- Create: `codeindex/crates/web/src/templates/files.html`
- Create: `codeindex/crates/web/src/templates/file_detail.html`
- Modify: `codeindex/crates/web/src/routes/api.rs`
- Modify: `codeindex/crates/web/src/server.rs`

- [ ] **Step 1: Add file list and detail API endpoints**

In `routes/api.rs`:
- `GET /api/files` — returns `Vec<FileInfo>` where `FileInfo` has `file_id, path, language, region_count, last_indexed_at`. Join files with a COUNT subquery on regions.
- `GET /api/files/{id}` — returns `FileDetail { file: IndexedFile, regions: Vec<Region> }`
- `GET /api/files/{id}/source` — reads the actual source file from disk, returns `{ path, content, language }`

- [ ] **Step 2: Create files.html template**

Two-panel layout:
- Left panel: file list grouped by directory, loaded server-side
- Each file entry: `hx-get="/files/detail/{id}" hx-target="#file-detail"` on click
- Right panel: `#file-detail` div for HTMX-swapped file detail

- [ ] **Step 3: Create file_detail.html partial**

Shows:
- File path, language, last indexed timestamp
- Table of regions: kind, name, lines, signature
- Source code with line numbers and syntax highlighting

- [ ] **Step 4: Create `routes/files.rs`**

Handlers for the file page (full) and file detail (HTMX partial).

- [ ] **Step 5: Wire routes and test**

Add routes to server.rs. Test by browsing files in an indexed project.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat: file explorer with source code viewer and region breakdown"
```

---

### Task 6: Dependency Graph

**Files:**
- Create: `codeindex/crates/web/src/routes/graph.rs`
- Create: `codeindex/crates/web/src/templates/graph.html`
- Create: `codeindex/crates/web/src/assets/js/graph.js`
- Download: `codeindex/crates/web/src/assets/js/cytoscape.min.js`
- Modify: `codeindex/crates/web/src/routes/api.rs`
- Modify: `codeindex/crates/web/src/server.rs`

- [ ] **Step 1: Download Cytoscape.js**

```bash
curl -o crates/web/src/assets/js/cytoscape.min.js https://cdnjs.cloudflare.com/ajax/libs/cytoscape/3.31.0/cytoscape.min.js
```

- [ ] **Step 2: Add graph API endpoints**

In `routes/api.rs`:
- `GET /api/graph?limit=200` — BFS traversal of the dependency graph, returns Cytoscape-format JSON:
```json
{
  "nodes": [{"data": {"id": "r_42", "label": "func_name", "kind": "function", "file": "path", "parent": "f_3"}}],
  "edges": [{"data": {"source": "r_42", "target": "r_17", "kind": "calls"}}]
}
```
Include file nodes as compound parents. Limit total nodes to prevent overload.

- `GET /api/graph/node/{id}` — detail for a clicked node: region info, file info, outgoing deps, incoming deps.

- [ ] **Step 3: Create graph.html template**

Full-page graph with:
- Toolbar: layout dropdown (dagre/cose), node limit slider, zoom buttons
- Full-height Cytoscape canvas
- Right side panel (hidden by default, shows on node click)
- Loads cytoscape.min.js and graph.js

- [ ] **Step 4: Create graph.js**

JavaScript that:
- Fetches `/api/graph?limit=200` on load
- Initializes Cytoscape with cose layout
- Colors nodes by kind (function=blue, struct=green, class=purple, etc.)
- Colors edges by kind (calls=blue, imports=gray, type_ref=purple)
- On node tap: fetches `/api/graph/node/{id}`, shows detail in side panel
- Layout selector changes layout algorithm
- Zoom buttons

- [ ] **Step 5: Create `routes/graph.rs`**

Handler for the graph page (full HTML).

- [ ] **Step 6: Wire routes and test**

Test on rigbox-sidecar — graph should show interconnected Go functions.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: interactive dependency graph with Cytoscape.js"
```

---

### Task 7: Settings + Activity Log

**Files:**
- Create: `codeindex/crates/web/src/routes/settings.rs`
- Create: `codeindex/crates/web/src/routes/activity.rs`
- Create: `codeindex/crates/web/src/templates/settings.html`
- Create: `codeindex/crates/web/src/templates/activity.html`
- Modify: `codeindex/crates/core/src/storage/sqlite.rs`
- Modify: `codeindex/crates/core/src/model.rs`
- Modify: `codeindex/crates/web/src/routes/api.rs`
- Modify: `codeindex/crates/web/src/server.rs`

- [ ] **Step 1: Add activity_log table to SQLite schema**

In `crates/core/src/storage/sqlite.rs`, add to `initialize()`:

```sql
CREATE TABLE IF NOT EXISTS activity_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    detail TEXT NOT NULL,
    source TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_activity_timestamp ON activity_log(timestamp DESC);
```

- [ ] **Step 2: Add ActivityEntry model**

In `crates/core/src/model.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEntry {
    pub id: i64,
    pub timestamp: i64,
    pub event_type: String,
    pub detail: String,
    pub source: String,
}
```

- [ ] **Step 3: Add SqliteStorage methods for activity**

```rust
pub fn insert_activity(&self, event_type: &str, detail: &str, source: &str) -> Result<i64> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    self.conn.execute(
        "INSERT INTO activity_log (timestamp, event_type, detail, source) VALUES (?1, ?2, ?3, ?4)",
        params![now, event_type, detail, source],
    )?;
    Ok(self.conn.last_insert_rowid())
}

pub fn list_activity(&self, limit: usize, offset: usize) -> Result<Vec<ActivityEntry>> {
    let mut stmt = self.conn.prepare(
        "SELECT id, timestamp, event_type, detail, source FROM activity_log ORDER BY timestamp DESC LIMIT ?1 OFFSET ?2"
    )?;
    let rows = stmt.query_map(params![limit as i64, offset as i64], |row| {
        Ok(ActivityEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            event_type: row.get(2)?,
            detail: row.get(3)?,
            source: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}
```

- [ ] **Step 4: Add config and activity API endpoints**

In `routes/api.rs`:
- `GET /api/config` — loads and returns Config as JSON
- `PUT /api/config` — receives partial config JSON, merges, saves
- `GET /api/activity?limit=50&offset=0` — returns paginated activity entries

- [ ] **Step 5: Create settings.html template**

Form sections for each config area (embedding, summary, query, daemon, index). Each field maps to a config key. Save button POSTs via HTMX to `/api/config`. Success/error toast.

- [ ] **Step 6: Create activity.html template**

Table with columns: Time, Event, Detail, Source. HTMX infinite scroll for pagination. Filter dropdown for event type.

- [ ] **Step 7: Create route handlers**

`routes/settings.rs` and `routes/activity.rs` — page handlers rendering their templates.

- [ ] **Step 8: Wire routes**

Add all new routes to server.rs.

- [ ] **Step 9: Run tests**

`cd codeindex && cargo test --workspace`
Verify activity_log table exists and CRUD works.

- [ ] **Step 10: Commit**

```bash
git add -A && git commit -m "feat: settings page and activity log with audit trail"
```

---

### Task 8: Activity Logging in Existing Commands + Polish

**Files:**
- Modify: `codeindex/crates/cli/src/commands/index.rs`
- Modify: `codeindex/crates/cli/src/commands/gc.rs`
- Modify: `codeindex/crates/cli/src/commands/config_cmd.rs`
- Modify: `codeindex/crates/web/src/routes/api.rs` (add /api/reindex)

- [ ] **Step 1: Add activity logging to index command**

After indexing completes in `index.rs`, insert activity:
```rust
storage.insert_activity("index", &format!("{{\"files\":{},\"regions\":{}}}", stats.files_indexed, stats.regions_extracted), "cli")?;
```

- [ ] **Step 2: Add activity logging to gc command**

After gc completes in `gc.rs`:
```rust
storage.insert_activity("gc", &format!("{{\"removed\":{}}}", removed), "cli")?;
```

- [ ] **Step 3: Add activity logging to config set**

After config save in `config_cmd.rs`:
```rust
// Open storage to log
if let Ok(storage) = SqliteStorage::open(&project_root.join(&config.index.path).join("index.db")) {
    let _ = storage.insert_activity("config_change", &format!("{{\"key\":\"{}\",\"value\":\"{}\"}}", key, value), "cli");
}
```

- [ ] **Step 4: Add `/api/reindex` POST endpoint**

In `routes/api.rs`, add a handler that triggers a full re-index by spawning a background task (or running synchronously for simplicity). Returns `{"status": "started"}`.

- [ ] **Step 5: Run full test suite**

`cd codeindex && cargo test --workspace`
`cd codeindex && cargo clippy --workspace`

- [ ] **Step 6: End-to-end test on real project**

```bash
cd /Users/jonathanjames/workspace/rigbox-sidecar
codeindex init && codeindex index
codeindex ui --no-open
# Open http://127.0.0.1:3742 in browser
# Verify: dashboard metrics, search, files, graph, settings, activity log
rm -rf .codeindex
```

- [ ] **Step 7: Commit and push**

```bash
git add -A && git commit -m "feat: activity logging + reindex endpoint + dashboard polish"
git push origin main
```
