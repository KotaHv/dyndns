use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Query, State},
    http::{request::Parts, StatusCode},
    routing::get,
    Json, Router,
};
use diesel::ExpressionMethods;
use serde::{Deserialize, Deserializer};

use crate::{
    db::{history, BoxHistoryOrder, History, HistoryIpVersion, HistoryRes},
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

#[derive(Deserialize, Debug)]
#[serde(default)]
struct SortBy {
    key: SortKey,
    order: SortOrder,
}

impl Default for SortBy {
    fn default() -> Self {
        Self {
            key: SortKey::Updated,
            order: SortOrder::Desc,
        }
    }
}

impl SortBy {
    fn order(&self) -> BoxHistoryOrder {
        match self.order {
            SortOrder::Desc => self.key.desc(),
            SortOrder::Asc => self.key.asc(),
        }
    }
}

#[derive(Debug)]
enum SortKey {
    Old,
    New,
    Version,
    Updated,
}

impl SortKey {
    fn desc(&self) -> BoxHistoryOrder {
        match self {
            SortKey::Old => Box::new(history::old_ip.desc()),
            SortKey::New => Box::new(history::new_ip.desc()),
            SortKey::Version => Box::new(history::version.desc()),
            SortKey::Updated => Box::new(history::updated.desc()),
        }
    }

    fn asc(&self) -> BoxHistoryOrder {
        match self {
            SortKey::Old => Box::new(history::old_ip.asc()),
            SortKey::New => Box::new(history::new_ip.asc()),
            SortKey::Version => Box::new(history::version.asc()),
            SortKey::Updated => Box::new(history::updated.asc()),
        }
    }
}

impl<'de> Deserialize<'de> for SortKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let s = match s.as_str() {
            "old_ip" => Self::Old,
            "new_ip" => Self::New,
            "version" => Self::Version,
            _ => Self::Updated,
        };
        Ok(s)
    }
}

#[derive(Debug)]
enum SortOrder {
    Desc,
    Asc,
}

impl<'de> Deserialize<'de> for SortOrder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "asc" => Ok(Self::Asc),
            _ => Ok(Self::Desc),
        }
    }
}

struct SortItems(Vec<SortBy>);

#[async_trait]
impl<S> FromRequestParts<S> for SortItems
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        #[derive(Deserialize)]
        struct A {
            sort_items: String,
        }
        let results = Query::<A>::from_request_parts(parts, state).await;
        match results {
            Ok(Query(results)) => {
                let results = serde_json::from_str::<Vec<SortBy>>(results.sort_items.as_str());
                match results {
                    Ok(sort_items) => Ok(Self(sort_items)),
                    Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
                }
            }
            Err(_) => Ok(Self(vec![SortBy::default()])),
        }
    }
}

async fn history(
    State(pool): State<DbPool>,
    Query(pagination): Query<Pagination>,
    SortItems(sort_items): SortItems,
) -> Result<Json<HistoryRes>, Error> {
    let sort_items: Vec<BoxHistoryOrder> = sort_items.into_iter().map(|v| v.order()).collect();
    let conn = pool.get().await?;
    let (histories, total) =
        History::paginate(&conn, pagination.page, pagination.per_page, sort_items).await?;
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
