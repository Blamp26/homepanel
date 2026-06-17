use crate::state::AppState;
use chrono::{Duration, Utc};
use hex::encode as hex_encode;
use homepanel_core::{error::Result, types::SessionId};
use sha2::{Digest, Sha256};

pub fn token_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex_encode(hasher.finalize())
}

pub fn new_session_expiry(days: u64) -> chrono::DateTime<Utc> {
    Utc::now() + Duration::days(days as i64)
}

pub fn extract_token(cookie_header: Option<&str>, cookie_name: &str) -> Option<String> {
    let header = cookie_header?;
    for cookie in header.split(';') {
        let mut parts = cookie.trim().splitn(2, '=');
        let name = parts.next()?.trim();
        let value = parts.next()?.trim();
        if name == cookie_name {
            return Some(value.to_string());
        }
    }
    None
}

pub async fn authenticate(state: &AppState, token: &str) -> Result<Option<String>> {
    let hash = token_hash(token);
    let now = Utc::now();
    let row = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT users.username
        FROM sessions
        JOIN users ON users.id = sessions.user_id
        WHERE sessions.token_hash = ?
          AND sessions.expires_at > ?
        LIMIT 1
        "#,
    )
    .bind(&hash)
    .bind(now)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| homepanel_core::error::ApiError::Database(err.to_string()))?;

    Ok(row.map(|(username,)| username))
}

pub fn new_session_id() -> SessionId {
    SessionId::new()
}
