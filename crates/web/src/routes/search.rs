use axum::extract::{Query, State};
use axum::response::Html;
use crate::state::SharedState;
use crate::templates;
use crate::routes::api::SearchParams;
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_core::storage::vectors::VectorIndex;
use codeindex_core::embedding::MockEmbeddingProvider;
use codeindex_core::query::{QueryEngine, QueryOptions};
use codeindex_core::model::QueryResult;

pub async fn page(State(_state): State<SharedState>) -> Html<String> {
    let content = r##"
        <div class="page-header">
            <h2>Search</h2>
        </div>
        <div style="margin-bottom:16px">
            <input
                type="text"
                name="q"
                id="search-input"
                class="search-input"
                placeholder="Search code..."
                hx-get="/search/results"
                hx-trigger="keyup changed delay:300ms"
                hx-target="#results"
                hx-include="this"
                autocomplete="off"
                style="width:100%;box-sizing:border-box;padding:10px 14px;font-size:1rem;border-radius:6px;border:1px solid #30363d;background:#161b22;color:#c9d1d9;outline:none;"
            />
            <div style="margin-top:6px;font-size:0.82em;color:#8b949e;">
                Try: authentication handler, :symbol Plugin, :file src/main.go, :deps src/auth.go::Login
            </div>
        </div>
        <div id="results"></div>
    "##;

    let scripts = r##"
        <link rel="stylesheet" href="/assets/css/hljs-dark.css">
        <script src="/assets/js/highlight.min.js"></script>
    "##;

    Html(templates::base_with_scripts("Search", "search", content, scripts))
}

fn language_from_ext(file: &str) -> &'static str {
    let ext = file.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "java" => "java",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "rb" => "ruby",
        "swift" => "swift",
        "kt" => "kotlin",
        "cs" => "csharp",
        "sh" | "bash" => "bash",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "md" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        _ => "plaintext",
    }
}

fn render_results(results: &[QueryResult]) -> String {
    if results.is_empty() {
        return "<p style=\"color:#8b949e;margin-top:16px;\">No results found.</p>".to_string();
    }

    let mut html = String::new();
    for result in results {
        let kind_str = result.kind.to_string();
        let name = html_escape(&result.name);
        let file = html_escape(&result.file);
        let sig = html_escape(&result.signature);
        let relevance_pct = (result.relevance * 100.0).min(100.0);
        let lang = language_from_ext(&result.file);
        let start = result.lines[0];
        let end = result.lines[1];

        html.push_str(&format!(
            "<div class=\"card\" style=\"margin-bottom:12px;\">\n\
    <div style=\"display:flex;justify-content:space-between;align-items:center\">\n\
        <div>\n\
            <strong style=\"color:#e6edf3\">{name}</strong>\n\
            <span class=\"badge badge-{kind_str}\" style=\"margin-left:8px;padding:2px 8px;border-radius:4px;font-size:0.78em;background:#21262d;color:#8b949e;border:1px solid #30363d;\">{kind_str}</span>\n\
        </div>\n\
        <span style=\"color:#8b949e;font-size:0.85em\">{file}:{start}-{end}</span>\n\
    </div>\n\
    <div style=\"margin-top:8px\"><code style=\"color:#8b949e;font-size:0.85em\">{sig}</code></div>\n\
    <div class=\"relevance-bar\" style=\"margin-top:8px;height:4px;background:#21262d;border-radius:2px;\">\n\
        <div class=\"fill\" style=\"width:{relevance_pct:.1}%;height:100%;background:#1f6feb;border-radius:2px;\"></div>\n\
    </div>",
        ));

        if let Some(code) = &result.code {
            let code_escaped = html_escape(code);
            html.push_str(&format!(
                "\n    <details style=\"margin-top:8px\">\n\
        <summary style=\"cursor:pointer;color:#58a6ff;font-size:0.9em\">Show code</summary>\n\
        <pre style=\"margin-top:8px;overflow:auto;border-radius:6px;\"><code class=\"language-{lang}\">{code_escaped}</code></pre>\n\
    </details>\n\
    <script>document.querySelectorAll('pre code').forEach(el => hljs.highlightElement(el));</script>",
            ));
        }

        html.push_str("\n</div>\n");
    }
    html
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

pub async fn results(
    State(state): State<SharedState>,
    Query(params): Query<SearchParams>,
) -> Html<String> {
    if params.q.trim().is_empty() {
        return Html(String::new());
    }

    let empty_response = Vec::new();

    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Html(render_results(&empty_response)),
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
        Ok(response) => Html(render_results(&response.results)),
        Err(_) => Html(render_results(&empty_response)),
    }
}
