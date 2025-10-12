use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use deadpool_diesel::{InteractError, PoolError};
use diesel::result::Error as DieselError;
use isahc::Error as rError;
use local_ip_address::Error as LError;
use serde::Serialize;
use tokio::task::JoinError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("DeadPoolError: {0}")]
    DeadPool(DeadPoolError),
    #[error("DieselError: {0}")]
    Diesel(#[from] DieselError),
    #[error("{reason}")]
    Custom {
        status: StatusCode,
        reason: String,
        code: Option<&'static str>,
    },
    #[error("Isahc Error: {0}")]
    Isahc(#[from] rError),
    #[error("Tokio JoinError: {0}")]
    Join(#[from] JoinError),
    #[error("ipv6 not found")]
    Ipv6NotFound,
    #[error("{0}")]
    Interface(#[from] LError),
    #[error("{0}")]
    SleepInterval(#[from] SleepIntervalError),
    #[error("Failed to parse IPv4 address : {0}")]
    IPv4ParseError(String),
    #[error("Failed to parse IPv6 address : {0}")]
    IPv6ParseError(String),
    #[error("{0}")]
    IOError(#[from] std::io::Error),
}

#[derive(Serialize)]
struct ErrorJson {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        error!("{}", self);

        let status = self.status_code();
        let code = self.code().map(str::to_owned);
        (
            status,
            Json(ErrorJson {
                error: self.to_string(),
                code,
            }),
        )
            .into_response()
    }
}

impl Error {
    pub fn unauthorized(reason: impl Into<String>, code: &'static str) -> Self {
        Self::Custom {
            status: StatusCode::UNAUTHORIZED,
            reason: reason.into(),
            code: Some(code),
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Error::Diesel(DieselError::NotFound) => StatusCode::NOT_FOUND,
            Error::SleepInterval(_) => StatusCode::BAD_REQUEST,
            Error::Custom { status, .. } => *status,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> Option<&'static str> {
        match self {
            Error::DeadPool(_) => Some("database_error"),
            Error::Diesel(DieselError::NotFound) => Some("not_found"),
            Error::Diesel(_) => Some("database_error"),
            Error::Custom { code, .. } => *code,
            Error::Isahc(_) => Some("http_client_error"),
            Error::Join(_) => Some("internal_error"),
            Error::Ipv6NotFound => Some("ipv6_not_found"),
            Error::Interface(_) => Some("interface_error"),
            Error::SleepInterval(_) => Some("invalid_sleep_interval"),
            Error::IPv4ParseError(_) => Some("ipv4_parse_error"),
            Error::IPv6ParseError(_) => Some("ipv6_parse_error"),
            Error::IOError(_) => Some("io_error"),
        }
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
pub enum SleepIntervalError {
    #[error("sleep_interval must be greater than zero")]
    NonPositive,
    #[error("sleep_interval exceeds supported range")]
    Overflow,
}

#[derive(Debug, thiserror::Error)]
pub enum DeadPoolError {
    #[error("InteractError: {0}")]
    Interact(#[from] InteractError),
    #[error("PoolError: {0}")]
    Pool(#[from] PoolError),
}
