use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use local_ip_address::list_afinet_netifas;
use reqwest::Client;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;
use tokio::{task, time};

mod api;
mod check;
mod v4;
mod v6;

pub use crate::Error;

use crate::{
    db::{DynDNS, History, IpVersion},
    init_dbpool, DbPool,
};

use self::{api::DynDNSAPI, check::CheckResultTrait, v4::Ipv4CheckResult, v6::Ipv6CheckResult};

pub struct HttpClient {
    inner: Arc<Client>,
    db_pool: DbPool,
    interface: String,
}

impl HttpClient {
    async fn new() -> Mutex<Self> {
        let db_pool = init_dbpool();
        let config = get_dyn_dns_config(&db_pool).await.unwrap();
        let interface = config.interface;
        let inner = match get_interface_address(&interface) {
            Some(ip) => Client::builder()
                .timeout(Duration::from_secs(5))
                .local_address(ip)
                .build()
                .unwrap(),
            None => Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
        };
        Mutex::new(Self {
            inner: Arc::new(inner),
            db_pool,
            interface,
        })
    }
    pub async fn client(&mut self) -> Arc<Client> {
        let config = get_dyn_dns_config(&self.db_pool).await.unwrap();
        let interface = config.interface;
        if &interface != &self.interface {
            let inner = match get_interface_address(&interface) {
                Some(ip) => Client::builder()
                    .timeout(Duration::from_secs(5))
                    .local_address(ip)
                    .build()
                    .unwrap(),
                None => Client::builder()
                    .timeout(Duration::from_secs(5))
                    .build()
                    .unwrap(),
            };
            self.inner = Arc::new(inner);
        }
        self.inner.clone()
    }
}

static CLIENT: OnceCell<Mutex<HttpClient>> = OnceCell::const_new();

pub async fn get_http_client() -> Arc<Client> {
    let client = CLIENT.get_or_init(HttpClient::new).await;
    client.lock().await.client().await
}

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
) -> (Ipv4CheckResult, Ipv6CheckResult) {
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

fn get_interface_address(interface: &String) -> Option<IpAddr> {
    let ifas = list_afinet_netifas().unwrap();
    let mut address = None;
    for (name, ip) in ifas {
        if &name == interface && ip.is_ipv4() && !ip.is_loopback() {
            debug!("{:?}", &ip);
            address = Some(ip);
            break;
        }
    }
    address
}
