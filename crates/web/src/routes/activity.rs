use axum::extract::{Query, State};
use axum::response::Html;
use crate::state::SharedState;
use crate::templates;
use codeindex_core::storage::sqlite::SqliteStorage;
use codeindex_core::model::ActivityEntry;

const PAGE_SIZE: usize = 50;

#[derive(serde::Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub offset: usize,
}

pub async fn page(State(state): State<SharedState>) -> Html<String> {
    let entries = load_entries(&state, 0);

    let table_rows = render_rows(&entries, 0);
    let load_more = if entries.len() >= PAGE_SIZE {
        format!(
            r##"<tr id="load-more-sentinel"
                    hx-get="/activity/more?offset={next}"
                    hx-trigger="revealed"
                    hx-swap="outerHTML">
                <td colspan="4" style="text-align:center;padding:12px;color:#8b949e;">Loading more…</td>
            </tr>"##,
            next = PAGE_SIZE
        )
    } else {
        String::new()
    };

    let content = format!(
        r##"
        <div class="page-header">
            <h2>Activity Log</h2>
        </div>
        <div class="card">
            <table style="width:100%;border-collapse:collapse;">
                <thead>
                    <tr style="border-bottom:1px solid #30363d;">
                        <th style="padding:10px 14px;text-align:left;color:#8b949e;font-size:0.85em;font-weight:500;">Time</th>
                        <th style="padding:10px 14px;text-align:left;color:#8b949e;font-size:0.85em;font-weight:500;">Event</th>
                        <th style="padding:10px 14px;text-align:left;color:#8b949e;font-size:0.85em;font-weight:500;">Detail</th>
                        <th style="padding:10px 14px;text-align:left;color:#8b949e;font-size:0.85em;font-weight:500;">Source</th>
                    </tr>
                </thead>
                <tbody id="activity-body">
                    {table_rows}
                    {load_more}
                </tbody>
            </table>
            {empty_msg}
        </div>
        <script>
        function relativeTime(ts) {{
            const now = Math.floor(Date.now() / 1000);
            const diff = now - ts;
            if (diff < 60) return diff + 's ago';
            if (diff < 3600) return Math.floor(diff / 60) + 'm ago';
            if (diff < 86400) return Math.floor(diff / 3600) + 'h ago';
            return Math.floor(diff / 86400) + 'd ago';
        }}
        document.querySelectorAll('[data-ts]').forEach(el => {{
            el.textContent = relativeTime(parseInt(el.dataset.ts, 10));
        }});
        </script>
        "##,
        table_rows = table_rows,
        load_more = load_more,
        empty_msg = if entries.is_empty() {
            r#"<div style="padding:24px;text-align:center;color:#8b949e;">No activity recorded yet. Run <code>codeindex index</code> to get started.</div>"#
        } else {
            ""
        },
    );

    Html(templates::base("Activity", "activity", &content))
}

pub async fn more(
    State(state): State<SharedState>,
    Query(params): Query<PaginationParams>,
) -> Html<String> {
    let offset = params.offset;
    let entries = load_entries(&state, offset);
    let rows = render_rows(&entries, offset);

    let load_more = if entries.len() >= PAGE_SIZE {
        let next = offset + PAGE_SIZE;
        format!(
            r##"<tr id="load-more-sentinel"
                    hx-get="/activity/more?offset={next}"
                    hx-trigger="revealed"
                    hx-swap="outerHTML">
                <td colspan="4" style="text-align:center;padding:12px;color:#8b949e;">Loading more…</td>
            </tr>"##,
            next = next
        )
    } else {
        String::new()
    };

    // Script to relativize newly injected timestamps
    let script = r#"<script>
    document.querySelectorAll('[data-ts]:not([data-rel])').forEach(el => {
        el.dataset.rel = '1';
        const now = Math.floor(Date.now() / 1000);
        const diff = now - parseInt(el.dataset.ts, 10);
        if (diff < 60) el.textContent = diff + 's ago';
        else if (diff < 3600) el.textContent = Math.floor(diff / 60) + 'm ago';
        else if (diff < 86400) el.textContent = Math.floor(diff / 3600) + 'h ago';
        else el.textContent = Math.floor(diff / 86400) + 'd ago';
    });
    </script>"#;

    Html(format!("{rows}{load_more}{script}"))
}

fn load_entries(state: &crate::state::AppState, offset: usize) -> Vec<ActivityEntry> {
    let storage = match SqliteStorage::open(&state.db_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    storage.list_activity(PAGE_SIZE, offset).unwrap_or_default()
}

fn render_rows(entries: &[ActivityEntry], _offset: usize) -> String {
    if entries.is_empty() {
        return String::new();
    }

    let mut html = String::new();
    for entry in entries {
        let event_type = html_escape(&entry.event_type);
        let detail = html_escape(&entry.detail);
        let source = html_escape(&entry.source);
        let badge_color = badge_color_for(&entry.event_type);

        html.push_str(&format!(
            r##"<tr style="border-bottom:1px solid #21262d;">
                <td style="padding:10px 14px;color:#8b949e;font-size:0.88em;white-space:nowrap;">
                    <span data-ts="{ts}">{ts}</span>
                </td>
                <td style="padding:10px 14px;">
                    <span style="padding:2px 8px;border-radius:4px;font-size:0.78em;background:{badge_bg};color:{badge_fg};border:1px solid {badge_border};">{event_type}</span>
                </td>
                <td style="padding:10px 14px;color:#c9d1d9;font-size:0.88em;font-family:monospace;max-width:420px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{detail}</td>
                <td style="padding:10px 14px;color:#8b949e;font-size:0.85em;">{source}</td>
            </tr>"##,
            ts = entry.timestamp,
            badge_bg = badge_color.0,
            badge_fg = badge_color.1,
            badge_border = badge_color.2,
            event_type = event_type,
            detail = detail,
            source = source,
        ));
    }
    html
}

/// Returns (background, foreground, border) CSS colours for a given event type.
fn badge_color_for(event_type: &str) -> (&'static str, &'static str, &'static str) {
    match event_type {
        "index" => ("#0d1117", "#3fb950", "#3fb950"),
        "gc" => ("#0d1117", "#f85149", "#f85149"),
        "config_change" => ("#0d1117", "#d29922", "#d29922"),
        "watch" => ("#0d1117", "#58a6ff", "#58a6ff"),
        _ => ("#21262d", "#8b949e", "#30363d"),
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
