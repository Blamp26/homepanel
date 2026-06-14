use axum::{response::IntoResponse, Json};

pub async fn sources() -> impl IntoResponse {
    Json(serde_json::json!({"items": [
        {"kind": "journal_unit", "unit": "homepaneld.service"},
        {"kind": "terminal", "terminal_id": "example"}
    ]}))
}

pub async fn read() -> impl IntoResponse {
    Json(serde_json::json!({"content": ""}))
}

pub async fn follow_ws() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}
