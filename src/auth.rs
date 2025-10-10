use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration as StdDuration,
};

use argon2::{
    Argon2,
    password_hash::{
        PasswordHash, PasswordHashString, PasswordHasher, PasswordVerifier, SaltString,
        rand_core::{OsRng, RngCore},
    },
};
use axum::{
    extract::Request,
    http::{StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
};
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use diesel::result::{DatabaseErrorKind, Error as DieselResultError};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, errors::ErrorKind};
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tower::{Layer, Service};

use crate::{DbPool, Error, config::Auth as AuthConfig, db::RefreshTokenRecord};

pub struct AuthManager {
    username: String,
    password_hash: PasswordHashString,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    token_ttl: Duration,
    refresh_token_ttl: Duration,
    pool: DbPool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

pub struct AuthToken {
    pub access_token: String,
    pub access_expires_at: DateTime<Utc>,
    pub refresh_token: String,
    pub refresh_expires_at: DateTime<Utc>,
}

struct RefreshTokenPayload {
    token: String,
    selector: String,
    verifier_hash: String,
}

impl AuthManager {
    pub fn new(config: &AuthConfig, pool: DbPool) -> Result<Self, String> {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let encoding_key = EncodingKey::from_secret(&secret);
        let decoding_key = DecodingKey::from_secret(&secret);
        let token_ttl = Duration::from_std(StdDuration::from_secs(config.token_ttl_seconds))
            .map_err(|err| format!("invalid authentication token ttl: {err}"))?;
        let refresh_token_ttl =
            Duration::from_std(StdDuration::from_secs(config.refresh_token_ttl_seconds))
                .map_err(|err| format!("invalid authentication refresh token ttl: {err}"))?;
        let salt = SaltString::generate(&mut OsRng);
        let password_hash_string = Argon2::default()
            .hash_password(config.password.as_bytes(), &salt)
            .map_err(|err| format!("failed to hash auth password: {err}"))?
            .to_string();
        let password_hash = PasswordHashString::new(&password_hash_string)
            .map_err(|err| format!("invalid generated auth password hash: {err}"))?;

        Ok(Self {
            username: config.username.clone(),
            encoding_key,
            decoding_key,
            token_ttl,
            refresh_token_ttl,
            password_hash,
            pool,
        })
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> Result<AuthToken, Error> {
        if username != self.username {
            return Err(Error::unauthorized(
                "invalid credentials",
                "invalid_credentials",
            ));
        }
        let parsed_hash = self.password_hash.password_hash();
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| Error::unauthorized("invalid credentials", "invalid_credentials"))?;

        self.issue_tokens().await
    }

    pub async fn refresh(&self, refresh_token: &str) -> Result<AuthToken, Error> {
        let now = Utc::now();
        let (selector, verifier) = Self::split_refresh_token(refresh_token)?;

        let conn = self.pool.get().await?;

        let Some(record) = RefreshTokenRecord::find(&conn, &selector).await? else {
            return Err(Error::unauthorized(
                "invalid refresh token",
                "invalid_refresh_token",
            ));
        };

        if record.expires_at <= now.naive_utc() {
            RefreshTokenRecord::delete(&conn, &selector).await?;
            return Err(Error::unauthorized(
                "refresh token expired",
                "refresh_token_expired",
            ));
        }

        let hash = PasswordHash::new(&record.verifier_hash).map_err(|err| Error::Custom {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            reason: format!("invalid refresh token hash: {err}"),
            code: Some("refresh_token_verification_failed"),
        })?;

        Argon2::default()
            .verify_password(verifier.as_bytes(), &hash)
            .map_err(|_| Error::unauthorized("invalid refresh token", "invalid_refresh_token"))?;

        RefreshTokenRecord::delete(&conn, &selector).await?;
        RefreshTokenRecord::delete_expired(&conn, now.naive_utc()).await?;

        drop(conn);
        self.issue_tokens().await
    }

    pub async fn revoke(&self, refresh_token: &str) -> Result<(), Error> {
        let now = Utc::now();
        let (selector, verifier) = Self::split_refresh_token(refresh_token)?;
        let conn = self.pool.get().await?;

        if let Some(record) = RefreshTokenRecord::find(&conn, &selector).await? {
            let hash = PasswordHash::new(&record.verifier_hash).map_err(|err| Error::Custom {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                reason: format!("invalid refresh token hash: {err}"),
                code: Some("refresh_token_verification_failed"),
            })?;

            Argon2::default()
                .verify_password(verifier.as_bytes(), &hash)
                .map_err(|_| {
                    Error::unauthorized("invalid refresh token", "invalid_refresh_token")
                })?;

            RefreshTokenRecord::delete(&conn, &selector).await?;
        }

        RefreshTokenRecord::delete_expired(&conn, now.naive_utc()).await?;
        Ok(())
    }

    async fn issue_tokens(&self) -> Result<AuthToken, Error> {
        let now = Utc::now();
        let access_expires_at = now + self.token_ttl;
        let claims = Claims {
            sub: self.username.clone(),
            exp: access_expires_at.timestamp(),
            iat: now.timestamp(),
        };
        let header = Header::new(Algorithm::HS256);
        let token = jsonwebtoken::encode(&header, &claims, &self.encoding_key).map_err(|err| {
            Error::Custom {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                reason: format!("failed to encode auth token: {err}"),
                code: Some("token_encoding_failed"),
            }
        })?;
        let refresh_expires_at = now + self.refresh_token_ttl;

        let conn = self.pool.get().await?;
        RefreshTokenRecord::delete_expired(&conn, now.naive_utc()).await?;

        let refresh_token = loop {
            let payload = self.generate_refresh_token_pair()?;
            let record = RefreshTokenRecord {
                selector: payload.selector.clone(),
                verifier_hash: payload.verifier_hash.clone(),
                expires_at: refresh_expires_at.naive_utc(),
                created_at: now.naive_utc(),
            };

            match RefreshTokenRecord::insert(&conn, record).await {
                Ok(()) => break payload.token,
                Err(Error::Diesel(DieselResultError::DatabaseError(
                    DatabaseErrorKind::UniqueViolation,
                    _,
                ))) => continue,
                Err(err) => return Err(err),
            }
        };

        drop(conn);

        Ok(AuthToken {
            access_token: token,
            access_expires_at,
            refresh_token,
            refresh_expires_at,
        })
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, Error> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data = jsonwebtoken::decode::<Claims>(token, &self.decoding_key, &validation)
            .map_err(|err| match err.kind() {
                ErrorKind::ExpiredSignature => {
                    Error::unauthorized("token expired", "token_expired")
                }
                _ => Error::unauthorized("invalid token", "invalid_token"),
            })?;
        Ok(token_data.claims)
    }

    fn generate_refresh_token_pair(&self) -> Result<RefreshTokenPayload, Error> {
        let selector = Self::random_token_segment(16);
        let verifier = Self::random_token_segment(32);
        let refresh_token = format!("{selector}.{verifier}");
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(verifier.as_bytes(), &salt)
            .map_err(|err| Error::Custom {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                reason: format!("failed to hash refresh token: {err}"),
                code: Some("refresh_token_hash_failed"),
            })?
            .to_string();
        Ok(RefreshTokenPayload {
            token: refresh_token,
            selector,
            verifier_hash: hash,
        })
    }

    fn random_token_segment(len: usize) -> String {
        let mut bytes = vec![0u8; len];
        OsRng.fill_bytes(&mut bytes);
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn split_refresh_token(token: &str) -> Result<(String, String), Error> {
        let mut parts = token.splitn(2, '.');
        let selector = parts.next().unwrap_or_default();
        let verifier = parts.next();
        match (selector.is_empty(), verifier) {
            (false, Some(verifier)) if !verifier.is_empty() => {
                Ok((selector.to_string(), verifier.to_string()))
            }
            _ => Err(Error::unauthorized(
                "invalid refresh token",
                "invalid_refresh_token",
            )),
        }
    }
}

#[derive(Clone)]
pub struct AuthLayer {
    auth: Arc<AuthManager>,
}

impl AuthLayer {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        AuthLayer { auth }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware {
            inner,
            auth: self.auth.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
    auth: Arc<AuthManager>,
}

impl<S> Service<Request> for AuthMiddleware<S>
where
    S: Service<Request, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = AuthFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let authorization = match req.headers().get(AUTHORIZATION) {
            Some(value) => match value.to_str() {
                Ok(value) => value.trim(),
                Err(_) => {
                    return AuthFuture::unauthorized_msg(
                        "invalid Authorization header",
                        "invalid_authorization_header",
                    );
                }
            },
            None => {
                return AuthFuture::unauthorized_msg(
                    "missing Authorization header",
                    "missing_authorization_header",
                );
            }
        };
        let Some(token) = authorization
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return AuthFuture::unauthorized_msg(
                "invalid Authorization header",
                "invalid_authorization_header",
            );
        };
        match self.auth.verify_token(token) {
            Ok(_) => AuthFuture::authorized(self.inner.call(req)),
            Err(err) => AuthFuture::unauthorized_error(err),
        }
    }
}
#[pin_project(project = AuthFutureProj)]
pub enum AuthFuture<F> {
    Authorized {
        #[pin]
        inner: F,
    },
    Unauthorized(Option<Response>),
}

impl<F> AuthFuture<F> {
    fn unauthorized_msg(message: &'static str, code: &'static str) -> Self {
        Self::Unauthorized(Some(Error::unauthorized(message, code).into_response()))
    }

    fn unauthorized_error(error: Error) -> Self {
        Self::Unauthorized(Some(error.into_response()))
    }

    fn authorized(inner: F) -> Self {
        Self::Authorized { inner }
    }
}

impl<F, E> Future for AuthFuture<F>
where
    F: Future<Output = Result<Response, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this {
            AuthFutureProj::Authorized { inner } => inner.poll(cx),
            AuthFutureProj::Unauthorized(response) => {
                let response = response.take().expect("AuthFuture polled after completion");
                Poll::Ready(Ok(response))
            }
        }
    }
}
