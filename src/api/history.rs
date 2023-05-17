use axum::{
    extract::{Query, State},
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
    #[serde(rename = "sortby")]
    key: SortKey,
    order: SortOrder,
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

impl Default for SortBy {
    fn default() -> Self {
        Self {
            key: SortKey::Updated,
            order: SortOrder::Desc,
        }
    }
}

async fn history(
    State(pool): State<DbPool>,
    Query(pagination): Query<Pagination>,
    Query(sort_items): Query<SortBy>,
) -> Result<Json<HistoryRes>, Error> {
    let conn = pool.get().await?;
    let (histories, total) = History::paginate(
        &conn,
        pagination.page,
        pagination.per_page,
        sort_items.order(),
    )
    .await?;
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
