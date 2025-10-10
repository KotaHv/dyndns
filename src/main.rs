#[macro_use]
extern crate tracing;

use std::{env, sync::Arc};

use axum::{Router, extract::FromRef};

use axum_extra::middleware::option_layer;
use dotenvy::dotenv;
use tokio::{
    net::TcpListener,
    sync::mpsc::{self, Sender},
};
use tower::ServiceBuilder;
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
    let layer = ServiceBuilder::new().layer(trace::TraceLayer).layer(cors);
    let (tx, rx) = mpsc::channel::<u64>(1);
    let state = AppState {
        pool: pool.clone(),
        tx,
        auth,
    };
    let app = Router::new()
        .nest("/api", api::routes(&state))
        .fallback_service(ServeDir::new(&CONFIG.web_dir))
        .layer(layer)
        .with_state(state);

    let listener = TcpListener::bind(config::CONFIG.addr).await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    info!("listening on http://{}", local_addr);
    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(err) = result {
                error!("server error: {}", err);
            }
        },
        _ = dyndns::launch(pool, rx) => {}
    };
}

#[derive(FromRef, Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub tx: Sender<u64>,
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
