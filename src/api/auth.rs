use std::sync::Arc;

use axum::{Json, Router, extract::State, http::StatusCode, routing::post};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AppState, Error, auth::AuthManager};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct TokenResponse {
    token: String,
    token_type: &'static str,
    expires_at: DateTime<Utc>,
    refresh_token: String,
    refresh_expires_at: DateTime<Utc>,
}

async fn login(
    State(auth): State<Arc<AuthManager>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, Error> {
    let token = auth
        .authenticate(&request.username, &request.password)
        .await?;
    Ok(Json(token.into()))
}

#[derive(Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

async fn refresh(
    State(auth): State<Arc<AuthManager>>,
    Json(request): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, Error> {
    let token = auth.refresh(&request.refresh_token).await?;
    Ok(Json(token.into()))
}

async fn logout(
    State(auth): State<Arc<AuthManager>>,
    Json(request): Json<RefreshRequest>,
) -> Result<StatusCode, Error> {
    auth.revoke(&request.refresh_token).await?;
    Ok(StatusCode::NO_CONTENT)
}

impl From<crate::auth::AuthToken> for TokenResponse {
    fn from(token: crate::auth::AuthToken) -> Self {
        TokenResponse {
            token: token.access_token,
            token_type: "Bearer",
            expires_at: token.access_expires_at,
            refresh_token: token.refresh_token,
            refresh_expires_at: token.refresh_expires_at,
        }
    }
}
