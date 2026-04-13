use axum::extract::State;
use axum::response::Html;
use crate::state::SharedState;
use crate::templates;

pub async fn index(State(_state): State<SharedState>) -> Html<String> {
    let content = r#"
        <div class="page-header">
            <h2>Dashboard</h2>
            <span id="last-indexed" class="text-muted" style="font-size:0.85em;"></span>
        </div>

        <!-- Stat Cards -->
        <div class="stat-cards">
            <div class="stat-card">
                <div class="label">Files Indexed</div>
                <div class="value" id="stat-files">—</div>
            </div>
            <div class="stat-card">
                <div class="label">Code Regions</div>
                <div class="value" id="stat-regions">—</div>
            </div>
            <div class="stat-card">
                <div class="label">Dependencies</div>
                <div class="value" id="stat-deps">—</div>
            </div>
            <div class="stat-card">
                <div class="label">Index Size</div>
                <div class="value" id="stat-size">—</div>
            </div>
        </div>

        <!-- Charts -->
        <div class="two-col">
            <div class="chart-container">
                <div class="card-header">
                    <h3 class="card-title">Language Breakdown</h3>
                </div>
                <div style="position:relative;height:220px;">
                    <canvas id="lang-chart"></canvas>
                </div>
            </div>
            <div class="chart-container">
                <div class="card-header">
                    <h3 class="card-title">Region Kinds</h3>
                </div>
                <div style="position:relative;height:220px;">
                    <canvas id="kind-chart"></canvas>
                </div>
            </div>
        </div>
    "#;

    let scripts = r#"
        <script src="/assets/js/chart.min.js"></script>
        <script src="/assets/js/dashboard.js"></script>
    "#;

    Html(templates::base_with_scripts("Dashboard", "dashboard", content, scripts))
}
