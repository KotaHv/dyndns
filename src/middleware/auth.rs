use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    extract::Request,
    http::header::AUTHORIZATION,
    response::{IntoResponse, Response},
};
use pin_project::pin_project;
use tower::{Layer, Service};

use crate::{Error, auth::AuthManager};

#[derive(Clone)]
pub struct AuthLayer {
    auth: Arc<AuthManager>,
}

impl AuthLayer {
    pub fn new(auth: Arc<AuthManager>) -> Self {
        Self { auth }
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
