use crate::{
    auth::{
        middleware::require_user,
        password::{hash_password, verify_password},
        sessions::{extract_token, new_session_expiry, new_session_id, token_hash},
    },
    state::{AppState, SessionRecord},
};
use axum::{extract::State, http::{header, HeaderMap, HeaderValue}, response::IntoResponse, Json};
use chrono::Utc;
use homepanel_core::error::{ApiError, ApiErrorResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn cookie_suffix(state: &AppState) -> &'static str {
    if state.config.server.public_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    }
}

#[derive(Debug, Deserialize)]
pub struct SetupRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct AuthStatusResponse {
    pub setup_required: bool,
    pub authenticated: bool,
    pub username: Option<String>,
}

async fn setup_required(state: &AppState) -> Result<bool, axum::response::Response> {
    let user_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await;
    match user_count {
        Ok((count,)) => Ok(count == 0),
        Err(err) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::from(ApiError::Database(err.to_string()))),
        )
            .into_response()),
    }
}

async fn current_username(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<String>, axum::response::Response> {
    let Some(token) = extract_token(
        headers.get(header::COOKIE).and_then(|value| value.to_str().ok()),
        &state.config.auth.cookie_name,
    ) else {
        return Ok(None);
    };

    match crate::auth::sessions::authenticate(state, &token).await {
        Ok(username) => Ok(username),
        Err(err) => Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiErrorResponse::from(ApiError::Database(err.to_string()))),
        )
            .into_response()),
    }
}

pub async fn status(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let setup_required = match setup_required(&state).await {
        Ok(value) => value,
        Err(response) => return response,
    };

    let username = match current_username(&state, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };

    Json(AuthStatusResponse {
        setup_required,
        authenticated: username.is_some(),
        username,
    })
    .into_response()
}

pub async fn setup(State(state): State<AppState>, Json(body): Json<SetupRequest>) -> impl IntoResponse {
    let setup_required = match setup_required(&state).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !setup_required {
        return (axum::http::StatusCode::FORBIDDEN, Json(ApiErrorResponse::from(ApiError::Forbidden))).into_response();
    }

    let password_hash = match hash_password(&body.password) {
        Ok(hash) => hash,
        Err(err) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse::from(ApiError::bad_request(err.to_string()))),
            )
                .into_response()
        }
    };
    let now = Utc::now();
    let user_id = Uuid::new_v4().to_string();
    if let Err(err) = sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&user_id)
    .bind(&body.username)
    .bind(&password_hash)
    .bind("admin")
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await
    {
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(ApiErrorResponse::from(ApiError::Database(err.to_string())))).into_response();
    }

    let token = Uuid::new_v4().to_string();
    let expiry = new_session_expiry(state.config.auth.session_days);
    let session_id = new_session_id().0;
    let now = Utc::now();
    let _ = sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_seen_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(&user_id)
    .bind(token_hash(&token))
    .bind(now)
    .bind(expiry)
    .bind(now)
    .execute(&state.db)
    .await;

    let record = SessionRecord {
        user_id,
        username: body.username.clone(),
        token_hash: token_hash(&token),
    };
    state.sessions.write().expect("session lock").insert(token.clone(), record);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{}={}; HttpOnly; Path=/; SameSite=Lax{}",
            state.config.auth.cookie_name,
            token,
            cookie_suffix(&state)
        ))
        .expect("cookie header"),
    );
    (headers, Json(serde_json::json!({"ok": true, "username": body.username}))).into_response()
}

pub async fn login(State(state): State<AppState>, Json(body): Json<LoginRequest>) -> impl IntoResponse {
    let user: Result<Option<(String, String)>, sqlx::Error> = sqlx::query_as::<_, (String, String)>(
        "SELECT id, password_hash FROM users WHERE username = ? LIMIT 1",
    )
    .bind(&body.username)
    .fetch_optional(&state.db)
    .await;
    let Some((user_id, password_hash)) = (match user {
        Ok(row) => row,
        Err(err) => {
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(ApiErrorResponse::from(ApiError::Database(err.to_string())))).into_response();
        }
    }) else {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(ApiErrorResponse::from(ApiError::Unauthorized)),
        )
            .into_response();
    };

    if !verify_password(&password_hash, &body.password) {
        return (
            axum::http::StatusCode::UNAUTHORIZED,
            Json(ApiErrorResponse::from(ApiError::Unauthorized)),
        )
            .into_response();
    }

    let token = Uuid::new_v4().to_string();
    let expiry = new_session_expiry(state.config.auth.session_days);
    let now = Utc::now();
    let session_id = new_session_id().0;
    if let Err(err) = sqlx::query(
        "INSERT INTO sessions (id, user_id, token_hash, created_at, expires_at, last_seen_at) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(&user_id)
    .bind(token_hash(&token))
    .bind(now)
    .bind(expiry)
    .bind(now)
    .execute(&state.db)
    .await
    {
        return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(ApiErrorResponse::from(ApiError::Database(err.to_string())))).into_response();
    }

    state.sessions.write().expect("session lock").insert(
        token.clone(),
        SessionRecord {
            user_id,
            username: body.username.clone(),
            token_hash: token_hash(&token),
        },
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{}={}; HttpOnly; Path=/; SameSite=Lax{}",
            state.config.auth.cookie_name,
            token,
            cookie_suffix(&state)
        ))
        .expect("cookie header"),
    );
    (headers, Json(serde_json::json!({"ok": true, "username": body.username}))).into_response()
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(token) = extract_token(
        headers.get(header::COOKIE).and_then(|value| value.to_str().ok()),
        &state.config.auth.cookie_name,
    ) {
        state.sessions.write().expect("session lock").remove(&token);
        let _ = sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash(&token))
            .execute(&state.db)
            .await;
    }

    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{}=; Max-Age=0; Path=/; SameSite=Lax{}",
            state.config.auth.cookie_name,
            cookie_suffix(&state)
        ))
        .expect("cookie header"),
    );
    (response_headers, Json(serde_json::json!({"ok": true}))).into_response()
}

pub async fn me(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    match require_user(
        &state,
        headers.get(header::COOKIE).and_then(|value| value.to_str().ok()),
    )
    .await
    {
        Ok(username) => Json(MeResponse { username }).into_response(),
        Err(err) => (axum::http::StatusCode::UNAUTHORIZED, Json(ApiErrorResponse::from(err))).into_response(),
    }
}
