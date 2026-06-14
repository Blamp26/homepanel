use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::{Path, PathBuf}};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TerminalId(pub String);

impl TerminalId {
    pub fn new(prefix: &str) -> Self {
        Self(format!("{prefix}-{}", Uuid::new_v4()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalKind {
    Shell,
    Command,
    GameServer,
    LogViewer,
    DockerExec,
    ServiceConsole,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalStatus {
    Starting,
    Running,
    Detached,
    Exited,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalRequest {
    pub name: String,
    pub kind: TerminalKind,
    pub command: String,
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub shell: Option<String>,
    #[serde(default)]
    pub auto_restart: bool,
    #[serde(default)]
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalMetadata {
    pub id: TerminalId,
    pub name: String,
    pub kind: TerminalKind,
    pub status: TerminalStatus,
    pub command: String,
    pub cwd: PathBuf,
    pub shell: Option<String>,
    pub cols: u16,
    pub rows: u16,
    pub auto_restart: bool,
    pub exit_code: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_attached_at: Option<DateTime<Utc>>,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerminalSummary {
    pub id: TerminalId,
    pub name: String,
    pub kind: TerminalKind,
    pub status: TerminalStatus,
    pub command: String,
    pub cwd: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub exit_code: Option<i32>,
    pub last_attached_at: Option<DateTime<Utc>>,
}

impl From<TerminalMetadata> for TerminalSummary {
    fn from(value: TerminalMetadata) -> Self {
        Self {
            id: value.id,
            name: value.name,
            kind: value.kind,
            status: value.status,
            command: value.command,
            cwd: value.cwd,
            cols: value.cols,
            rows: value.rows,
            exit_code: value.exit_code,
            last_attached_at: value.last_attached_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum UserRole {
    #[serde(rename = "admin")]
    Admin,
    #[serde(rename = "user")]
    User,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingsEntry {
    pub key: String,
    pub value_json: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LogSource {
    JournalUnit { unit: String },
    File { path: PathBuf },
    Terminal { terminal_id: String },
    DockerContainer { container_id: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEvent {
    pub action: String,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub details_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileAllowlist {
    pub allowed_paths: Vec<PathBuf>,
}

impl FileAllowlist {
    pub fn is_allowed(&self, path: impl AsRef<Path>) -> bool {
        let candidate = canonicalize_like(path.as_ref());
        self.allowed_paths.iter().any(|allowed| {
            let allowed = canonicalize_like(allowed);
            candidate == allowed
                || candidate
                    .strip_prefix(&allowed)
                    .map(|rest| !rest.components().any(|component| matches!(component, std::path::Component::ParentDir)))
                    .unwrap_or(false)
        })
    }
}

fn canonicalize_like(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        use std::path::Component;
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) | Component::Normal(_) => out.push(component.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        path.to_path_buf()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::FileAllowlist;
    use std::path::PathBuf;

    #[test]
    fn allowlist_blocks_parent_escape() {
        let allowlist = FileAllowlist {
            allowed_paths: vec![PathBuf::from("/srv")],
        };
        assert!(allowlist.is_allowed("/srv/minecraft"));
        assert!(!allowlist.is_allowed("/srv/../etc/passwd"));
    }
}
