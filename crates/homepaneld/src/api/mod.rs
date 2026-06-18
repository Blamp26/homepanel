pub mod auth;
pub mod files;
pub mod logs;
pub mod settings;
pub mod services;
pub mod system;
pub mod terminals;

use axum::{routing::{get, post}, Router};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/auth/setup", post(auth::setup))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/status", get(auth::status))
        .route("/api/auth/me", get(auth::me))
        .route("/api/terminals", get(terminals::list).post(terminals::create))
        .route(
            "/api/terminals/:id",
            get(terminals::get).patch(terminals::update).delete(terminals::delete),
        )
        .route("/api/terminals/:id/kill", post(terminals::kill))
        .route("/api/terminals/:id/restart", post(terminals::restart))
        .route("/api/terminals/:id/clear-scrollback", post(terminals::clear_scrollback))
        .route("/api/terminals/:id/scrollback", get(terminals::scrollback))
        .route("/api/terminals/:id/ws", get(terminals::ws))
        .route("/api/system/summary", get(system::summary))
        .route("/api/system/disks", get(system::disks))
        .route("/api/system/network", get(system::network))
        .route("/api/system/processes", get(system::processes))
        .route("/api/system/metrics/ws", get(system::metrics_ws))
        .route("/api/files", get(files::list).delete(files::delete))
        .route("/api/files/preview", get(files::preview))
        .route("/api/files/mkdir", post(files::mkdir))
        .route("/api/files/rename", post(files::rename))
        .route("/api/files/upload", post(files::upload))
        .route("/api/files/download", get(files::download))
        .route("/api/services", get(services::list))
        .route("/api/services/:name", get(services::get))
        .route("/api/services/:name/start", post(services::start))
        .route("/api/services/:name/stop", post(services::stop))
        .route("/api/services/:name/restart", post(services::restart))
        .route("/api/services/:name/logs", get(services::logs))
        .route("/api/logs/sources", get(logs::sources))
        .route("/api/logs/read", get(logs::read))
        .route("/api/logs/follow/ws", get(logs::follow_ws))
        .route("/api/settings", get(settings::get).patch(settings::update))
}
