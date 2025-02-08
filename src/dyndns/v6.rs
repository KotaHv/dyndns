use std::net::{IpAddr, Ipv6Addr};

use async_trait::async_trait;
use local_ip_address::list_afinet_netifas;
use tokio::task::spawn_blocking;

use crate::{
    db::{History, IpVersion},
    DbPool,
};

use super::{
    check::{CheckIpTrait, CheckResultTrait, GetIpTrait},
    Error, CLIENT,
};

static LOOKUP_URL: &'static str = "https://api-ipv6.ip.sb/ip";

#[derive(Debug, Default)]
pub struct Ipv6CheckResult {
    old: Option<Vec<Ipv6Addr>>,
    new: Option<Vec<Ipv6Addr>>,
    external: Option<Ipv6Addr>,
}

impl CheckResultTrait for Ipv6CheckResult {
    type IpType = Option<Vec<Ipv6Addr>>;

    fn old(&self) -> &Self::IpType {
        &self.old
    }

    fn new(&self) -> &Self::IpType {
        &self.new
    }
    fn is_changed(&self) -> bool {
        self.new.is_some()
    }
}

impl Ipv6CheckResult {
    pub fn external(&self) -> Option<Ipv6Addr> {
        self.external
    }
}

pub struct Params {
    pub db_pool: DbPool,
    pub enable: IpVersion,
    pub interface: String,
}

#[async_trait]
impl GetIpTrait for Params {
    type NewIp = Vec<Ipv6Addr>;
    type OldIp = (Option<Vec<Ipv6Addr>>, Vec<Ipv6Addr>);
    async fn get_new_ip(&self) -> Result<Self::NewIp, Error> {
        let interface = self.interface.clone();
        spawn_blocking(move || get_ipv6_addresses(&interface)).await?
    }
    async fn get_old_ip(&self) -> Result<Option<Self::OldIp>, Error> {
        let conn = self.db_pool.get().await?;
        Ok(History::get_v6(&conn)
            .await?
            .map(|(old_ip_opt, new_ip_str)| {
                (
                    old_ip_opt.map(|old_ip_str| parse_ipv6_list(&old_ip_str)),
                    parse_ipv6_list(&new_ip_str),
                )
            }))
    }
}

#[async_trait]
impl CheckIpTrait for Params {
    type ResultType = Ipv6CheckResult;
    async fn check_result(&self) -> Result<Ipv6CheckResult, Error> {
        let mut check_result = Ipv6CheckResult::default();
        if let IpVersion::V4 = self.enable {
            return Ok(check_result);
        }
        debug!("check v6");

        let previous_ips_opt = self.get_old_ip().await?;
        debug!("{:?}", &previous_ips_opt);

        let current_ips = self.get_new_ip().await?;

        let (previous_ips, new_ips) = match previous_ips_opt {
            Some((db_old_ips_opt, db_new_ips)) => {
                let mut existing_ips = vec![];

                if let Some(db_old_ips) = &db_old_ips_opt {
                    for prev_ip in db_old_ips {
                        existing_ips.push(prev_ip)
                    }
                }

                for curr_ip in &db_new_ips {
                    existing_ips.push(curr_ip)
                }

                let (new_ips, mut previous_ips): (Vec<Ipv6Addr>, Vec<Ipv6Addr>) = current_ips
                    .into_iter()
                    .partition(|ip| !existing_ips.contains(&ip));

                if previous_ips.is_empty() {
                    previous_ips = db_new_ips;
                }
                (Some(previous_ips), (!new_ips.is_empty()).then_some(new_ips))
            }
            None => (None, Some(current_ips)),
        };
        check_result.new = new_ips;
        check_result.old = previous_ips;
        if check_result.is_changed() {
            check_result.external = get_external_ipv6().await;
        }
        Ok(check_result)
    }
}

async fn get_external_ipv6() -> Option<Ipv6Addr> {
    let res = CLIENT.get(LOOKUP_URL).send().await.ok();
    let ip_str = res?.text().await.ok();
    ip_str?.trim().parse().ok()
}

fn get_ipv6_addresses(interface: &str) -> Result<Vec<Ipv6Addr>, Error> {
    let ifas = list_afinet_netifas().unwrap();
    let mut ipv6_addresses = vec![];
    for (name, ip) in ifas {
        if let IpAddr::V6(v6) = ip {
            if name == interface && (v6.segments()[0] & 0xffc0) != 0xfe80 {
                ipv6_addresses.push(v6);
            }
        }
    }
    ipv6_addresses
        .is_empty()
        .then(|| Err(Error::Ipv6NotFound))
        .unwrap_or(Ok(ipv6_addresses))
}

fn parse_ipv6_list(input: &str) -> Vec<Ipv6Addr> {
    input
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}
