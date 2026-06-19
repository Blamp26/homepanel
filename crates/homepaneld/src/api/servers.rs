use crate::{db::models::ServerRow, state::AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use homepanel_core::error::{ApiError, ApiErrorResponse};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    io::{BufRead, BufReader},
    path::{Path as FsPath, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::AsyncReadExt,
    process::Command,
    time::timeout,
};
use tracing::debug;
use uuid::Uuid;

const DEFAULT_LOG_LINES: usize = 200;
const MAX_LOG_LINES: usize = 1000;
const SCRIPT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Debug, Serialize, Clone)]
pub struct ServerStatusSummary {
    pub state: String,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ServerRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub working_dir: Option<String>,
    pub start_script: Option<String>,
    pub stop_script: Option<String>,
    pub restart_script: Option<String>,
    pub log_type: Option<String>,
    pub log_path: Option<String>,
    pub log_unit: Option<String>,
    pub status_type: Option<String>,
    pub status_value: Option<String>,
    pub status: ServerStatusSummary,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ServerListResponse {
    pub items: Vec<ServerRecord>,
}

#[derive(Debug, Serialize)]
pub struct ServerActionResult {
    pub ok: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize)]
pub struct ServerLogsResponse {
    pub items: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerCreateRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerUpdateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub working_dir: Option<String>,
    pub start_script: Option<String>,
    pub stop_script: Option<String>,
    pub restart_script: Option<String>,
    pub log_type: Option<String>,
    pub log_path: Option<String>,
    pub log_unit: Option<String>,
    pub status_type: Option<String>,
    pub status_value: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerLogsQuery {
    pub lines: Option<usize>,
}

fn api_error_response(err: ApiError) -> Response {
    let status = match &err {
        ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
        ApiError::Forbidden => StatusCode::FORBIDDEN,
        ApiError::NotFound => StatusCode::NOT_FOUND,
        ApiError::BadRequest(_) | ApiError::Message { .. } | ApiError::Json(_) => {
            StatusCode::BAD_REQUEST
        }
        ApiError::Io(io) => match io.kind() {
            std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
            std::io::ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
            _ => StatusCode::BAD_REQUEST,
        },
        ApiError::Config(_) | ApiError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(ApiErrorResponse::from(err))).into_response()
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn validate_name(name: &str) -> Result<String, ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(ApiError::bad_request("name is required"));
    }
    Ok(trimmed.to_string())
}

fn validate_log_type(value: Option<String>) -> Result<Option<String>, ApiError> {
    let value = normalize_optional_text(value);
    match value.as_deref() {
        None => Ok(None),
        Some("file") | Some("journal") => Ok(value),
        Some(other) => Err(ApiError::bad_request(format!(
            "unsupported log type `{other}`"
        ))),
    }
}

fn validate_status_type(value: Option<String>) -> Result<Option<String>, ApiError> {
    let value = normalize_optional_text(value);
    match value.as_deref() {
        None => Ok(None),
        Some("manual") | Some("process") | Some("systemd") | Some("tcp") | Some("http") => {
            Ok(value)
        }
        Some(other) => Err(ApiError::bad_request(format!(
            "unsupported status type `{other}`"
        ))),
    }
}

fn normalize_log_lines(requested: Option<usize>) -> usize {
    requested.unwrap_or(DEFAULT_LOG_LINES).clamp(1, MAX_LOG_LINES)
}

fn ensure_absolute_file(path: &FsPath, label: &str) -> Result<(), ApiError> {
    if !path.is_absolute() {
        return Err(ApiError::bad_request(format!("{label} must be absolute")));
    }

    let metadata = fs::metadata(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => {
            ApiError::bad_request(format!("{label} does not exist: {}", path.display()))
        }
        std::io::ErrorKind::PermissionDenied => ApiError::Forbidden,
        _ => ApiError::Io(err),
    })?;

    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "{label} must point to a file: {}",
            path.display()
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(ApiError::bad_request(format!(
                "{label} is not executable: {}",
                path.display()
            )));
        }
    }

    Ok(())
}

fn ensure_absolute_directory(path: &FsPath, label: &str) -> Result<(), ApiError> {
    if !path.is_absolute() {
        return Err(ApiError::bad_request(format!("{label} must be absolute")));
    }

    let metadata = fs::metadata(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => {
            ApiError::bad_request(format!("{label} does not exist: {}", path.display()))
        }
        std::io::ErrorKind::PermissionDenied => ApiError::Forbidden,
        _ => ApiError::Io(err),
    })?;

    if !metadata.is_dir() {
        return Err(ApiError::bad_request(format!(
            "{label} must point to a directory: {}",
            path.display()
        )));
    }

    Ok(())
}

fn read_task(
    pipe: Option<impl tokio::io::AsyncRead + Unpin + Send + 'static>,
) -> tokio::task::JoinHandle<Result<Vec<u8>, std::io::Error>> {
    tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut reader) = pipe {
            reader.read_to_end(&mut bytes).await?;
        }
        Ok(bytes)
    })
}

async fn run_script(
    script_path: &str,
    working_dir: Option<&str>,
    action: &str,
) -> Result<ServerActionResult, ApiError> {
    let script_path = PathBuf::from(script_path);
    ensure_absolute_file(&script_path, &format!("{action} script"))?;

    if let Some(dir) = working_dir {
        ensure_absolute_directory(FsPath::new(dir), "working directory")?;
    }

    let mut command = Command::new(&script_path);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = working_dir {
        command.current_dir(dir);
    }

    let mut child = command
        .spawn()
        .map_err(|err| ApiError::message("script_spawn_failed", err.to_string()))?;
    let stdout_task = read_task(child.stdout.take());
    let stderr_task = read_task(child.stderr.take());

    let status = match timeout(Duration::from_secs(SCRIPT_TIMEOUT_SECONDS), child.wait()).await {
        Ok(result) => {
            result.map_err(|err| ApiError::message("script_wait_failed", err.to_string()))?
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(ApiError::message(
                "script_timeout",
                format!("{action} script timed out after {SCRIPT_TIMEOUT_SECONDS} seconds"),
            ));
        }
    };

    let stdout = stdout_task
        .await
        .map_err(|err| ApiError::message("script_stdout_failed", err.to_string()))?
        .map_err(ApiError::Io)?;
    let stderr = stderr_task
        .await
        .map_err(|err| ApiError::message("script_stderr_failed", err.to_string()))?
        .map_err(ApiError::Io)?;

    Ok(ServerActionResult {
        ok: status.success(),
        exit_code: status.code(),
        stdout: String::from_utf8_lossy(&stdout).trim_end().to_string(),
        stderr: String::from_utf8_lossy(&stderr).trim_end().to_string(),
    })
}

fn merge_action_results(first: ServerActionResult, second: ServerActionResult) -> ServerActionResult {
    let stdout = [first.stdout, second.stdout]
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    let stderr = [first.stderr, second.stderr]
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    ServerActionResult {
        ok: first.ok && second.ok,
        exit_code: second.exit_code.or(first.exit_code),
        stdout,
        stderr,
    }
}

async fn read_journal(unit: &str, limit: usize) -> Result<Vec<String>, ApiError> {
    let output = Command::new("journalctl")
        .args([
            "--no-pager",
            "--no-hostname",
            "--output=short-iso",
            "--unit",
            unit,
            "-n",
            &limit.to_string(),
        ])
        .output()
        .await
        .map_err(ApiError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!("journalctl exited with status {}", output.status)
        } else {
            stderr
        };
        return Err(ApiError::message("journalctl_failed", message));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.to_string())
        .collect())
}

fn tail_file_lines(path: &FsPath, limit: usize) -> Result<Vec<String>, ApiError> {
    if !path.is_absolute() {
        return Err(ApiError::bad_request("log file path must be absolute"));
    }

    let file = fs::File::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => {
            ApiError::bad_request(format!("log file does not exist: {}", path.display()))
        }
        std::io::ErrorKind::PermissionDenied => ApiError::Forbidden,
        _ => ApiError::Io(err),
    })?;
    let metadata = file.metadata().map_err(ApiError::Io)?;
    if !metadata.is_file() {
        return Err(ApiError::bad_request(format!(
            "log file path must point to a file: {}",
            path.display()
        )));
    }

    let reader = BufReader::new(file);
    let mut lines = VecDeque::with_capacity(limit.saturating_add(1));
    for line in reader.lines() {
        let line = line.map_err(ApiError::Io)?;
        if lines.len() == limit {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    Ok(lines.into_iter().collect())
}

async fn resolve_status(server: &ServerRow) -> ServerStatusSummary {
    match server.status_type.as_deref().unwrap_or("manual") {
        "systemd" => {
            let Some(unit) = server.status_value.as_deref().filter(|value| !value.trim().is_empty())
            else {
                return ServerStatusSummary {
                    state: "unknown".to_string(),
                    detail: Some("No systemd unit configured".to_string()),
                };
            };

            match Command::new("systemctl")
                .args(["show", unit, "--property=ActiveState", "--value"])
                .output()
                .await
            {
                Ok(output) if output.status.success() => {
                    let active_state = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let state = if active_state == "active" {
                        "running"
                    } else {
                        "stopped"
                    };
                    ServerStatusSummary {
                        state: state.to_string(),
                        detail: Some(active_state),
                    }
                }
                Ok(output) => ServerStatusSummary {
                    state: "unknown".to_string(),
                    detail: Some(format!("systemctl exited with status {}", output.status)),
                },
                Err(err) => ServerStatusSummary {
                    state: "unknown".to_string(),
                    detail: Some(err.to_string()),
                },
            }
        }
        "process" => {
            let Some(process_name) =
                server.status_value.as_deref().filter(|value| !value.trim().is_empty())
            else {
                return ServerStatusSummary {
                    state: "unknown".to_string(),
                    detail: Some("No process name configured".to_string()),
                };
            };

            match Command::new("pgrep").args(["-x", process_name]).output().await {
                Ok(output) if output.status.success() => ServerStatusSummary {
                    state: "running".to_string(),
                    detail: Some(process_name.to_string()),
                },
                Ok(output) if output.status.code() == Some(1) => ServerStatusSummary {
                    state: "stopped".to_string(),
                    detail: Some(process_name.to_string()),
                },
                Ok(output) => ServerStatusSummary {
                    state: "unknown".to_string(),
                    detail: Some(format!("pgrep exited with status {}", output.status)),
                },
                Err(err) => ServerStatusSummary {
                    state: "unknown".to_string(),
                    detail: Some(err.to_string()),
                },
            }
        }
        "tcp" | "http" => ServerStatusSummary {
            state: "unknown".to_string(),
            detail: Some("This status check is not implemented yet".to_string()),
        },
        _ => ServerStatusSummary {
            state: "unknown".to_string(),
            detail: Some("Manual status".to_string()),
        },
    }
}

async fn server_to_record(server: ServerRow) -> ServerRecord {
    let status = resolve_status(&server).await;
    ServerRecord {
        id: server.id,
        name: server.name,
        description: server.description,
        working_dir: server.working_dir,
        start_script: server.start_script,
        stop_script: server.stop_script,
        restart_script: server.restart_script,
        log_type: server.log_type,
        log_path: server.log_path,
        log_unit: server.log_unit,
        status_type: server.status_type,
        status_value: server.status_value,
        status,
        created_at: server.created_at.to_rfc3339(),
        updated_at: server.updated_at.to_rfc3339(),
    }
}

async fn fetch_server(state: &AppState, id: &str) -> Result<ServerRow, ApiError> {
    sqlx::query_as::<_, ServerRow>("SELECT * FROM servers WHERE id = ? LIMIT 1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| ApiError::Database(err.to_string()))?
        .ok_or(ApiError::NotFound)
}

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let rows = match sqlx::query_as::<_, ServerRow>("SELECT * FROM servers ORDER BY updated_at DESC")
        .fetch_all(&state.db)
        .await
    {
        Ok(rows) => rows,
        Err(err) => return api_error_response(ApiError::Database(err.to_string())),
    };

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(server_to_record(row).await);
    }

    Json(ServerListResponse { items }).into_response()
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<ServerCreateRequest>,
) -> impl IntoResponse {
    let name = match validate_name(&body.name) {
        Ok(name) => name,
        Err(err) => return api_error_response(err),
    };

    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    if let Err(err) = sqlx::query(
        "INSERT INTO servers (
            id, name, description, working_dir, start_script, stop_script, restart_script,
            log_type, log_path, log_unit, status_type, status_value, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&name)
    .bind(normalize_optional_text(body.description))
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    {
        return api_error_response(ApiError::Database(err.to_string()));
    }

    match fetch_server(&state, &id).await {
        Ok(server) => Json(server_to_record(server).await).into_response(),
        Err(err) => api_error_response(err),
    }
}

pub async fn get(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    match fetch_server(&state, &id).await {
        Ok(server) => Json(server_to_record(server).await).into_response(),
        Err(err) => api_error_response(err),
    }
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ServerUpdateRequest>,
) -> impl IntoResponse {
    let current = match fetch_server(&state, &id).await {
        Ok(server) => server,
        Err(err) => return api_error_response(err),
    };

    let name = match body.name {
        Some(value) => match validate_name(&value) {
            Ok(name) => name,
            Err(err) => return api_error_response(err),
        },
        None => current.name.clone(),
    };

    let log_type = match validate_log_type(body.log_type) {
        Ok(Some(value)) => Some(value),
        Ok(None) => current.log_type.clone(),
        Err(err) => return api_error_response(err),
    };
    let status_type = match validate_status_type(body.status_type) {
        Ok(Some(value)) => Some(value),
        Ok(None) => current.status_type.clone(),
        Err(err) => return api_error_response(err),
    };

    let now = Utc::now();
    if let Err(err) = sqlx::query(
        "UPDATE servers SET
            name = ?, description = ?, working_dir = ?, start_script = ?, stop_script = ?,
            restart_script = ?, log_type = ?, log_path = ?, log_unit = ?, status_type = ?,
            status_value = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&name)
    .bind(
        body.description
            .map(Some)
            .unwrap_or(current.description.clone())
            .and_then(|value| normalize_optional_text(Some(value))),
    )
    .bind(
        body.working_dir
            .map(Some)
            .unwrap_or(current.working_dir.clone())
            .and_then(|value| normalize_optional_text(Some(value))),
    )
    .bind(
        body.start_script
            .map(Some)
            .unwrap_or(current.start_script.clone())
            .and_then(|value| normalize_optional_text(Some(value))),
    )
    .bind(
        body.stop_script
            .map(Some)
            .unwrap_or(current.stop_script.clone())
            .and_then(|value| normalize_optional_text(Some(value))),
    )
    .bind(
        body.restart_script
            .map(Some)
            .unwrap_or(current.restart_script.clone())
            .and_then(|value| normalize_optional_text(Some(value))),
    )
    .bind(log_type)
    .bind(
        body.log_path
            .map(Some)
            .unwrap_or(current.log_path.clone())
            .and_then(|value| normalize_optional_text(Some(value))),
    )
    .bind(
        body.log_unit
            .map(Some)
            .unwrap_or(current.log_unit.clone())
            .and_then(|value| normalize_optional_text(Some(value))),
    )
    .bind(status_type)
    .bind(
        body.status_value
            .map(Some)
            .unwrap_or(current.status_value.clone())
            .and_then(|value| normalize_optional_text(Some(value))),
    )
    .bind(now)
    .bind(&id)
    .execute(&state.db)
    .await
    {
        return api_error_response(ApiError::Database(err.to_string()));
    }

    match fetch_server(&state, &id).await {
        Ok(server) => Json(server_to_record(server).await).into_response(),
        Err(err) => api_error_response(err),
    }
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    let exists = match fetch_server(&state, &id).await {
        Ok(server) => server,
        Err(err) => return api_error_response(err),
    };
    debug!(server = %exists.name, "deleting server card");

    match sqlx::query("DELETE FROM servers WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(_) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(err) => api_error_response(ApiError::Database(err.to_string())),
    }
}

async fn execute_server_action(server: &ServerRow, action: &'static str) -> Result<ServerActionResult, ApiError> {
    match action {
        "start" => {
            let script = server
                .start_script
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("start script is not configured"))?;
            run_script(script, server.working_dir.as_deref(), "start").await
        }
        "stop" => {
            let script = server
                .stop_script
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("stop script is not configured"))?;
            run_script(script, server.working_dir.as_deref(), "stop").await
        }
        "restart" => {
            if let Some(script) = server.restart_script.as_deref() {
                return run_script(script, server.working_dir.as_deref(), "restart").await;
            }

            let stop_script = server
                .stop_script
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("restart script is not configured"))?;
            let start_script = server
                .start_script
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("restart script is not configured"))?;

            let stop_result = run_script(stop_script, server.working_dir.as_deref(), "stop").await?;
            if !stop_result.ok {
                return Ok(stop_result);
            }
            let start_result =
                run_script(start_script, server.working_dir.as_deref(), "start").await?;
            Ok(merge_action_results(stop_result, start_result))
        }
        _ => Err(ApiError::bad_request("unsupported action")),
    }
}

async fn action_response(state: AppState, id: String, action: &'static str) -> Response {
    let server = match fetch_server(&state, &id).await {
        Ok(server) => server,
        Err(err) => return api_error_response(err),
    };
    debug!(server = %server.name, action, "running server action");

    match execute_server_action(&server, action).await {
        Ok(result) => Json(result).into_response(),
        Err(err) => api_error_response(err),
    }
}

pub async fn start(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    action_response(state, id, "start").await
}

pub async fn stop(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    action_response(state, id, "stop").await
}

pub async fn restart(State(state): State<AppState>, Path(id): Path<String>) -> impl IntoResponse {
    action_response(state, id, "restart").await
}

pub async fn logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ServerLogsQuery>,
) -> impl IntoResponse {
    let server = match fetch_server(&state, &id).await {
        Ok(server) => server,
        Err(err) => return api_error_response(err),
    };
    let lines = normalize_log_lines(query.lines);

    let items = match server.log_type.as_deref() {
        Some("file") => {
            let path = match server.log_path.as_deref() {
                Some(path) => path,
                None => return api_error_response(ApiError::bad_request("log file path is not configured")),
            };
            match tail_file_lines(FsPath::new(path), lines) {
                Ok(lines) => lines,
                Err(err) => return api_error_response(err),
            }
        }
        Some("journal") => {
            let unit = match server.log_unit.as_deref() {
                Some(unit) => unit,
                None => return api_error_response(ApiError::bad_request("journal unit is not configured")),
            };
            match read_journal(unit, lines).await {
                Ok(lines) => lines,
                Err(err) => return api_error_response(err),
            }
        }
        _ => Vec::new(),
    };

    Json(ServerLogsResponse { items }).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_log_limits() {
        assert_eq!(normalize_log_lines(None), DEFAULT_LOG_LINES);
        assert_eq!(normalize_log_lines(Some(0)), 1);
        assert_eq!(normalize_log_lines(Some(100)), 100);
        assert_eq!(normalize_log_lines(Some(5000)), MAX_LOG_LINES);
    }

    #[test]
    fn validates_names() {
        assert!(validate_name("Minecraft").is_ok());
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn validates_log_type_values() {
        assert_eq!(validate_log_type(Some("file".to_string())).unwrap(), Some("file".to_string()));
        assert!(validate_log_type(Some("bad".to_string())).is_err());
    }

    #[test]
    fn validates_status_type_values() {
        assert_eq!(
            validate_status_type(Some("systemd".to_string())).unwrap(),
            Some("systemd".to_string())
        );
        assert!(validate_status_type(Some("bad".to_string())).is_err());
    }
}
