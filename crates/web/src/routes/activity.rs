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
                <td colspan="4" class="text-muted" style="text-align:center;padding:12px;">Loading more…</td>
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
            <table>
                <thead>
                    <tr>
                        <th>Time</th>
                        <th>Event</th>
                        <th>Detail</th>
                        <th>Source</th>
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
            r#"<div class="empty-state">No activity recorded yet. Run <code>codeindex index</code> to get started.</div>"#
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
                <td colspan="4" class="text-muted" style="text-align:center;padding:12px;">Loading more…</td>
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
        let badge_class = badge_class_for(&entry.event_type);

        html.push_str(&format!(
            r##"<tr>
                <td class="text-muted font-mono" style="white-space:nowrap;">
                    <span data-ts="{ts}">{ts}</span>
                </td>
                <td>
                    <span class="{badge_class}">{event_type}</span>
                </td>
                <td class="font-mono" style="max-width:420px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{detail}</td>
                <td class="text-muted">{source}</td>
            </tr>"##,
            ts = entry.timestamp,
            badge_class = badge_class,
            event_type = event_type,
            detail = detail,
            source = source,
        ));
    }
    html
}

/// Returns the CSS class string for a badge matching the given event type.
fn badge_class_for(event_type: &str) -> &'static str {
    match event_type {
        "index" => "badge badge-index",
        "gc" => "badge badge-gc",
        "config_change" => "badge badge-config",
        "watch" => "badge badge-watch",
        _ => "badge",
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
