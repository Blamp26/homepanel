use axum::{response::IntoResponse, Json};

fn read_meminfo_value(key: &str) -> Option<u64> {
    let contents = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in contents.lines() {
        if let Some(value) = line.strip_prefix(key) {
            let value = value.split_whitespace().nth(1)?;
            return value.parse().ok();
        }
    }
    None
}

pub async fn summary() -> impl IntoResponse {
    let uptime = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(str::to_string))
        .unwrap_or_else(|| "0".to_string());
    Json(serde_json::json!({
        "cpu": { "loadavg": std::fs::read_to_string("/proc/loadavg").ok() },
        "memory": {
            "total_kb": read_meminfo_value("MemTotal:"),
            "available_kb": read_meminfo_value("MemAvailable:"),
            "swap_total_kb": read_meminfo_value("SwapTotal:"),
            "swap_free_kb": read_meminfo_value("SwapFree:")
        },
        "uptime_seconds": uptime,
    }))
}

pub async fn disks() -> impl IntoResponse {
    Json(serde_json::json!({"items": []}))
}

pub async fn network() -> impl IntoResponse {
    let data = std::fs::read_to_string("/proc/net/dev").ok();
    Json(serde_json::json!({"proc_net_dev": data}))
}

pub async fn processes() -> impl IntoResponse {
    Json(serde_json::json!({"items": []}))
}

pub async fn metrics_ws() -> impl IntoResponse {
    Json(serde_json::json!({"ok": true}))
}
