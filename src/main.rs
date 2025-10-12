#[macro_use]
extern crate tracing;

use std::{env, sync::Arc};

use axum::{Router, extract::FromRef};

use axum_extra::middleware::option_layer;
use dotenvy::dotenv;
use tokio::{net::TcpListener, signal, sync::watch};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

mod api;
mod auth;
mod config;
mod db;
mod dyndns;
mod error;
mod middleware;
mod trace;
mod util;

pub use config::CONFIG;
pub use error::Error;

pub type DbPool = deadpool_diesel::sqlite::Pool;
pub type DbConn = deadpool_diesel::sqlite::Object;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    launch_info();
    dotenv().ok();
    trace::init();
    db::run_migrations().unwrap();
    let pool = init_dbpool();
    let auth = auth::AuthManager::new(&CONFIG.auth, pool.clone())
        .await
        .unwrap_or_else(|err| panic!("{}", err));
    let auth = Arc::new(auth);

    let cors = if CONFIG.debug {
        Some(
            CorsLayer::new()
                .allow_headers(Any)
                .allow_methods(Any)
                .allow_origin(Any),
        )
    } else {
        None
    };
    let cors = option_layer(cors);
    let (interval_tx, interval_rx) = watch::channel(0u64);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let state = AppState {
        pool: pool.clone(),
        interval_tx,
        auth,
    };
    let app = Router::new()
        .nest("/api", api::routes(&state))
        .fallback_service(ServeDir::new(&CONFIG.web_dir))
        .layer(middleware::trace::TraceLayer)
        .layer(cors)
        .with_state(state);

    let listener = TcpListener::bind(config::CONFIG.addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    info!("listening on http://{}", local_addr);
    let worker = tokio::spawn(dyndns::launch(pool, interval_rx, shutdown_rx.clone()));
    if let Err(err) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        error!("server error: {}", err);
    }
    info!("axum server stopped");
    debug!("notifying DynDNS worker to shutdown");
    if shutdown_tx.send(true).is_err() {
        warn!("failed to notify DynDNS worker, channel already closed");
    }
    if let Err(err) = worker.await {
        error!("failed to join DynDNS worker: {}", err);
    }
    info!("shutdown complete");
}

#[derive(FromRef, Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub interval_tx: watch::Sender<u64>,
    pub auth: Arc<auth::AuthManager>,
}

fn init_dbpool() -> DbPool {
    let manager = deadpool_diesel::sqlite::Manager::new(
        CONFIG.database_url.as_str(),
        deadpool_diesel::Runtime::Tokio1,
    );
    deadpool_diesel::sqlite::Pool::builder(manager)
        .build()
        .unwrap()
}

fn launch_info() {
    println!();
    println!(
        "=================== Starting DynDNS {} ===================",
        env!("CARGO_PKG_VERSION")
    );
    println!();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("received shutdown signal, initiating graceful shutdown");
}
