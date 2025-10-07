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
        PasswordHashString, PasswordHasher, PasswordVerifier, SaltString,
        rand_core::{OsRng, RngCore},
    },
};
use axum::{
    extract::Request,
    http::{StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, errors::ErrorKind};
use pin_project::pin_project;
use serde::{Deserialize, Serialize};
use tower::{Layer, Service};

use crate::{Error, config::Auth as AuthConfig};

pub struct AuthManager {
    username: String,
    password_hash: PasswordHashString,
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    token_ttl: Duration,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

pub struct AuthToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

impl AuthManager {
    pub fn new(config: &AuthConfig) -> Result<Self, String> {
        if config.token_ttl_seconds == 0 {
            return Err("authentication token ttl must be greater than zero".into());
        }
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let encoding_key = EncodingKey::from_secret(&secret);
        let decoding_key = DecodingKey::from_secret(&secret);
        let token_ttl = Duration::from_std(StdDuration::from_secs(config.token_ttl_seconds))
            .map_err(|err| format!("invalid authentication token ttl: {err}"))?;
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
            password_hash,
        })
    }

    pub fn authenticate(&self, username: &str, password: &str) -> Result<AuthToken, Error> {
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

        let now = Utc::now();
        let expires_at = now + self.token_ttl;
        let claims = Claims {
            sub: self.username.clone(),
            exp: expires_at.timestamp(),
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
        Ok(AuthToken { token, expires_at })
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
