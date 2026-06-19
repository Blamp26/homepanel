use crate::state::AppState;
use axum::{extract::State, response::IntoResponse, Json};
use nix::sys::statvfs::statvfs;
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    os::unix::fs::MetadataExt,
};
use tokio::process::Command;

#[derive(Debug, Serialize)]
pub struct DiskUsage {
    pub mount_point: String,
    pub total_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Default)]
pub struct ServiceCounts {
    pub total: Option<u64>,
    pub running: Option<u64>,
    pub failed: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct OverviewResponse {
    pub api_status: String,
    pub hostname: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub load_average: Option<[f64; 3]>,
    pub memory_total_bytes: Option<u64>,
    pub memory_available_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub disks: Vec<DiskUsage>,
    pub primary_ips: Vec<String>,
    pub terminal_count: usize,
    pub service_summary: ServiceCounts,
    pub storage_path: String,
    pub database_path: Option<String>,
    pub version: String,
}

fn read_meminfo() -> HashMap<String, u64> {
    let Ok(contents) = fs::read_to_string("/proc/meminfo") else {
        return HashMap::new();
    };

    let mut values = HashMap::new();
    for line in contents.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let numeric = value.split_whitespace().next().and_then(|value| value.parse().ok());
        if let Some(number) = numeric {
            values.insert(key.to_string(), number);
        }
    }
    values
}

fn read_hostname() -> Option<String> {
    fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_uptime_seconds() -> Option<u64> {
    let uptime = fs::read_to_string("/proc/uptime").ok()?;
    let seconds = uptime.split_whitespace().next()?.parse::<f64>().ok()?;
    Some(seconds.floor() as u64)
}

fn read_load_average() -> Option<[f64; 3]> {
    let loadavg = fs::read_to_string("/proc/loadavg").ok()?;
    let mut values = loadavg
        .split_whitespace()
        .take(3)
        .filter_map(|value| value.parse::<f64>().ok());
    Some([values.next()?, values.next()?, values.next()?])
}

fn disk_usage(mount_point: &Path) -> Option<DiskUsage> {
    let stat = statvfs(mount_point).ok()?;
    let fragment_size = u64::try_from(stat.fragment_size()).ok()?;
    let total_blocks = u64::try_from(stat.blocks()).ok()?;
    let available_blocks = u64::try_from(stat.blocks_available()).ok()?;
    let total_bytes = total_blocks.checked_mul(fragment_size)?;
    let available_bytes = available_blocks.checked_mul(fragment_size)?;
    let used_bytes = total_bytes.saturating_sub(available_bytes);

    Some(DiskUsage {
        mount_point: mount_point.to_string_lossy().into_owned(),
        total_bytes: Some(total_bytes),
        used_bytes: Some(used_bytes),
        available_bytes: Some(available_bytes),
    })
}

fn mount_device_id(mount_point: &Path) -> Option<u64> {
    fs::metadata(mount_point).ok().map(|metadata| metadata.dev())
}

fn resolve_sqlite_database_path(database_url: &str) -> Option<String> {
    database_url
        .strip_prefix("sqlite:///")
        .map(|value| format!("/{value}"))
        .or_else(|| database_url.strip_prefix("sqlite://").map(str::to_string))
        .or_else(|| database_url.strip_prefix("sqlite:").map(str::to_string))
}

async fn read_primary_ips() -> Vec<String> {
    let output = Command::new("hostname").arg("-I").output().await;
    let Ok(output) = output else {
        return Vec::new();
    };

    if !output.status.success() {
        return Vec::new();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split_whitespace()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

async fn read_service_counts() -> ServiceCounts {
    let output = Command::new("systemctl")
        .args([
            "list-units",
            "--type=service",
            "--all",
            "--no-pager",
            "--plain",
            "--no-legend",
        ])
        .output()
        .await;

    let Ok(output) = output else {
        return ServiceCounts::default();
    };

    if !output.status.success() {
        return ServiceCounts::default();
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut total = 0u64;
    let mut running = 0u64;
    let mut failed = 0u64;

    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let _name = parts.next();
        let _load = parts.next();
        let active = parts.next().unwrap_or_default().to_lowercase();
        let sub = parts.next().unwrap_or_default().to_lowercase();

        if active.is_empty() {
          continue;
        }

        total += 1;
        if active == "active" || sub == "running" {
            running += 1;
        }
        if active == "failed" || sub == "failed" {
            failed += 1;
        }
    }

    ServiceCounts {
        total: Some(total),
        running: Some(running),
        failed: Some(failed),
    }
}

async fn read_disks() -> Vec<DiskUsage> {
    let mut disks = Vec::new();
    for mount_point in ["/", "/mnt/games", "/var/lib/homepanel"] {
        let path = PathBuf::from(mount_point);
        if !path.exists() {
            continue;
        }
        if let Some(disk) = disk_usage(&path) {
            disks.push((mount_device_id(&path), disk));
        }
    }

    disks.sort_by(|a, b| {
        let (a_mount, b_mount) = (&a.1.mount_point, &b.1.mount_point);
        let rank = |mount: &str| match mount {
            "/" => 0,
            "/mnt/games" => 1,
            "/var/lib/homepanel" => 2,
            _ => 3,
        };
        rank(a_mount)
            .cmp(&rank(b_mount))
            .then_with(|| a_mount.cmp(b_mount))
    });

    let mut deduped: Vec<(Option<u64>, DiskUsage)> = Vec::new();
    for (device_id, disk) in disks {
        let same_device = device_id.is_some()
            && deduped
                .iter()
                .any(|(existing_device_id, _)| existing_device_id == &device_id);
        if same_device && disk.mount_point != "/" && disk.mount_point != "/mnt/games" {
            continue;
        }

        let same_signature = deduped.iter().any(|(_, existing)| {
            existing.total_bytes == disk.total_bytes
                && existing.available_bytes == disk.available_bytes
        });
        if same_signature && disk.mount_point == "/var/lib/homepanel" {
            continue;
        }
        deduped.push((device_id, disk));
    }

    deduped.into_iter().map(|(_, disk)| disk).collect()
}

pub async fn get(State(state): State<AppState>) -> impl IntoResponse {
    let uptime_seconds = read_uptime_seconds();
    let meminfo = read_meminfo();
    let memory_total_kb = meminfo.get("MemTotal").copied();
    let memory_available_kb = meminfo.get("MemAvailable").copied().or_else(|| {
        match (
            meminfo.get("MemFree").copied(),
            meminfo.get("Buffers").copied(),
            meminfo.get("Cached").copied(),
        ) {
            (Some(free), Some(buffers), Some(cached)) => {
                Some(free.saturating_add(buffers).saturating_add(cached))
            }
            _ => None,
        }
    });
    let memory_total_bytes = memory_total_kb.map(|value| value * 1024);
    let memory_available_bytes = memory_available_kb.map(|value| value * 1024);
    let memory_used_bytes = match (memory_total_bytes, memory_available_bytes) {
        (Some(total), Some(available)) => Some(total.saturating_sub(available)),
        _ => None,
    };

    let disks = read_disks().await;
    let primary_ips = read_primary_ips().await;
    let terminal_count = state.agent.manager().list_terminals().len();
    let service_summary = read_service_counts().await;
    let hostname = read_hostname();
    let database_path = resolve_sqlite_database_path(&state.config.data.database_url);

    Json(OverviewResponse {
        api_status: "online".to_string(),
        hostname,
        uptime_seconds,
        load_average: read_load_average(),
        memory_total_bytes,
        memory_available_bytes,
        memory_used_bytes,
        disks,
        primary_ips,
        terminal_count,
        service_summary,
        storage_path: state.config.data.data_dir.to_string_lossy().into_owned(),
        database_path,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
