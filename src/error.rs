use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use deadpool_diesel::{InteractError, PoolError};
use diesel::result::Error as DieselError;
use isahc::Error as IsahcError;
use local_ip_address::Error as LocalIpError;
use serde::Serialize;
use tokio::task::JoinError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    DynDns(#[from] DynDnsError),
    #[error(transparent)]
    Network(#[from] NetworkError),
    #[error(transparent)]
    System(#[from] SystemError),
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("DeadPoolError: {0}")]
    DeadPool(#[from] DeadPoolError),
    #[error("DieselError: {0}")]
    Diesel(#[from] DieselError),
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("{reason}")]
    Unauthorized { reason: String, code: &'static str },
    #[error("failed to encode auth token: {0}")]
    TokenEncodingFailed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DynDnsError {
    #[error("DynDNS configuration not set yet")]
    NotConfigured,
    #[error("{0}")]
    ValidationFailed(String),
    #[error(transparent)]
    SleepInterval(#[from] SleepIntervalError),
}

#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("Isahc Error: {0}")]
    Http(#[from] IsahcError),
    #[error("{0}")]
    Interface(#[from] LocalIpError),
    #[error("ipv6 not found")]
    Ipv6NotFound,
    #[error("ipv4 not found")]
    Ipv4NotFound,
    #[error("Failed to parse IPv4 address : {0}")]
    IPv4ParseError(String),
    #[error("Failed to parse IPv6 address : {0}")]
    IPv6ParseError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum SystemError {
    #[error("Tokio JoinError: {0}")]
    Join(#[from] JoinError),
    #[error("{0}")]
    Io(#[from] std::io::Error),
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
        AuthError::Unauthorized {
            reason: reason.into(),
            code,
        }
        .into()
    }

    pub fn validation_failed(reason: impl Into<String>) -> Self {
        DynDnsError::ValidationFailed(reason.into()).into()
    }

    pub fn dyn_dns_not_configured() -> Self {
        DynDnsError::NotConfigured.into()
    }

    pub fn token_encoding_failed(reason: impl Into<String>) -> Self {
        AuthError::TokenEncodingFailed(reason.into()).into()
    }

    pub fn ipv4_parse_error(input: impl Into<String>) -> Self {
        NetworkError::IPv4ParseError(input.into()).into()
    }

    pub fn ipv6_parse_error(input: impl Into<String>) -> Self {
        NetworkError::IPv6ParseError(input.into()).into()
    }

    pub fn ipv6_not_found() -> Self {
        NetworkError::Ipv6NotFound.into()
    }

    pub fn ipv4_not_found() -> Self {
        NetworkError::Ipv4NotFound.into()
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Error::Database(DatabaseError::Diesel(DieselError::NotFound)) => StatusCode::NOT_FOUND,
            Error::Auth(AuthError::Unauthorized { .. }) => StatusCode::UNAUTHORIZED,
            Error::DynDns(DynDnsError::NotConfigured) => StatusCode::NOT_FOUND,
            Error::DynDns(DynDnsError::ValidationFailed(_)) => StatusCode::BAD_REQUEST,
            Error::DynDns(DynDnsError::SleepInterval(_)) => StatusCode::BAD_REQUEST,
            Error::Auth(AuthError::TokenEncodingFailed(_)) => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> Option<&'static str> {
        match self {
            Error::Database(db) => match db {
                DatabaseError::DeadPool(_) => Some("database_error"),
                DatabaseError::Diesel(DieselError::NotFound) => Some("not_found"),
                DatabaseError::Diesel(_) => Some("database_error"),
            },
            Error::Auth(auth) => match auth {
                AuthError::Unauthorized { code, .. } => Some(*code),
                AuthError::TokenEncodingFailed(_) => Some("token_encoding_failed"),
            },
            Error::DynDns(dyndns) => match dyndns {
                DynDnsError::NotConfigured => Some("dyndns_not_configured"),
                DynDnsError::ValidationFailed(_) => Some("validation_failed"),
                DynDnsError::SleepInterval(_) => Some("invalid_sleep_interval"),
            },
            Error::Network(net) => match net {
                NetworkError::Http(_) => Some("http_client_error"),
                NetworkError::Interface(_) => Some("interface_error"),
                NetworkError::Ipv6NotFound => Some("ipv6_not_found"),
                NetworkError::Ipv4NotFound => Some("ipv4_not_found"),
                NetworkError::IPv4ParseError(_) => Some("ipv4_parse_error"),
                NetworkError::IPv6ParseError(_) => Some("ipv6_parse_error"),
            },
            Error::System(system) => match system {
                SystemError::Join(_) => Some("internal_error"),
                SystemError::Io(_) => Some("io_error"),
            },
        }
    }
}

impl From<DieselError> for Error {
    fn from(err: DieselError) -> Self {
        Error::Database(err.into())
    }
}

impl From<SleepIntervalError> for Error {
    fn from(err: SleepIntervalError) -> Self {
        Error::DynDns(err.into())
    }
}

impl From<DeadPoolError> for Error {
    fn from(err: DeadPoolError) -> Self {
        Error::Database(err.into())
    }
}

impl From<PoolError> for Error {
    fn from(err: PoolError) -> Self {
        Error::from(DeadPoolError::from(err))
    }
}

impl From<InteractError> for Error {
    fn from(err: InteractError) -> Self {
        Error::from(DeadPoolError::from(err))
    }
}

impl From<IsahcError> for Error {
    fn from(err: IsahcError) -> Self {
        Error::Network(err.into())
    }
}

impl From<LocalIpError> for Error {
    fn from(err: LocalIpError) -> Self {
        Error::Network(err.into())
    }
}

impl From<JoinError> for Error {
    fn from(err: JoinError) -> Self {
        Error::System(err.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::System(err.into())
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
