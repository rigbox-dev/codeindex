use axum::extract::State;
use axum::response::Html;
use crate::state::SharedState;
use crate::templates;

pub async fn index(State(state): State<SharedState>) -> Html<String> {
    let content = format!(r#"
        <h2>Dashboard</h2>
        <p>Index at <code>{}</code></p>
        <p>More metrics coming soon.</p>
    "#, state.index_dir.display());
    Html(templates::base("Dashboard", "dashboard", &content))
}
