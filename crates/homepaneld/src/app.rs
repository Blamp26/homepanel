use crate::{api, config::AppConfig, db, state::AppState, static_files};
use axum::Router;
use homepanel_core::error::Result;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

pub async fn run(config: AppConfig) -> Result<()> {
    std::fs::create_dir_all(&config.data.data_dir)?;
    let db = db::connect(&config.data.database_url).await?;
    let state = AppState::new(config.clone(), db);
    let router = router(state);
    let listener = TcpListener::bind(&config.server.bind).await?;
    axum::serve(listener, router.into_make_service())
        .await
        .map_err(|err| homepanel_core::error::ApiError::Io(err))?;
    Ok(())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(api::router().with_state(state))
        .layer(TraceLayer::new_for_http())
        .fallback_service(static_files::service())
}
