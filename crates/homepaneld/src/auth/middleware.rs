use crate::{auth::sessions::{authenticate, extract_token}, state::AppState};
use homepanel_core::error::{ApiError, Result};

pub async fn require_user(state: &AppState, cookie_header: Option<&str>) -> Result<String> {
    let token = extract_token(cookie_header, &state.config.auth.cookie_name)
        .ok_or(ApiError::Unauthorized)?;
    authenticate(state, &token)
        .await?
        .ok_or(ApiError::Unauthorized)
}
