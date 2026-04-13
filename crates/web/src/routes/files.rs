use axum::extract::{State, Path};
use axum::response::Html;
use crate::state::SharedState;
use crate::templates;
use codeindex_core::storage::sqlite::SqliteStorage;
use std::collections::BTreeMap;
use std::fs;

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn language_class(lang: &str) -> &'static str {
    match lang {
        "rust" => "badge-rust",
        "python" => "badge-python",
        "javascript" | "js" => "badge-javascript",
        "typescript" | "ts" => "badge-typescript",
        "go" => "badge-go",
        _ => "badge-other",
    }
}

fn hljs_language(lang: &str) -> &'static str {
    match lang {
        "rust" => "rust",
        "python" => "python",
        "javascript" => "javascript",
        "typescript" => "typescript",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "cpp" => "cpp",
        "ruby" => "ruby",
        "swift" => "swift",
        "kotlin" => "kotlin",
        "csharp" => "csharp",
        "bash" | "shell" => "bash",
        "toml" => "toml",
        "yaml" => "yaml",
        "json" => "json",
        "markdown" => "markdown",
        "html" => "html",
        "css" => "css",
        _ => "plaintext",
    }
}

fn format_timestamp(ts: i64) -> String {
    // Format as a readable date from unix timestamp (seconds)
    let secs = ts as u64;
    let days = secs / 86400;
    let epoch_days = 719162u64; // days from year 1 to 1970-01-01
    let total_days = epoch_days + days;
    // Simple Gregorian calendar calculation
    let z = total_days + 306;
    let h = 100 * z - 25;
    let a = h / 3652425;
    let b = a - a / 4;
    let year = (100 * b + h) / 36525;
    let c = b + z - 365 * year - year / 4;
    let month = (5 * c + 456) / 153;
    let day = c - (153 * month - 457) / 5;
    let (month, year) = if month > 12 {
        (month - 12, year + 1)
    } else {
        (month, year)
    };
    format!("{:04}-{:02}-{:02}", year, month, day)
}

pub async fn page(State(state): State<SharedState>) -> Html<String> {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => {
            let content = r#"<div class="page-header"><h2>Files</h2></div>
                <div class="card text-danger">Failed to open database.</div>"#;
            return Html(templates::base("Files", "files", content));
        }
    };

    let conn = storage.conn();
    let mut stmt = match conn.prepare(
        "SELECT f.file_id, f.path, f.language, f.last_indexed_at, COUNT(r.region_id) as region_count \
         FROM files f LEFT JOIN regions r ON f.file_id = r.file_id \
         GROUP BY f.file_id ORDER BY f.path",
    ) {
        Ok(s) => s,
        Err(_) => {
            let content = r#"<div class="page-header"><h2>Files</h2></div>
                <div class="card text-danger">Query preparation failed.</div>"#;
            return Html(templates::base("Files", "files", content));
        }
    };

    #[allow(dead_code)]
    struct FileRow {
        file_id: i64,
        path: String,
        language: String,
        last_indexed_at: i64,
        region_count: i64,
    }

    let files: Vec<FileRow> = stmt
        .query_map([], |row| {
            Ok(FileRow {
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

    // Group files by directory prefix
    let mut by_dir: BTreeMap<String, Vec<&FileRow>> = BTreeMap::new();
    for f in &files {
        let dir = if let Some(pos) = f.path.rfind('/') {
            f.path[..pos].to_string()
        } else {
            ".".to_string()
        };
        by_dir.entry(dir).or_default().push(f);
    }

    // Build left panel (file tree)
    let mut file_list_html = String::new();
    file_list_html.push_str("<ul class=\"file-tree\">");
    for (dir, dir_files) in &by_dir {
        let dir_escaped = html_escape(dir);
        file_list_html.push_str(&format!(
            "<li class=\"dir-entry\"><span class=\"dir-name\">{dir_escaped}/</span><ul>"
        ));
        for f in dir_files {
            let filename = f.path.rfind('/').map(|p| &f.path[p+1..]).unwrap_or(&f.path);
            let filename_escaped = html_escape(filename);
            let lang = html_escape(&f.language);
            let lang_class = language_class(&f.language);
            file_list_html.push_str(&format!(
                "<li class=\"file-entry\" \
                    hx-get=\"/files/detail/{file_id}\" \
                    hx-target=\"#file-detail\" \
                    hx-swap=\"innerHTML\">\
                    <span class=\"file-name\">{filename_escaped}</span> \
                    <span class=\"badge {lang_class}\">{lang}</span>\
                </li>",
                file_id = f.file_id,
            ));
        }
        file_list_html.push_str("</ul></li>");
    }
    file_list_html.push_str("</ul>");

    let total = files.len();

    let content = format!(r#"
        <div class="page-header">
            <h2>Files</h2>
            <span class="text-muted text-sm">{total} files indexed</span>
        </div>
        <div class="files-layout">
            <!-- Left panel: file list -->
            <div class="files-sidebar">
                {file_list_html}
            </div>
            <!-- Right panel: file detail -->
            <div id="file-detail" class="files-detail">
                <div class="flex items-center justify-center text-muted empty-placeholder">
                    Select a file to view details
                </div>
            </div>
        </div>
    "#);

    let scripts = r#"
        <link rel="stylesheet" href="/assets/css/hljs-dark.css">
        <script src="/assets/js/highlight.min.js"></script>
    "#;

    Html(templates::base_with_scripts("Files", "files", &content, scripts))
}

pub async fn detail(
    State(state): State<SharedState>,
    Path(file_id): Path<i64>,
) -> Html<String> {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Html(r#"<div class="text-danger files-detail">Failed to open database.</div>"#.to_string()),
    };

    let file = match storage.get_file(file_id) {
        Ok(Some(f)) => f,
        Ok(None) => return Html(r#"<div class="text-danger files-detail">File not found.</div>"#.to_string()),
        Err(_) => return Html(r#"<div class="text-danger files-detail">Error loading file.</div>"#.to_string()),
    };

    let regions = storage.get_regions_for_file(file_id).unwrap_or_default();

    let filename = file.path.rfind('/').map(|p| &file.path[p+1..]).unwrap_or(&file.path);
    let path_escaped = html_escape(&file.path);
    let filename_escaped = html_escape(filename);
    let lang_escaped = html_escape(&file.language);
    let lang_class = language_class(&file.language);
    let date_str = format_timestamp(file.last_indexed_at);

    // Build regions table
    let mut regions_html = String::new();
    if regions.is_empty() {
        regions_html.push_str(r#"<p class="text-muted text-sm">No regions indexed for this file.</p>"#);
    } else {
        regions_html.push_str(r#"<table class="region-table"><thead><tr><th>Kind</th><th>Name</th><th>Lines</th><th>Signature</th></tr></thead><tbody>"#);
        for r in &regions {
            let kind_str = r.kind.as_str();
            let kind_class = format!("kind-{}", kind_str.replace(' ', "_"));
            let kind_escaped = html_escape(kind_str);
            let name_escaped = html_escape(&r.name);
            let sig_escaped = html_escape(&r.signature);
            regions_html.push_str(&format!(
                "<tr>\
                    <td><span class=\"kind-badge {kind_class}\">{kind_escaped}</span></td>\
                    <td class=\"fw-500\">{name_escaped}</td>\
                    <td class=\"text-muted font-mono nowrap\">{}-{}</td>\
                    <td class=\"text-muted font-mono text-sm truncate\">{sig_escaped}</td>\
                </tr>",
                r.start_line, r.end_line
            ));
        }
        regions_html.push_str("</tbody></table>");
    }

    // Try to load source from disk
    let full_path = state.project_root.join(&file.path);
    let source_html = match fs::read_to_string(&full_path) {
        Ok(content) => {
            let hljs_lang = hljs_language(&file.language);
            // Build line-numbered pre
            let mut lines_html = String::new();
            for line in content.lines() {
                let line_escaped = html_escape(line);
                lines_html.push_str(&format!("<span class=\"ln\">{line_escaped}\n</span>"));
            }
            format!(
                r#"<div class="source-container">
                    <div class="source-header">
                        <span class="text-muted text-sm">Source</span>
                        <span class="text-muted text-xs">{} lines</span>
                    </div>
                    <div class="source-scroll">
                        <pre><code class="language-{hljs_lang} line-numbers">{lines_html}</code></pre>
                    </div>
                </div>
                <script>
                    (function() {{
                        var codeEl = document.querySelector('.source-container code');
                        if (codeEl && window.hljs) {{ hljs.highlightElement(codeEl); }}
                    }})();
                </script>"#,
                content.lines().count(),
            )
        }
        Err(_) => {
            format!(
                r#"<div class="source-unavailable">
                    Source file not available on disk: <code class="text-danger">{path_escaped}</code>
                </div>"#
            )
        }
    };

    let html = format!(r#"
        <div class="flex justify-between items-center gap-1 mb-2">
            <div>
                <h3 class="mb-1">{filename_escaped}</h3>
                <div class="text-muted font-mono text-sm">{path_escaped}</div>
            </div>
            <div class="flex items-center gap-1 nowrap">
                <span class="badge {lang_class}">{lang_escaped}</span>
                <span class="text-muted text-xs">indexed {date_str}</span>
            </div>
        </div>
        <div class="mb-2">
            <div class="section-label mb-1">Regions ({count})</div>
            {regions_html}
        </div>
        {source_html}
    "#,
        count = regions.len(),
    );

    Html(html)
}
