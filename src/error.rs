use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};

use deadpool_diesel::{InteractError, PoolError};
use diesel::result::Error as DieselError;
use local_ip_address::Error as LError;
use reqwest::Error as rError;
use serde::Serialize;
use tokio::task::JoinError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("DeadPoolError: {0}")]
    DeadPool(DeadPoolError),
    #[error("DieselError: {0}")]
    Diesel(#[from] DieselError),
    #[error("{reason}")]
    Custom { status: StatusCode, reason: String },
    #[error("Reqwest Error: {0}")]
    Reqwest(#[from] rError),
    #[error("Tokio JoinError: {0}")]
    Join(#[from] JoinError),
    #[error("ipv6 not found")]
    Ipv6NotFound,
    #[error("{0}")]
    Interface(#[from] LError),
}

#[derive(Serialize)]
struct ErrorJson {
    error: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!("{}", self);

        let status = match self {
            Error::Diesel(DieselError::NotFound) => StatusCode::NOT_FOUND,
            Error::Custom { status, reason: _ } => status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(ErrorJson {
                error: self.to_string(),
            }),
        )
            .into_response()
    }
}

impl<E> From<E> for Error
where
    E: Into<DeadPoolError>,
{
    fn from(e: E) -> Self {
        Self::DeadPool(e.into())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DeadPoolError {
    #[error("InteractError: {0}")]
    Interact(#[from] InteractError),
    #[error("PoolError: {0}")]
    Pool(#[from] PoolError),
}
