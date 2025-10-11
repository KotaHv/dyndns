use std::time::Duration;

use isahc::{HttpClient, config::Configurable};
use once_cell::sync::Lazy;
use tokio::sync::watch;
use tokio::time;

mod api;
mod check;
mod v4;
mod v6;

pub use crate::Error;

use crate::{
    DbPool,
    db::{DynDNS, History, IpVersion},
};

use self::{api::DynDNSAPI, check::CheckResultTrait, v4::Ipv4CheckResult, v6::Ipv6CheckResult};

pub static CLIENT: Lazy<HttpClient> = Lazy::new(|| {
    HttpClient::builder()
        .timeout(Duration::from_secs(5))
        .default_header(
            "user-agent",
            format!("dyndns/{}", env!("CARGO_PKG_VERSION")),
        )
        .build()
        .unwrap()
});

pub async fn launch(
    pool: DbPool,
    interval_rx: watch::Receiver<u64>,
    shutdown: watch::Receiver<bool>,
) {
    info!("DynDNS API start");
    let worker = DynDnsWorker::new(pool, interval_rx, shutdown).await;
    worker.run().await;
    info!("DynDNS API stop");
}

struct DynDnsWorker {
    pool: DbPool,
    interval_rx: watch::Receiver<u64>,
    interval_secs: u64,
    shutdown: watch::Receiver<bool>,
}

impl DynDnsWorker {
    async fn new(
        pool: DbPool,
        interval_rx: watch::Receiver<u64>,
        shutdown: watch::Receiver<bool>,
    ) -> Self {
        let interval_secs = Self::load_sleep_interval(&pool).await;
        Self {
            pool,
            interval_rx,
            interval_secs,
            shutdown,
        }
    }

    async fn run(mut self) {
        loop {
            let mut interval = time::interval(time::Duration::from_secs(self.interval_secs));
            let start_time = interval.tick().await;
            if let Err(e) = check(&self.pool).await {
                error!("{}", e);
            }
            debug!("sleep {}s", self.interval_secs);
            let mut shutdown = self.shutdown.clone();
            tokio::select! {
                _ = shutdown.changed() => {
                    return;
                },
                _ = self.wait(start_time, interval) => {
                    debug!("wake");
                }
            }
        }
    }

    async fn wait(&mut self, start_time: time::Instant, mut interval: time::Interval) {
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    return;
                },
                Ok(_) = self.interval_rx.changed() => {
                    self.interval_secs = *self.interval_rx.borrow();
                    debug!("new interval {}s", self.interval_secs);
                    interval = time::interval_at(
                        start_time,
                        Duration::from_secs(self.interval_secs),
                    );
                    interval.tick().await;
                },
            }
        }
    }

    async fn load_sleep_interval(pool: &DbPool) -> u64 {
        match pool.get().await {
            Ok(conn) => match DynDNS::get_sleep_interval(&conn).await {
                Ok(v) => v as u64,
                Err(e) => {
                    error!("{}", e);
                    10
                }
            },
            Err(e) => {
                error!("{}", e);
                10
            }
        }
    }
}

async fn join(
    pool: &DbPool,
    enable: IpVersion,
    interface: String,
) -> (Ipv4CheckResult, Ipv6CheckResult) {
    let v4 = v4::Params {
        pool: pool.clone(),
        enable: enable.clone(),
        interface: interface.clone(),
    };
    let v6 = v6::Params {
        db_pool: pool.clone(),
        enable: enable.clone(),
        interface,
    };

    tokio::join!(check::check(v4), check::check(v6))
}

async fn check(pool: &DbPool) -> Result<(), Error> {
    let config = get_dyn_dns_config(&pool).await?;
    let enable = config.ip;
    let interface = config.interface;
    let (v4, v6) = join(pool, enable, interface).await;
    let mut dyndns_api = DynDNSAPI::new(
        config.server,
        config.username,
        config.password,
        config.hostname,
    );
    dyndns_api.params.myip.v4 = v4.new().clone();
    dyndns_api.params.myip.v6 = v6.external();

    if v4.is_changed() || v6.is_changed() {
        update(dyndns_api, pool, v4, v6).await?;
    }
    Ok(())
}

async fn update(
    dyn_dns_api: DynDNSAPI,
    pool: &DbPool,
    v4: Ipv4CheckResult,
    v6: Ipv6CheckResult,
) -> Result<(), Error> {
    info!(
        "ip address changed, start update: {}",
        &dyn_dns_api.params.myip
    );
    if dyn_dns_api.update().await? {
        info!("Successful update!");
        let conn = pool.get().await?;
        if let Some(new) = v4.new() {
            History::insert_v4(&conn, v4.old(), new).await?;
        }
        if let Some(new) = v6.new() {
            History::insert_v6(&conn, v6.old(), new).await?;
        }
    }

    Ok(())
}

async fn get_dyn_dns_config(pool: &DbPool) -> Result<DynDNS, Error> {
    let conn = pool.get().await?;
    Ok(DynDNS::get(&conn).await?)
}
