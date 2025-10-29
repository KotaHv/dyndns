use std::time::Duration;

use isahc::{AsyncBody, Request, RequestExt, Response, config::Configurable};

use crate::Error;

#[derive(Clone)]
pub(crate) struct HttpClient {
    inner: isahc::HttpClient,
    max_attempts: usize,
    initial_delay: Duration,
}

impl HttpClient {
    pub fn new(max_attempts: usize, initial_delay: Duration) -> Self {
        let inner = isahc::HttpClient::builder()
            .connect_timeout(Duration::from_secs(5))
            .default_header(
                "user-agent",
                format!("dyndns/{}", env!("CARGO_PKG_VERSION")),
            )
            .build()
            .unwrap();

        Self {
            inner,
            max_attempts: max_attempts.max(1),
            initial_delay,
        }
    }

    pub async fn send_async<B>(&self, request: Request<B>) -> Result<Response<AsyncBody>, Error>
    where
        B: Into<AsyncBody> + Clone + Send + Sync + 'static,
    {
        let mut delay = self.initial_delay;
        let body = request.body().clone();

        for attempt in 1..self.max_attempts {
            let builder = request.to_builder();
            let current_request = builder.body(body.clone()).unwrap();

            match self.inner.send_async(current_request).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    let err: Error = err.into();
                    warn!(
                        "http client attempt {}/{} failed: {}",
                        attempt, self.max_attempts, err
                    );

                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                        delay = delay.saturating_mul(2);
                    }
                }
            }
        }
        self.inner.send_async(request).await.map_err(|err| {
            let err: Error = err.into();
            error!("http client retries exhausted: {}", err);
            err
        })
    }
}
