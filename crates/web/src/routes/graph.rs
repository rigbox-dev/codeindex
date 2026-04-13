use axum::response::Html;
use crate::templates;

pub async fn page() -> Html<String> {
    let content = r#"
        <div class="page-header" style="display:flex;align-items:center;gap:12px;padding-bottom:8px;">
            <h2 style="margin:0;">Dependency Graph</h2>
            <div style="display:flex;align-items:center;gap:8px;margin-left:auto;">
                <label style="color:#8b949e;font-size:0.85em;" for="layout-select">Layout:</label>
                <select id="layout-select" style="background:#21262d;border:1px solid #30363d;color:#e6edf3;padding:4px 8px;border-radius:4px;font-size:0.85em;cursor:pointer;">
                    <option value="cose">Force-directed</option>
                    <option value="grid">Grid</option>
                    <option value="circle">Circle</option>
                    <option value="breadthfirst">Breadth-first</option>
                </select>
                <button id="fit-btn" style="background:#21262d;border:1px solid #30363d;color:#e6edf3;padding:4px 12px;border-radius:4px;font-size:0.85em;cursor:pointer;">Fit</button>
            </div>
        </div>
        <div style="position:relative;display:flex;height:calc(100vh - 100px);">
            <div id="cy" style="flex:1;background:#0d1117;border:1px solid #30363d;border-radius:6px;"></div>
            <div id="node-detail" class="side-panel"></div>
        </div>
    "#;

    let scripts = r#"
        <style>
            .page-header { border-bottom: 1px solid #30363d; margin-bottom: 12px; }
            .side-panel {
                position: absolute;
                right: 0;
                top: 0;
                bottom: 0;
                width: 340px;
                background: #161b22;
                border-left: 1px solid #30363d;
                border-radius: 0 6px 6px 0;
                overflow-y: auto;
                transform: translateX(100%);
                transition: transform 0.2s ease;
                z-index: 10;
                color: #e6edf3;
            }
            .side-panel.open {
                transform: translateX(0);
            }
            .side-panel h3 { color: #e6edf3; font-size: 1em; }
            .side-panel h4 { color: #8b949e; font-size: 0.85em; text-transform: uppercase; letter-spacing: 0.5px; margin: 12px 0 6px; }
            .side-panel pre { background: #0d1117; border: 1px solid #30363d; border-radius: 4px; padding: 8px; overflow-x: auto; color: #e6edf3; }
            .side-panel ul { padding-left: 16px; margin: 4px 0; }
            .side-panel li { font-size: 0.85em; margin-bottom: 4px; color: #c9d1d9; }
            .badge { display:inline-block; padding:1px 6px; border-radius:3px; font-size:0.75em; font-weight:500; border:1px solid #30363d; background:#21262d; color:#8b949e; }
            .badge-function { background:rgba(88,166,255,0.12); color:#58a6ff; border-color:rgba(88,166,255,0.25); }
            .badge-struct { background:rgba(63,185,80,0.1); color:#3fb950; border-color:rgba(63,185,80,0.2); }
            .badge-class { background:rgba(188,140,255,0.1); color:#bc8cff; border-color:rgba(188,140,255,0.2); }
            .badge-method { background:rgba(210,153,34,0.12); color:#d29922; border-color:rgba(210,153,34,0.25); }
            .badge-interface { background:rgba(248,81,73,0.1); color:#f85149; border-color:rgba(248,81,73,0.2); }
            .badge-module, .badge-impl_block, .badge-enum { background:#21262d; color:#8b949e; border-color:#30363d; }
        </style>
        <script src="/assets/js/cytoscape.min.js"></script>
        <script src="/assets/js/graph.js"></script>
    "#;

    Html(templates::base_with_scripts("Graph", "graph", content, scripts))
}
