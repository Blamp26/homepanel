use axum::{response::IntoResponse, Json};

pub async fn list() -> impl IntoResponse {
    Json(serde_json::json!({"items": []}))
}

pub async fn get() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}

pub async fn start() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}

pub async fn stop() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}

pub async fn restart() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}

pub async fn logs() -> impl IntoResponse {
    Json(serde_json::json!({"items": []}))
}
