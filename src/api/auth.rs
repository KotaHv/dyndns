use std::sync::Arc;

use axum::{Json, Router, extract::State, routing::post};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{AppState, Error, auth::AuthManager};

pub fn routes() -> Router<AppState> {
    Router::new().route("/login", post(login))
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    token_type: &'static str,
    expires_at: DateTime<Utc>,
}

async fn login(
    State(auth): State<Arc<AuthManager>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, Error> {
    let token = auth.authenticate(&request.username, &request.password)?;
    Ok(Json(LoginResponse {
        token: token.token,
        token_type: "Bearer",
        expires_at: token.expires_at,
    }))
}
