use axum::response::Html;
use crate::templates;

pub async fn page() -> Html<String> {
    let content = r#"
        <div class="page-header">
            <h2>Dependency Graph</h2>
            <div class="graph-toolbar">
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
        <div class="graph-wrap">
            <div id="cy"></div>
            <div id="node-detail" class="side-panel"></div>
        </div>
    "#;

    let scripts = r#"
        <script src="/assets/js/cytoscape.min.js"></script>
        <script src="/assets/js/graph.js"></script>
    "#;

    Html(templates::base_with_scripts("Graph", "graph", content, scripts))
}
