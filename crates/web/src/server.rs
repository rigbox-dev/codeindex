use axum::{Router, routing::get, response::IntoResponse, http::{header, StatusCode, Uri}};
use rust_embed::Embed;
use std::sync::Arc;
use crate::state::AppState;
use crate::routes;

#[derive(Embed)]
#[folder = "src/assets/"]
struct Assets;

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches("/assets/");
    match Assets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_text_plain();
            (StatusCode::OK, [(header::CONTENT_TYPE, mime.to_string())], content.data.to_vec()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(routes::dashboard::index))
        .route("/api/stats", get(routes::api::stats))
        .route("/search", get(routes::search::page))
        .route("/search/results", get(routes::search::results))
        .route("/api/search", get(routes::api::search))
        .route("/graph", get(routes::graph::page))
        .route("/api/graph", get(routes::api::graph_data))
        .route("/api/graph/node/:id", get(routes::api::graph_node_detail))
        .route("/files", get(routes::files::page))
        .route("/files/detail/:id", get(routes::files::detail))
        .route("/api/files", get(routes::api::list_files))
        .route("/api/files/:id", get(routes::api::get_file_detail))
        .route("/api/files/:id/source", get(routes::api::get_file_source))
        .route("/settings", get(routes::settings::page))
        .route("/activity", get(routes::activity::page))
        .route("/activity/more", get(routes::activity::more))
        .route("/api/config", get(routes::api::get_config).put(routes::api::update_config))
        .route("/api/activity", get(routes::api::list_activity))
        .route("/assets/{*path}", get(static_handler))
        .with_state(state)
}
