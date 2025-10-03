use axum::Router;
use axum::http::StatusCode;

use crate::AppState;
use crate::auth::AuthLayer;

mod auth;
mod dyndns;
mod history;
mod interfaces;

pub fn routes(state: &AppState) -> Router<AppState> {
    let auth_layer = AuthLayer::new(state.auth.clone());
    let protected_routes = Router::new()
        .nest("/dyndns", dyndns::routes())
        .nest("/history", history::routes())
        .nest("/interfaces", interfaces::routes())
        .route_layer(auth_layer);

    Router::new()
        .nest("/auth", auth::routes())
        .merge(protected_routes)
        .fallback(|| async { StatusCode::NOT_FOUND })
}
