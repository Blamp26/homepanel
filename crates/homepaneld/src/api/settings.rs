use axum::{response::IntoResponse, Json};

pub async fn get() -> impl IntoResponse {
    Json(serde_json::json!({"settings": {}}))
}

pub async fn update() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}
