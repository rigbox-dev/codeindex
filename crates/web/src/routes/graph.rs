use axum::response::Html;
use crate::templates;

pub async fn page() -> Html<String> {
    let content = r#"
        <div class="page-header">
            <h2>Dependency Graph</h2>
            <div class="graph-toolbar">
                <div class="graph-toggle">
                    <button class="graph-toggle-btn active" onclick="showGraph('cytoscape', event)">Cytoscape</button>
                    <button class="graph-toggle-btn" onclick="showGraph('reactflow', event)">React Flow</button>
                </div>
                <div id="cy-controls" style="display:flex;align-items:center;gap:8px;">
                    <label class="text-muted" for="layout-select">Layout:</label>
                    <select id="layout-select">
                        <option value="cose">Force-directed</option>
                        <option value="grid">Grid</option>
                        <option value="circle">Circle</option>
                        <option value="breadthfirst">Breadth-first</option>
                    </select>
                    <button id="fit-btn">Fit</button>
                </div>
            </div>
        </div>

        <div id="graph-cytoscape" style="display:block">
            <div class="graph-wrap">
                <div id="cy"></div>
                <div id="node-detail" class="side-panel"></div>
            </div>
        </div>

        <div id="graph-reactflow" style="display:none">
            <iframe id="rf-iframe" src="" style="width:100%;height:calc(100vh - 140px);border:none;border-radius:8px;" loading="lazy"></iframe>
        </div>
    "#;

    let scripts = r#"
        <script src="/assets/js/cytoscape.min.js"></script>
        <script src="/assets/js/graph.js"></script>
        <script>
        function showGraph(which, event) {
            const isCyto = which === 'cytoscape';
            document.getElementById('graph-cytoscape').style.display = isCyto ? 'block' : 'none';
            document.getElementById('graph-reactflow').style.display = isCyto ? 'none' : 'block';
            document.getElementById('cy-controls').style.display = isCyto ? 'flex' : 'none';

            // Lazy-load the iframe only when first switching to React Flow
            if (!isCyto) {
                const iframe = document.getElementById('rf-iframe');
                if (!iframe.src || iframe.src === window.location.href) {
                    iframe.src = '/assets/graph-app/index.html';
                }
            }

            document.querySelectorAll('.graph-toggle-btn').forEach(b => b.classList.remove('active'));
            if (event && event.target) event.target.classList.add('active');
        }
        </script>
    "#;

    Html(templates::base_with_scripts("Graph", "graph", content, scripts))
}
