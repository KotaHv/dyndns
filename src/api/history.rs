use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::{
    db::{History, HistoryIpVersion, HistoryRes},
    AppState, DbPool, Error,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(history))
        .route("/current", get(current))
}

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Pagination {
    page: usize,
    per_page: i64,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 10,
        }
    }
}

async fn history(
    State(pool): State<DbPool>,
    Query(pagination): Query<Pagination>,
) -> Result<Json<HistoryRes>, Error> {
    warn!("{:?}", pagination);
    let conn = pool.get().await?;
    let (histories, total) = History::paginate(&conn, pagination.page, pagination.per_page).await?;
    Ok(Json(HistoryRes::new(total, histories)))
}

#[derive(Deserialize)]
struct Current {
    version: HistoryIpVersion,
}

async fn current(
    State(pool): State<DbPool>,
    Query(query): Query<Current>,
) -> Result<Json<Option<History>>, Error> {
    let conn = pool.get().await?;
    let history = History::get_current(&conn, query.version).await?;
    Ok(Json(history))
}
