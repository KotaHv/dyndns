use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::http::{Request, Response};
use chrono::{Local, SecondsFormat};
use futures_util::ready;
use pin_project::pin_project;
use tokio::time::Instant;
use tower::{Layer, Service};
use tracing_subscriber::{
    filter::Targets,
    fmt::{self, time},
    prelude::*,
};
use yansi::Paint;

use crate::CONFIG;

pub fn init() {
    let is_color = CONFIG.log.style.is_color();
    if !is_color {
        yansi::disable();
    }
    let format = fmt::layer().with_timer(LocalTime).with_ansi(is_color);
    let level = CONFIG.log.level.as_str();
    let filter: Targets = match level.parse() {
        Ok(f) => f,
        Err(e) => {
            let err = format!("string {} did not parse successfully: {}", level, e);
            panic!("{}", err.red().bold());
        }
    };

    tracing_subscriber::registry()
        .with(format)
        .with(filter)
        .init();
}

struct LocalTime;

impl time::FormatTime for LocalTime {
    fn format_time(&self, w: &mut fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(
            w,
            "{}",
            Local::now().to_rfc3339_opts(SecondsFormat::Millis, false)
        )
    }
}

#[derive(Clone)]
pub struct TraceLayer;

impl<S> Layer<S> for TraceLayer {
    type Service = TraceMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct TraceMiddleware<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for TraceMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = TraceFuture<S::Future>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }
    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let start = Instant::now();
        let method = req.method().to_string();
        let path = req.uri().path().to_string();
        let response_future = self.inner.call(req);
        TraceFuture {
            response_future,
            method,
            path,
            start,
        }
    }
}

#[pin_project]
pub struct TraceFuture<F> {
    #[pin]
    response_future: F,
    start: Instant,
    method: String,
    path: String,
}

impl<F, ResBody, E> Future for TraceFuture<F>
where
    F: Future<Output = Result<Response<ResBody>, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let res = ready!(this.response_future.poll(cx)?);
        let status = res.status();
        let status = match status.as_u16() {
            100..=199 => status.blue(),
            200..=299 => status.green(),
            300..=399 => status.cyan(),
            400..=499 => status.yellow(),
            _ => status.red(),
        };
        info!(
            method = ?this.method.green(),
            path = ?this.path.blue(),
            status = ?status,
            elapsed = ?this.start.elapsed().rgb(248, 200, 220)
        );
        Poll::Ready(Ok(res))
    }
}
