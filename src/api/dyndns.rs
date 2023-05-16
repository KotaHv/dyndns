use async_trait::async_trait;
use axum::{
    extract::{FromRequest, State},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use validator::Validate;

use crate::{db::DynDNS, AppState, DbPool, Error};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(get_dyndns))
        .route("/", post(create_dyndns))
        .route("/", put(update_dyndns))
}

async fn get_dyndns(State(pool): State<DbPool>) -> Result<Json<DynDNS>, Error> {
    let conn = pool.get().await?;
    let res = DynDNS::get(&conn).await?;

    Ok(Json(res))
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
    let interval = dyndns.sleep_interval;
    let res = DynDNS::update(&conn, dyndns).await?;
    if let Err(e) = state.tx.send(interval as u64).await {
        error!("{}", e);
    }
    Ok(Json(res))
}

#[async_trait]
impl<S, B> FromRequest<S, B> for DynDNS
where
    Json<DynDNS>: FromRequest<S, B>,
    B: Send + 'static,
    S: Send + Sync,
{
    type Rejection = Response;
    async fn from_request(req: Request<B>, state: &S) -> Result<Self, Self::Rejection> {
        let Json(dyndns) = Json::<DynDNS>::from_request(req, state)
            .await
            .map_err(IntoResponse::into_response)?;
        if let Err(e) = dyndns.validate() {
            return Err(Error::Custom {
                status: StatusCode::BAD_REQUEST,
                reason: e.to_string(),
            }
            .into_response());
        }

        Ok(dyndns)
    }
}
