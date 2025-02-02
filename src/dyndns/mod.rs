use std::{
    net::{Ipv4Addr, Ipv6Addr},
    time::Duration,
};

use once_cell::sync::Lazy;
use reqwest::Client;
use tokio::sync::mpsc::Receiver;

mod api;
mod check;
mod v4;
mod v6;

pub use crate::Error;
use tokio::{task, time};

use crate::{
    db::{DynDNS, History, IpVersion},
    DbPool,
};

use self::{api::DynDNSAPI, check::CheckResult};

pub static CLIENT_V4: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .local_address("0.0.0.0:0".parse().ok())
        .build()
        .unwrap()
});

pub static CLIENT_V6: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(5))
        .local_address("[::]:0".parse().ok())
        .build()
        .unwrap()
});

pub fn launch(pool: DbPool, rx: Receiver<u64>) -> task::JoinHandle<()> {
    info!("dyn_dns api start");
    tokio::spawn(check_loop(pool, rx))
}

async fn check_loop(pool: DbPool, rx: Receiver<u64>) {
    let mut rx = rx;
    loop {
        if let Err(e) = check(&pool).await {
            error!("{}", e);
        }
        let secs = match pool.get().await {
            Ok(conn) => match DynDNS::get_sleep_interval(&conn).await {
                Ok(v) => Ok(v as u64),
                Err(e) => Err(e.into()),
            },
            Err(e) => Err(e.into()),
        }
        .unwrap_or_else(|e: Error| {
            error!("{}", e);
            10
        });
        listen_interval(&mut rx, secs).await;
        debug!("wake");
    }
}

async fn listen_interval(rx: &mut Receiver<u64>, secs: u64) {
    debug!("sleep {}s", secs);
    let mut interval = time::interval(time::Duration::from_secs(secs));
    let instant = interval.tick().await;
    loop {
        tokio::select! {
            _ = async {
                interval.tick().await;
            } => {
                return;
            },
            v = async  {
                if let Some(v) = rx.recv().await {
                    return Some(v);
                }
                None
            } => {
                if let Some(v) = v {
                    debug!("new iterval {}s", v);
                    interval = time::interval_at(instant, time::Duration::from_secs(v));
                    interval.tick().await;
                }
            }
        };
    }
}

async fn join(
    pool: &DbPool,
    enable: IpVersion,
    interface: String,
) -> (CheckResult<Ipv4Addr>, CheckResult<Vec<Ipv6Addr>>) {
    let v4 = v4::Params {
        pool: pool.clone(),
        enable: enable.clone(),
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

    if v4.is_change() || v6.is_change() {
        let dyn_dns_api = DynDNSAPI::new(
            config.server,
            config.username,
            config.password,
            config.hostname,
        );
        update(dyn_dns_api, pool, v4, v6).await?;
    }
    Ok(())
}

async fn update(
    mut dyn_dns_api: DynDNSAPI,
    pool: &DbPool,
    v4: CheckResult<Ipv4Addr>,
    v6: CheckResult<Vec<Ipv6Addr>>,
) -> Result<(), Error> {
    let conn = pool.get().await?;
    if let Some(new) = v4.new {
        info!("ipv4 address changed, start update: {}", &new);
        if dyn_dns_api.update_v4(&new).await? {
            History::insert_v4(&conn, v4.old, new).await?;
        }
    }
    if let Some(new) = v6.new {
        info!("ipv6 address changed, start update: {:#?}", &new);
        if dyn_dns_api.update_v6().await? {
            History::insert_v6(&conn, v6.old, new).await?;
        }
    }

    Ok(())
}

async fn get_dyn_dns_config(pool: &DbPool) -> Result<DynDNS, Error> {
    let conn = pool.get().await?;
    Ok(DynDNS::get(&conn).await?)
}
