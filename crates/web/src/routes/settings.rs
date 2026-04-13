use axum::extract::State;
use axum::response::Html;
use crate::state::SharedState;
use crate::templates;
use codeindex_core::config::Config;

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

pub async fn page(State(state): State<SharedState>) -> Html<String> {
    let config = Config::load(&state.project_root).unwrap_or_default();

    let embedding_model = html_escape(config.embedding.model.as_deref().unwrap_or(""));
    let summary_enabled = if config.summary.enabled { "checked" } else { "" };
    let summary_model = html_escape(&config.summary.model);
    let query_default_top = config.query.default_top;
    let query_default_depth = config.query.default_depth;
    let daemon_debounce_ms = config.daemon.debounce_ms;
    let git_hooks = if config.index.git_hooks { "checked" } else { "" };

    let provider_local_selected = if config.embedding.provider == "local" { "selected" } else { "" };
    let provider_voyage_selected = if config.embedding.provider == "voyage" { "selected" } else { "" };

    let content = format!(
        r##"
        <div class="page-header">
            <h2>Settings</h2>
        </div>
        <div id="save-status" style="margin-bottom:12px;min-height:24px;"></div>
        <form id="settings-form"
              hx-put="/api/config"
              hx-target="#save-status"
              hx-swap="innerHTML">

            <!-- Embedding Section -->
            <div class="card" style="margin-bottom:20px;">
                <div class="card-header">
                    <h3 class="card-title">Embedding</h3>
                </div>
                <div style="padding:16px;display:grid;gap:12px;">
                    <div>
                        <label style="display:block;margin-bottom:4px;color:#8b949e;font-size:0.87em;">Provider</label>
                        <select name="embedding.provider" style="width:100%;padding:8px 10px;background:#161b22;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-size:0.95em;">
                            <option value="local" {provider_local_selected}>local (ONNX)</option>
                            <option value="voyage" {provider_voyage_selected}>voyage</option>
                        </select>
                    </div>
                    <div>
                        <label style="display:block;margin-bottom:4px;color:#8b949e;font-size:0.87em;">Model (optional)</label>
                        <input type="text" name="embedding.model" value="{embedding_model}"
                               placeholder="leave blank for default"
                               style="width:100%;box-sizing:border-box;padding:8px 10px;background:#161b22;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-size:0.95em;" />
                    </div>
                </div>
            </div>

            <!-- Summary Section -->
            <div class="card" style="margin-bottom:20px;">
                <div class="card-header">
                    <h3 class="card-title">Summary</h3>
                </div>
                <div style="padding:16px;display:grid;gap:12px;">
                    <div style="display:flex;align-items:center;gap:10px;">
                        <input type="checkbox" name="summary.enabled" id="summary-enabled" value="true" {summary_enabled}
                               style="width:16px;height:16px;cursor:pointer;" />
                        <label for="summary-enabled" style="color:#c9d1d9;font-size:0.95em;cursor:pointer;">Enable summaries</label>
                    </div>
                    <div>
                        <label style="display:block;margin-bottom:4px;color:#8b949e;font-size:0.87em;">Model</label>
                        <input type="text" name="summary.model" value="{summary_model}"
                               style="width:100%;box-sizing:border-box;padding:8px 10px;background:#161b22;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-size:0.95em;" />
                    </div>
                </div>
            </div>

            <!-- Query Section -->
            <div class="card" style="margin-bottom:20px;">
                <div class="card-header">
                    <h3 class="card-title">Query</h3>
                </div>
                <div style="padding:16px;display:grid;grid-template-columns:1fr 1fr;gap:12px;">
                    <div>
                        <label style="display:block;margin-bottom:4px;color:#8b949e;font-size:0.87em;">Default top results</label>
                        <input type="number" name="query.default_top" value="{query_default_top}" min="1" max="100"
                               style="width:100%;box-sizing:border-box;padding:8px 10px;background:#161b22;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-size:0.95em;" />
                    </div>
                    <div>
                        <label style="display:block;margin-bottom:4px;color:#8b949e;font-size:0.87em;">Default graph depth</label>
                        <input type="number" name="query.default_depth" value="{query_default_depth}" min="0" max="5"
                               style="width:100%;box-sizing:border-box;padding:8px 10px;background:#161b22;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-size:0.95em;" />
                    </div>
                </div>
            </div>

            <!-- Daemon Section -->
            <div class="card" style="margin-bottom:20px;">
                <div class="card-header">
                    <h3 class="card-title">Daemon</h3>
                </div>
                <div style="padding:16px;display:grid;gap:12px;">
                    <div>
                        <label style="display:block;margin-bottom:4px;color:#8b949e;font-size:0.87em;">Debounce (ms)</label>
                        <input type="number" name="daemon.debounce_ms" value="{daemon_debounce_ms}" min="0"
                               style="width:100%;box-sizing:border-box;padding:8px 10px;background:#161b22;border:1px solid #30363d;border-radius:6px;color:#c9d1d9;font-size:0.95em;" />
                    </div>
                </div>
            </div>

            <!-- Index Section -->
            <div class="card" style="margin-bottom:20px;">
                <div class="card-header">
                    <h3 class="card-title">Index</h3>
                </div>
                <div style="padding:16px;display:grid;gap:12px;">
                    <div style="display:flex;align-items:center;gap:10px;">
                        <input type="checkbox" name="index.git_hooks" id="git-hooks" value="true" {git_hooks}
                               style="width:16px;height:16px;cursor:pointer;" />
                        <label for="git-hooks" style="color:#c9d1d9;font-size:0.95em;cursor:pointer;">Install git hooks (auto-index on commit)</label>
                    </div>
                </div>
            </div>

            <div style="display:flex;gap:12px;align-items:center;">
                <button type="submit"
                        style="padding:8px 20px;background:#1f6feb;border:none;border-radius:6px;color:#fff;font-size:0.95em;cursor:pointer;font-weight:500;">
                    Save Settings
                </button>
            </div>
        </form>

        <script>
        // Intercept the HTMX form submit so we serialise correctly (including checkbox state)
        document.getElementById('settings-form').addEventListener('htmx:configRequest', function(evt) {{
            const form = document.getElementById('settings-form');
            const fd = new FormData(form);
            const body = {{}};

            // Embedding
            body['embedding'] = {{}};
            body['embedding']['provider'] = fd.get('embedding.provider') || 'local';
            const embModel = fd.get('embedding.model') || '';
            body['embedding']['model'] = embModel === '' ? null : embModel;

            // Summary
            body['summary'] = {{}};
            body['summary']['enabled'] = fd.has('summary.enabled');
            body['summary']['model'] = fd.get('summary.model') || '';
            body['summary']['provider'] = 'anthropic';
            body['summary']['api_key_env'] = 'ANTHROPIC_API_KEY';
            body['summary']['batch_size'] = 20;
            body['summary']['interval_seconds'] = 30;

            // Query
            body['query'] = {{}};
            body['query']['default_top'] = parseInt(fd.get('query.default_top') || '5', 10);
            body['query']['default_depth'] = parseInt(fd.get('query.default_depth') || '1', 10);
            body['query']['enhance'] = false;

            // Daemon
            body['daemon'] = {{}};
            body['daemon']['debounce_ms'] = parseInt(fd.get('daemon.debounce_ms') || '500', 10);
            body['daemon']['max_concurrent_embeds'] = 4;
            body['daemon']['respect_gitignore'] = true;
            body['daemon']['extra_ignore'] = [];

            // Index
            body['index'] = {{}};
            body['index']['path'] = '.codeindex';
            body['index']['git_hooks'] = fd.has('index.git_hooks');

            // Languages (preserve from current config — not editable in UI yet)
            body['languages'] = {languages_json};

            evt.detail.parameters = body;
            evt.detail.headers['Content-Type'] = 'application/json';
        }});
        </script>
        "##,
        embedding_model = embedding_model,
        provider_local_selected = provider_local_selected,
        provider_voyage_selected = provider_voyage_selected,
        summary_enabled = summary_enabled,
        summary_model = summary_model,
        query_default_top = query_default_top,
        query_default_depth = query_default_depth,
        daemon_debounce_ms = daemon_debounce_ms,
        git_hooks = git_hooks,
        languages_json = serde_json::to_string(&config.languages).unwrap_or_else(|_| "[]".to_string()),
    );

    Html(templates::base("Settings", "settings", &content))
}
