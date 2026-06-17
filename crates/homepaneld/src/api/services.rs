use axum::{
    extract::Path,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use homepanel_core::error::{ApiError, ApiErrorResponse};
use serde::Serialize;
use std::{collections::HashMap, process::Stdio};
use tokio::process::Command;
use tracing::debug;

#[derive(Debug, Serialize)]
pub struct ServiceSummary {
    pub name: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct ServiceListResponse {
    pub items: Vec<ServiceSummary>,
}

#[derive(Debug, Serialize)]
pub struct ServiceDetails {
    pub name: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub unit_file_state: String,
    pub description: String,
    pub fragment_path: Option<String>,
    pub main_pid: u32,
    pub memory_current: Option<u64>,
    pub cpu_usage_nsec: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ServiceLogsResponse {
    pub items: Vec<String>,
}

fn error_response(status: StatusCode, error: ApiError) -> impl IntoResponse {
    (status, Json(ApiErrorResponse::from(error)))
}

fn validate_service_name(name: &str) -> Result<(), ApiError> {
    if !name.ends_with(".service") {
        return Err(ApiError::bad_request("service name must end with .service"));
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '@'))
    {
        return Err(ApiError::bad_request(
            "service name contains invalid characters",
        ));
    }

    Ok(())
}

async fn run_command(program: &str, args: &[&str]) -> Result<String, ApiError> {
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|err| ApiError::message("command_spawn_failed", format!("{program}: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if stderr.is_empty() {
            format!("{program} exited with status {}", output.status)
        } else {
            stderr
        };
        return Err(ApiError::message("command_failed", message));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_list_units(output: &str) -> Vec<ServiceSummary> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let name = parts.next()?.to_string();
            let load = parts.next()?.to_string();
            let active = parts.next()?.to_string();
            let sub = parts.next()?.to_string();
            let description = parts.collect::<Vec<_>>().join(" ");
            Some(ServiceSummary {
                name,
                load,
                active,
                sub,
                description,
            })
        })
        .collect()
}

fn parse_show_properties(output: &str) -> ServiceDetails {
    let mut values: HashMap<&str, String> = HashMap::new();
    for line in output.lines() {
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key, value.to_string());
        }
    }

    let parse_u32 = |key: &str| {
        values
            .get(key)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0)
    };
    let parse_u64 = |key: &str| values.get(key).and_then(|value| value.parse::<u64>().ok());
    let fragment_path = values.get("FragmentPath").and_then(|value| {
        if value.is_empty() || value == "n/a" || value == "/dev/null" {
            None
        } else {
            Some(value.clone())
        }
    });

    ServiceDetails {
        name: values
            .get("Id")
            .cloned()
            .unwrap_or_else(|| "unknown.service".to_string()),
        load_state: values.get("LoadState").cloned().unwrap_or_default(),
        active_state: values.get("ActiveState").cloned().unwrap_or_default(),
        sub_state: values.get("SubState").cloned().unwrap_or_default(),
        unit_file_state: values.get("UnitFileState").cloned().unwrap_or_default(),
        description: values.get("Description").cloned().unwrap_or_default(),
        fragment_path,
        main_pid: parse_u32("MainPID"),
        memory_current: parse_u64("MemoryCurrent"),
        cpu_usage_nsec: parse_u64("CPUUsageNSec"),
    }
}

async fn service_output(program: &str, args: &[&str]) -> Result<String, ApiError> {
    run_command(program, args).await
}

pub async fn list() -> impl IntoResponse {
    match service_output(
        "systemctl",
        &[
            "list-units",
            "--type=service",
            "--all",
            "--no-pager",
            "--plain",
            "--no-legend",
        ],
    )
    .await
    {
        Ok(output) => {
            let items = parse_list_units(&output);
            Json(ServiceListResponse { items }).into_response()
        }
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

pub async fn get(Path(name): Path<String>) -> impl IntoResponse {
    if let Err(err) = validate_service_name(&name) {
        return error_response(StatusCode::BAD_REQUEST, err).into_response();
    }

    debug!(service = %name, "loading systemd service details");
    match service_output(
        "systemctl",
        &[
            "show",
            &name,
            "--no-pager",
            "--plain",
            "--property=Id,LoadState,ActiveState,SubState,UnitFileState,Description,FragmentPath,MainPID,MemoryCurrent,CPUUsageNSec",
        ],
    )
    .await
    {
        Ok(output) => Json(parse_show_properties(&output)).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

async fn service_action(name: String, action: &'static str) -> impl IntoResponse {
    if let Err(err) = validate_service_name(&name) {
        return error_response(StatusCode::BAD_REQUEST, err).into_response();
    }

    debug!(service = %name, action, "running systemctl action");
    match service_output("systemctl", &[action, &name]).await {
        Ok(_) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

pub async fn start(Path(name): Path<String>) -> impl IntoResponse {
    service_action(name, "start").await
}

pub async fn stop(Path(name): Path<String>) -> impl IntoResponse {
    service_action(name, "stop").await
}

pub async fn restart(Path(name): Path<String>) -> impl IntoResponse {
    service_action(name, "restart").await
}

pub async fn logs(Path(name): Path<String>) -> impl IntoResponse {
    if let Err(err) = validate_service_name(&name) {
        return error_response(StatusCode::BAD_REQUEST, err).into_response();
    }

    debug!(service = %name, "reading service logs");
    match service_output(
        "journalctl",
        &["-u", &name, "-n", "200", "--no-pager", "--output=short-iso"],
    )
    .await
    {
        Ok(output) => Json(ServiceLogsResponse {
            items: output.lines().map(|line| line.to_string()).collect(),
        })
        .into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_service_name() {
        assert!(validate_service_name("homepaneld.service").is_ok());
        assert!(validate_service_name("ssh@host.service").is_ok());
        assert!(validate_service_name("bad.service;rm -rf /").is_err());
        assert!(validate_service_name("bad.unit").is_err());
    }

    #[test]
    fn parses_list_units_output() {
        let services = parse_list_units(
            "ssh.service loaded active running OpenBSD Secure Shell server\n\
             homepaneld.service loaded active running HomePanel daemon\n",
        );

        assert_eq!(services.len(), 2);
        assert_eq!(services[0].name, "ssh.service");
        assert_eq!(services[0].description, "OpenBSD Secure Shell server");
        assert_eq!(services[1].name, "homepaneld.service");
    }

    #[test]
    fn parses_service_properties() {
        let service = parse_show_properties(
            "Id=homepaneld.service\n\
             LoadState=loaded\n\
             ActiveState=active\n\
             SubState=running\n\
             UnitFileState=enabled\n\
             Description=HomePanel daemon\n\
             FragmentPath=/usr/lib/systemd/system/homepaneld.service\n\
             MainPID=1234\n\
             MemoryCurrent=54321\n\
             CPUUsageNSec=98765\n",
        );

        assert_eq!(service.name, "homepaneld.service");
        assert_eq!(service.load_state, "loaded");
        assert_eq!(service.main_pid, 1234);
        assert_eq!(service.memory_current, Some(54321));
        assert_eq!(service.cpu_usage_nsec, Some(98765));
    }
}
