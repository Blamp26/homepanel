use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub user_id: String,
    pub token_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub user_agent: Option<String>,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ServerRow {
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
