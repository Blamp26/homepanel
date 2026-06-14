use crate::state::AppState;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use homepanel_core::{
    error::{ApiError, ApiErrorResponse},
    types::FileAllowlist,
};
use serde::Deserialize;
use std::{fs, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct PathQuery {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct WriteRequest {
    pub path: String,
    pub content: String,
}

fn allowlist(state: &AppState) -> FileAllowlist {
    FileAllowlist {
        allowed_paths: state.config.files.allowed_paths.clone(),
    }
}

fn ensure_allowed(state: &AppState, path: &str) -> Result<PathBuf, ApiError> {
    let path = PathBuf::from(path);
    if allowlist(state).is_allowed(&path) {
        Ok(path)
    } else {
        Err(ApiError::Forbidden)
    }
}

pub async fn list(State(state): State<AppState>, Query(query): Query<PathQuery>) -> impl IntoResponse {
    let Ok(path) = ensure_allowed(&state, &query.path) else {
        return (axum::http::StatusCode::FORBIDDEN, Json(ApiErrorResponse::from(ApiError::Forbidden))).into_response();
    };
    match fs::read_dir(path) {
        Ok(entries) => {
            let files: Vec<_> = entries
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let metadata = entry.metadata().ok();
                    serde_json::json!({
                        "name": entry.file_name().to_string_lossy(),
                        "is_dir": metadata.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                        "size": metadata.as_ref().map(|m| m.len()).unwrap_or(0)
                    })
                })
                .collect();
            Json(serde_json::json!({"entries": files})).into_response()
        }
        Err(err) => (axum::http::StatusCode::BAD_REQUEST, Json(ApiErrorResponse::from(ApiError::Io(err)))).into_response(),
    }
}

pub async fn read(State(state): State<AppState>, Query(query): Query<PathQuery>) -> impl IntoResponse {
    let Ok(path) = ensure_allowed(&state, &query.path) else {
        return (axum::http::StatusCode::FORBIDDEN, Json(ApiErrorResponse::from(ApiError::Forbidden))).into_response();
    };
    match fs::read_to_string(path) {
        Ok(content) => Json(serde_json::json!({"content": content})).into_response(),
        Err(err) => (axum::http::StatusCode::BAD_REQUEST, Json(ApiErrorResponse::from(ApiError::Io(err)))).into_response(),
    }
}

pub async fn write(State(state): State<AppState>, Json(body): Json<WriteRequest>) -> impl IntoResponse {
    let Ok(path) = ensure_allowed(&state, &body.path) else {
        return (axum::http::StatusCode::FORBIDDEN, Json(ApiErrorResponse::from(ApiError::Forbidden))).into_response();
    };
    match fs::write(path, body.content) {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(err) => (axum::http::StatusCode::BAD_REQUEST, Json(ApiErrorResponse::from(ApiError::Io(err)))).into_response(),
    }
}

pub async fn mkdir(State(state): State<AppState>, Json(body): Json<PathQuery>) -> impl IntoResponse {
    let Ok(path) = ensure_allowed(&state, &body.path) else {
        return (axum::http::StatusCode::FORBIDDEN, Json(ApiErrorResponse::from(ApiError::Forbidden))).into_response();
    };
    match fs::create_dir_all(path) {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(err) => (axum::http::StatusCode::BAD_REQUEST, Json(ApiErrorResponse::from(ApiError::Io(err)))).into_response(),
    }
}

pub async fn rename(State(state): State<AppState>, Json(body): Json<WriteRequest>) -> impl IntoResponse {
    let Ok(path) = ensure_allowed(&state, &body.path) else {
        return (axum::http::StatusCode::FORBIDDEN, Json(ApiErrorResponse::from(ApiError::Forbidden))).into_response();
    };
    match fs::rename(path, body.content) {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(err) => (axum::http::StatusCode::BAD_REQUEST, Json(ApiErrorResponse::from(ApiError::Io(err)))).into_response(),
    }
}

pub async fn delete(State(state): State<AppState>, Query(query): Query<PathQuery>) -> impl IntoResponse {
    let Ok(path) = ensure_allowed(&state, &query.path) else {
        return (axum::http::StatusCode::FORBIDDEN, Json(ApiErrorResponse::from(ApiError::Forbidden))).into_response();
    };
    match fs::remove_file(path) {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(err) => (axum::http::StatusCode::BAD_REQUEST, Json(ApiErrorResponse::from(ApiError::Io(err)))).into_response(),
    }
}

pub async fn upload() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}

pub async fn download() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}
