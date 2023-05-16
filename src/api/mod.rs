use axum::Router;

use crate::AppState;

mod dyndns;
mod history;
mod interfaces;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .nest("/dyndns", dyndns::routes())
        .nest("/history", history::routes())
        .with_state(state)
        .nest("/interfaces", interfaces::routes())
}
