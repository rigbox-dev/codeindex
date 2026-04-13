mod state;
mod server;
mod routes;
mod templates;

use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;

pub async fn start_server(project_root: PathBuf, port: u16) -> Result<()> {
    let state = Arc::new(state::AppState::new(project_root)?);
    let app = server::build_router(state);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Dashboard at http://127.0.0.1:{}", port);
    axum::serve(listener, app).await?;
    Ok(())
}
