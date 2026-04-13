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
        <div class="mb-2">
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
            />
            <div class="search-hint">
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
        return r#"<p class="text-muted mt-1">No results found.</p>"#.to_string();
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
            "<div class=\"card mb-1\">\n\
    <div class=\"flex justify-between items-center\">\n\
        <div>\n\
            <strong>{name}</strong>\n\
            <span class=\"badge badge-{kind_str}\" style=\"margin-left:8px;\">{kind_str}</span>\n\
        </div>\n\
        <span class=\"text-muted\" style=\"font-size:0.85em\">{file}:{start}-{end}</span>\n\
    </div>\n\
    <div class=\"mt-1\"><code class=\"text-muted font-mono\" style=\"font-size:0.85em\">{sig}</code></div>\n\
    <div class=\"relevance-bar mt-1\">\n\
        <div class=\"fill\" style=\"width:{relevance_pct:.1}%\"></div>\n\
    </div>",
        ));

        if let Some(code) = &result.code {
            let code_escaped = html_escape(code);
            html.push_str(&format!(
                "\n    <details class=\"mt-1\">\n\
        <summary class=\"text-accent\" style=\"cursor:pointer;font-size:0.9em\">Show code</summary>\n\
        <pre class=\"mt-1\" style=\"overflow:auto;\"><code class=\"language-{lang}\">{code_escaped}</code></pre>\n\
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
