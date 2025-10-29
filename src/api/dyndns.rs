use axum::{
    Json, Router,
    extract::{FromRequest, Request, State},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use validator::Validate;

use crate::{AppState, DbPool, Error, db::DynDNS};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_dyndns))
        .route("/", post(create_dyndns))
        .route("/", put(update_dyndns))
}

async fn get_dyndns(State(pool): State<DbPool>) -> Result<Json<DynDNS>, Error> {
    let conn = pool.get().await?;
    match DynDNS::get_option(&conn).await? {
        Some(res) => Ok(Json(res)),
        None => Err(Error::dyn_dns_not_configured()),
    }
}

async fn create_dyndns(
    State(state): State<AppState>,
    dyndns: DynDNS,
) -> Result<Json<DynDNS>, Error> {
    let conn = state.pool.get().await?;
    let res = match DynDNS::get_option(&conn).await? {
        Some(res) => res,
        None => DynDNS::create(&conn, dyndns).await?,
    };
    Ok(Json(res))
}

async fn update_dyndns(
    State(state): State<AppState>,
    dyndns: DynDNS,
) -> Result<Json<DynDNS>, Error> {
    let conn = state.pool.get().await?;
    let interval = dyndns.sleep_interval.get();
    let res = DynDNS::update(&conn, dyndns).await?;
    state.interval_tx.send_replace(interval);
    Ok(Json(res))
}

impl<S> FromRequest<S> for DynDNS
where
    Json<DynDNS>: FromRequest<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(dyndns) = Json::<DynDNS>::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;
        if let Err(e) = dyndns.validate() {
            return Err(Error::validation_failed(e.to_string()).into_response());
        }

        Ok(dyndns)
    }
}
