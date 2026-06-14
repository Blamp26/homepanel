use crate::error::ApiError;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub data: DataConfig,
    pub agent: AgentConfig,
    pub auth: AuthConfig,
    pub files: FilesConfig,
    pub security: SecurityConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind: String,
    pub public_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataConfig {
    pub data_dir: PathBuf,
    pub database_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentConfig {
    pub socket_path: PathBuf,
    pub terminal_scrollback_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthConfig {
    pub cookie_name: String,
    pub session_days: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilesConfig {
    pub allowed_paths: Vec<PathBuf>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub allow_remote_terminal: bool,
    pub require_https_for_remote: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind: "127.0.0.1:8080".to_string(),
                public_url: "http://127.0.0.1:8080".to_string(),
            },
            data: DataConfig {
                data_dir: PathBuf::from("/var/lib/homepanel"),
                database_url: "sqlite:///var/lib/homepanel/homepanel.db".to_string(),
            },
            agent: AgentConfig {
                socket_path: PathBuf::from("/run/homepanel/agent.sock"),
                terminal_scrollback_bytes: 10 * 1024 * 1024,
            },
            auth: AuthConfig {
                cookie_name: "homepanel_session".to_string(),
                session_days: 30,
            },
            files: FilesConfig {
                allowed_paths: vec![
                    PathBuf::from("/home"),
                    PathBuf::from("/srv"),
                    PathBuf::from("/mnt"),
                    PathBuf::from("/DATA"),
                ],
            },
            security: SecurityConfig {
                allow_remote_terminal: false,
                require_https_for_remote: true,
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read config: {0}")]
    Io(#[from] std::io::Error),
    #[error("config parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

pub fn load_config(path: Option<&Path>) -> Result<AppConfig, ConfigError> {
    let candidates = match path {
        Some(path) => vec![path.to_path_buf()],
        None => vec![
            PathBuf::from("/etc/homepanel/config.toml"),
            PathBuf::from("./config.toml"),
        ],
    };

    for candidate in candidates {
        if candidate.exists() {
            let contents = fs::read_to_string(candidate)?;
            return Ok(toml::from_str(&contents)?);
        }
    }

    Ok(AppConfig::default())
}

impl From<ConfigError> for ApiError {
    fn from(value: ConfigError) -> Self {
        Self::Config(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bind_is_localhost() {
        assert_eq!(AppConfig::default().server.bind, "127.0.0.1:8080");
    }

    #[test]
    fn missing_config_falls_back_to_default() {
        let config = load_config(Some(Path::new("/definitely/not/here.toml"))).unwrap();
        assert_eq!(config.server.bind, "127.0.0.1:8080");
    }
}
