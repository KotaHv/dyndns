use std::time::Duration;

use tokio::{sync::watch, time};

use crate::{
    DbPool, Error,
    db::{DynDNS, History, IpVersion},
};

use super::{
    checker::{
        ipv4::{Ipv4CheckResult, Ipv4Checker},
        ipv6::{Ipv6CheckResult, Ipv6Checker, Ipv6HistorySnapshot, parse_ipv6_list},
        run_checker,
    },
    http_client::HttpClient,
    updater::{DynDnsAuth, DynDnsUpdater},
};

pub async fn launch(
    pool: DbPool,
    interval_rx: watch::Receiver<u64>,
    shutdown_rx: watch::Receiver<bool>,
) {
    info!("DynDNS scheduler start");
    let scheduler = DynDnsScheduler::new(pool, interval_rx, shutdown_rx).await;
    scheduler.run().await;
    info!("DynDNS scheduler stop");
}

pub struct DynDnsScheduler {
    pool: DbPool,
    client: HttpClient,
    interval_rx: watch::Receiver<u64>,
    shutdown_rx: watch::Receiver<bool>,
    interval_secs: u64,
}

impl DynDnsScheduler {
    async fn new(
        pool: DbPool,
        interval_rx: watch::Receiver<u64>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        let client = HttpClient::new(3, Duration::from_millis(200));
        let interval_secs = Self::load_interval_seconds(&pool).await;
        Self {
            pool,
            client,
            interval_rx,
            shutdown_rx,
            interval_secs,
        }
    }

    async fn run(mut self) {
        loop {
            let mut interval = time::interval(Duration::from_secs(self.interval_secs));
            let start_time = interval.tick().await;

            if let Err(err) = self.execute_cycle().await {
                error!("{}", err);
            }

            debug!("sleep {}s", self.interval_secs);

            let mut shutdown = self.shutdown_rx.clone();
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

    async fn execute_cycle(&self) -> Result<(), Error> {
        let config = self.load_config().await?;
        let ipv4_previous = self.load_ipv4_history().await?;
        let ipv6_history = self.load_ipv6_history().await?;

        let client = &self.client;
        let interface = config.interface.as_str();
        let run_ipv4 = matches!(config.ip, IpVersion::V4 | IpVersion::ALL);
        let run_ipv6 = matches!(config.ip, IpVersion::V6 | IpVersion::ALL);

        let (ipv4_result, ipv6_result) = match (run_ipv4, run_ipv6) {
            (true, true) => {
                let ipv4_checker = Ipv4Checker::new(client, interface, ipv4_previous);
                let ipv6_checker = Ipv6Checker::new(client, interface, ipv6_history);
                tokio::join!(run_checker(ipv4_checker), run_checker(ipv6_checker))
            }
            (true, false) => {
                let ipv4_checker = Ipv4Checker::new(client, interface, ipv4_previous);
                (run_checker(ipv4_checker).await, Ipv6CheckResult::default())
            }
            (false, true) => {
                let ipv6_checker = Ipv6Checker::new(client, interface, ipv6_history);
                (Ipv4CheckResult::default(), run_checker(ipv6_checker).await)
            }
            (false, false) => (Ipv4CheckResult::default(), Ipv6CheckResult::default()),
        };

        let auth = DynDnsAuth::from(&config);
        let updater = DynDnsUpdater::new(&self.client, auth, config.hostname.as_str());
        let updated = updater.apply(&ipv4_result, &ipv6_result).await?;

        if updated {
            self.persist_history(&ipv4_result, &ipv6_result).await?;
        }

        Ok(())
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
                    interval = time::interval_at(start_time, Duration::from_secs(self.interval_secs));
                    interval.tick().await;
                }
            }
        }
    }

    async fn load_config(&self) -> Result<DynDNS, Error> {
        let conn = self.pool.get().await?;
        DynDNS::get(&conn).await
    }

    async fn load_interval_seconds(pool: &DbPool) -> u64 {
        match pool.get().await {
            Ok(conn) => match DynDNS::get_sleep_interval(&conn).await {
                Ok(value) => value.into(),
                Err(err) => {
                    error!("{}", err);
                    10
                }
            },
            Err(err) => {
                error!("{}", err);
                10
            }
        }
    }

    async fn load_ipv4_history(&self) -> Result<Option<std::net::Ipv4Addr>, Error> {
        use std::net::Ipv4Addr;

        let conn = self.pool.get().await?;
        let record = History::get_v4(&conn).await?;
        Ok(record.and_then(|value| value.parse::<Ipv4Addr>().ok()))
    }

    async fn load_ipv6_history(&self) -> Result<Option<Ipv6HistorySnapshot>, Error> {
        let conn = self.pool.get().await?;
        let record = History::get_v6(&conn).await?;
        Ok(record.map(|(previous, latest)| {
            let previous_parsed = previous.map(|value| parse_ipv6_list(&value));
            let latest_parsed = parse_ipv6_list(&latest);
            Ipv6HistorySnapshot::new(previous_parsed, latest_parsed)
        }))
    }

    async fn persist_history(
        &self,
        ipv4: &Ipv4CheckResult,
        ipv6: &Ipv6CheckResult,
    ) -> Result<(), Error> {
        let conn = self.pool.get().await?;

        if let Some(new) = ipv4.current.as_ref() {
            History::insert_v4(&conn, &ipv4.previous, new).await?;
        }

        if let Some(new) = ipv6.current.as_ref() {
            History::insert_v6(&conn, &ipv6.previous, new).await?;
        }

        Ok(())
    }
}
