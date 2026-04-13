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
                <div class="card" style="padding:20px;color:#f85149;">Failed to open database.</div>"#;
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
                <div class="card" style="padding:20px;color:#f85149;">Query preparation failed.</div>"#;
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
                    hx-swap=\"innerHTML\" \
                    style=\"cursor:pointer;\">\
                    <span class=\"file-name\">{filename_escaped}</span> \
                    <span class=\"badge {lang_class}\" style=\"font-size:0.72em;\">{lang}</span>\
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
            <span style="color:#8b949e;font-size:0.85em;">{total} files indexed</span>
        </div>
        <div style="display:flex;gap:0;height:calc(100vh - 120px);overflow:hidden;">
            <!-- Left panel: file list -->
            <div style="width:30%;min-width:220px;border-right:1px solid #30363d;overflow-y:auto;padding:12px 0;">
                {file_list_html}
            </div>
            <!-- Right panel: file detail -->
            <div id="file-detail" style="flex:1;overflow-y:auto;padding:20px;">
                <div style="display:flex;align-items:center;justify-content:center;height:200px;color:#8b949e;">
                    Select a file to view details
                </div>
            </div>
        </div>
    "#);

    let scripts = r#"
        <link rel="stylesheet" href="/assets/css/hljs-dark.css">
        <script src="/assets/js/highlight.min.js"></script>
        <style>
            .file-tree { list-style:none; padding:0; margin:0; }
            .file-tree ul { list-style:none; padding:0 0 0 12px; margin:0; }
            .dir-entry { margin-bottom:2px; }
            .dir-name { color:#58a6ff; font-size:0.82em; font-weight:600; padding:4px 12px; display:block; letter-spacing:0.2px; }
            .file-entry { padding:4px 12px 4px 16px; border-radius:4px; transition:background 0.12s; display:flex; align-items:center; gap:6px; }
            .file-entry:hover { background:rgba(255,255,255,0.06); }
            .file-entry.active { background:rgba(88,166,255,0.12); }
            .file-name { color:#c9d1d9; font-size:0.88em; flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
            .badge { display:inline-block; padding:1px 6px; border-radius:3px; font-size:0.75em; font-weight:500; border:1px solid #30363d; background:#21262d; color:#8b949e; }
            .badge-rust { background:rgba(222,165,132,0.15); color:#dea584; border-color:rgba(222,165,132,0.3); }
            .badge-python { background:rgba(53,142,200,0.15); color:#4ec9b0; border-color:rgba(53,142,200,0.3); }
            .badge-javascript, .badge-js { background:rgba(241,224,90,0.12); color:#f1e05a; border-color:rgba(241,224,90,0.25); }
            .badge-typescript, .badge-ts { background:rgba(43,116,137,0.15); color:#3178c6; border-color:rgba(49,120,198,0.3); }
            .badge-go { background:rgba(0,173,216,0.12); color:#00aed8; border-color:rgba(0,173,216,0.25); }
            .badge-other { background:#21262d; color:#8b949e; border-color:#30363d; }
            .region-table { width:100%; border-collapse:collapse; font-size:0.875em; }
            .region-table th { text-align:left; padding:8px 10px; color:#8b949e; border-bottom:1px solid #30363d; font-weight:500; }
            .region-table td { padding:7px 10px; border-bottom:1px solid #21262d; vertical-align:top; }
            .region-table tr:last-child td { border-bottom:none; }
            .region-table tr:hover td { background:rgba(255,255,255,0.03); }
            .kind-badge { display:inline-block; padding:1px 7px; border-radius:3px; font-size:0.78em; background:#21262d; color:#8b949e; border:1px solid #30363d; }
            .kind-function { background:rgba(88,166,255,0.12); color:#58a6ff; border-color:rgba(88,166,255,0.25); }
            .kind-struct, .kind-class { background:rgba(63,185,80,0.1); color:#3fb950; border-color:rgba(63,185,80,0.2); }
            .kind-impl_block { background:rgba(188,140,255,0.1); color:#bc8cff; border-color:rgba(188,140,255,0.2); }
            .kind-trait, .kind-interface { background:rgba(255,166,77,0.1); color:#ffa64d; border-color:rgba(255,166,77,0.2); }
            .source-container { margin-top:16px; border-radius:6px; overflow:hidden; border:1px solid #30363d; }
            .source-header { display:flex; justify-content:space-between; align-items:center; padding:8px 14px; background:#161b22; border-bottom:1px solid #30363d; }
            .line-numbers { counter-reset:line; }
            .line-numbers .ln { display:flex; }
            .line-numbers .ln::before { counter-increment:line; content:counter(line); display:inline-block; min-width:40px; padding-right:12px; color:#484f58; text-align:right; user-select:none; font-size:0.82em; flex-shrink:0; }
        </style>
    "#;

    Html(templates::base_with_scripts("Files", "files", &content, scripts))
}

pub async fn detail(
    State(state): State<SharedState>,
    Path(file_id): Path<i64>,
) -> Html<String> {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Html(r#"<div style="color:#f85149;padding:20px;">Failed to open database.</div>"#.to_string()),
    };

    let file = match storage.get_file(file_id) {
        Ok(Some(f)) => f,
        Ok(None) => return Html(r#"<div style="color:#f85149;padding:20px;">File not found.</div>"#.to_string()),
        Err(_) => return Html(r#"<div style="color:#f85149;padding:20px;">Error loading file.</div>"#.to_string()),
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
        regions_html.push_str(r#"<p style="color:#8b949e;font-size:0.9em;">No regions indexed for this file.</p>"#);
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
                    <td style=\"color:#e6edf3;font-weight:500;\">{name_escaped}</td>\
                    <td style=\"color:#8b949e;font-family:monospace;white-space:nowrap;\">{}-{}</td>\
                    <td style=\"color:#8b949e;font-family:monospace;font-size:0.82em;max-width:300px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;\">{sig_escaped}</td>\
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
                        <span style="color:#8b949e;font-size:0.85em;">Source</span>
                        <span style="color:#484f58;font-size:0.8em;">{} lines</span>
                    </div>
                    <div style="overflow:auto;max-height:500px;">
                        <pre style="margin:0;padding:12px 0;font-size:0.82em;line-height:1.6;background:#0d1117;"><code class="language-{hljs_lang} line-numbers">{lines_html}</code></pre>
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
                r#"<div style="margin-top:16px;padding:12px 16px;background:#161b22;border:1px solid #30363d;border-radius:6px;color:#8b949e;font-size:0.88em;">
                    Source file not available on disk: <code style="color:#f85149;">{path_escaped}</code>
                </div>"#
            )
        }
    };

    let html = format!(r#"
        <div style="margin-bottom:16px;display:flex;align-items:flex-start;justify-content:space-between;gap:12px;">
            <div>
                <h3 style="margin:0 0 4px;color:#e6edf3;font-size:1.05em;">{filename_escaped}</h3>
                <div style="color:#8b949e;font-size:0.82em;font-family:monospace;">{path_escaped}</div>
            </div>
            <div style="display:flex;align-items:center;gap:8px;flex-shrink:0;">
                <span class="badge {lang_class}">{lang_escaped}</span>
                <span style="color:#484f58;font-size:0.8em;">indexed {date_str}</span>
            </div>
        </div>
        <div style="margin-bottom:16px;">
            <div style="font-size:0.85em;font-weight:600;color:#8b949e;margin-bottom:8px;text-transform:uppercase;letter-spacing:0.5px;">Regions ({count})</div>
            {regions_html}
        </div>
        {source_html}
    "#,
        count = regions.len(),
    );

    Html(html)
}
