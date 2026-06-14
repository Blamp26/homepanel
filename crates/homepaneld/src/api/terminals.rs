use crate::state::AppState;
use axum::{
    extract::{Path, State, WebSocketUpgrade},
    http::HeaderMap,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures::{SinkExt, StreamExt};
use homepanel_core::{
    error::{ApiError, ApiErrorResponse},
    protocol::{ClientTerminalMessage, ServerTerminalMessage},
    types::{TerminalId, TerminalRequest, TerminalSummary},
};
use tracing::{debug, warn};

fn manager(state: &AppState) -> std::sync::Arc<homepanel_agent::pty::manager::TerminalSessionManager> {
    state.agent.manager()
}

pub async fn list(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let _ = headers;
    let terminals = manager(&state).list_terminals();
    debug!(
        terminal_count = terminals.len(),
        statuses = ?terminals.iter().map(|terminal| (&terminal.id.0, &terminal.status)).collect::<Vec<_>>(),
        "listed terminal summaries",
    );
    Json(terminals)
}

pub async fn create(State(state): State<AppState>, Json(body): Json<TerminalRequest>) -> impl IntoResponse {
    match manager(&state).create_terminal(body) {
        Ok(metadata) => {
            debug!(
                terminal_id = %metadata.id.0,
                status = ?metadata.status,
                "created terminal",
            );
            let _ = sqlx::query(
                "INSERT INTO terminal_sessions (id, name, kind, status, command, cwd, shell, cols, rows, auto_restart, exit_code, created_at, updated_at, last_attached_at, metadata_json) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&metadata.id.0)
            .bind(&metadata.name)
            .bind(serde_json::to_string(&metadata.kind).unwrap_or_default().trim_matches('"').to_string())
            .bind(serde_json::to_string(&metadata.status).unwrap_or_default().trim_matches('"').to_string())
            .bind(&metadata.command)
            .bind(metadata.cwd.to_string_lossy().to_string())
            .bind(&metadata.shell)
            .bind(i64::from(metadata.cols))
            .bind(i64::from(metadata.rows))
            .bind(i64::from(metadata.auto_restart as i32))
            .bind(metadata.exit_code)
            .bind(metadata.created_at)
            .bind(metadata.updated_at)
            .bind(metadata.last_attached_at)
            .bind(metadata.metadata_json.as_ref().map(|value| value.to_string()))
            .execute(&state.db)
            .await;
            Json(TerminalSummary::from(metadata)).into_response()
        }
        Err(err) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse::from(err)),
        )
            .into_response(),
    }
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let terminal_id = TerminalId(id);
    match manager(&state).get_terminal(&terminal_id) {
        Some(metadata) => Json(TerminalSummary::from(metadata)).into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::from(ApiError::NotFound)),
        )
            .into_response(),
    }
}

pub async fn update() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let terminal_id = TerminalId(id);
    match manager(&state).kill_terminal(&terminal_id) {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(err) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::from(err)),
        )
            .into_response(),
    }
}

pub async fn kill(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let terminal_id = TerminalId(id);
    match manager(&state).kill_terminal(&terminal_id) {
        Ok(()) => {
            debug!(terminal_id = %terminal_id.0, "persisting killed terminal");
            let _ = sqlx::query("UPDATE terminal_sessions SET status = ?, exit_code = ? WHERE id = ?")
                .bind("exited")
                .bind(0i32)
                .bind(&terminal_id.0)
                .execute(&state.db)
                .await;
            Json(serde_json::json!({"ok": true})).into_response()
        }
        Err(err) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::from(err)),
        )
            .into_response(),
    }
}

pub async fn restart() -> impl IntoResponse {
    (
        axum::http::StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({"error": "restart not implemented yet"})),
    )
        .into_response()
}

pub async fn clear_scrollback(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let terminal_id = TerminalId(id);
    match manager(&state).clear_scrollback(&terminal_id) {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(err) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::from(err)),
        )
            .into_response(),
    }
}

pub async fn scrollback(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let terminal_id = TerminalId(id);
    match manager(&state).attach_terminal(&terminal_id) {
        Ok(attachment) => {
            let data = STANDARD.encode(&attachment.scrollback);
            Json(serde_json::json!({"terminal_id": terminal_id.0, "data": data})).into_response()
        }
        Err(err) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::from(err)),
        )
            .into_response(),
    }
}

pub async fn ws(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    let terminal_id = TerminalId(id);
    match manager(&state).attach_terminal(&terminal_id) {
        Ok(attachment) => {
            debug!(terminal_id = %terminal_id.0, "websocket upgrade accepted");
            ws.on_upgrade(move |socket| async move {
                let _ = websocket_loop(state, terminal_id, attachment, socket).await;
            })
        }
        Err(err) => {
            warn!(
                terminal_id = %terminal_id.0,
                error = %err,
                "websocket upgrade rejected",
            );
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::from(err)),
            )
                .into_response()
        }
    }
}

async fn websocket_loop(
    state: AppState,
    terminal_id: TerminalId,
    attachment: homepanel_agent::pty::manager::TerminalAttachment,
    socket: axum::extract::ws::WebSocket,
) -> Result<(), ApiError> {
    let (mut sink, mut stream) = socket.split();

    let hello = serde_json::to_string(&ServerTerminalMessage::Hello {
        terminal_id: attachment.metadata.id.0.clone(),
        status: attachment.metadata.status.clone(),
    })?;
    sink.send(axum::extract::ws::Message::Text(hello))
        .await
        .map_err(|err| ApiError::Message {
            code: "ws_error",
            message: err.to_string(),
        })?;

    if !attachment.scrollback.is_empty() {
        let scrollback = serde_json::to_string(&ServerTerminalMessage::Scrollback {
            data: STANDARD.encode(&attachment.scrollback),
        })?;
        sink.send(axum::extract::ws::Message::Text(scrollback))
            .await
            .map_err(|err| ApiError::Message {
                code: "ws_error",
                message: err.to_string(),
            })?;
    }

    let mut receiver = attachment.receiver;
    loop {
        tokio::select! {
            Some(message) = stream.next() => {
                let message = message.map_err(|err| ApiError::Message { code: "ws_error", message: err.to_string() })?;
                match message {
                    axum::extract::ws::Message::Text(text) => {
                        if let Ok(client) = serde_json::from_str::<ClientTerminalMessage>(&text) {
                            if let Some(response) = manager(&state).handle_client_message(&terminal_id, client)? {
                                let encoded = serde_json::to_string(&response)?;
                                sink.send(axum::extract::ws::Message::Text(encoded))
                                    .await
                                    .map_err(|err| ApiError::Message { code: "ws_error", message: err.to_string() })?;
                            }
                        }
                    }
                    axum::extract::ws::Message::Close(frame) => {
                        debug!(terminal_id = %terminal_id.0, close = ?frame, "websocket closed by client");
                        break;
                    }
                    axum::extract::ws::Message::Ping(bytes) => {
                        sink.send(axum::extract::ws::Message::Pong(bytes))
                            .await
                            .map_err(|err| ApiError::Message { code: "ws_error", message: err.to_string() })?;
                    }
                    _ => {}
                }
            }
            Ok(bytes) = receiver.recv() => {
                if bytes.is_empty() {
                    let code = manager(&state)
                        .get_terminal(&terminal_id)
                        .and_then(|terminal| terminal.exit_code);
                    let exit = serde_json::to_string(&ServerTerminalMessage::Exit { code })?;
                    sink.send(axum::extract::ws::Message::Text(exit))
                        .await
                        .map_err(|err| ApiError::Message { code: "ws_error", message: err.to_string() })?;
                    debug!(terminal_id = %terminal_id.0, code = ?code, "pty exit received");
                    break;
                }
                let output = serde_json::to_string(&ServerTerminalMessage::Output {
                    data: STANDARD.encode(bytes),
                })?;
                sink.send(axum::extract::ws::Message::Text(output))
                    .await
                    .map_err(|err| ApiError::Message { code: "ws_error", message: err.to_string() })?;
            }
        }
    }

    debug!(terminal_id = %terminal_id.0, "websocket loop ended");
    Ok(())
}
