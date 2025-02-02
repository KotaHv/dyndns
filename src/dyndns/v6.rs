use std::collections::HashSet;
use std::net::{IpAddr, Ipv6Addr};

use async_trait::async_trait;
use local_ip_address::list_afinet_netifas;
use tokio::task::spawn_blocking;

use crate::{
    db::{History, IpVersion},
    DbPool,
};

use super::{
    check::{CheckIp, GetIp},
    Error,
};

pub struct Params {
    pub db_pool: DbPool,
    pub enable: IpVersion,
    pub interface: String,
}

#[async_trait]
impl GetIp for Params {
    type Ip = Vec<Ipv6Addr>;
    async fn get_new_ip(&self) -> Result<Self::Ip, Error> {
        let interface = self.interface.clone();
        spawn_blocking(move || get_ipv6_addresses(&interface)).await?
    }
    async fn get_old_ip(&self) -> Result<Option<Self::Ip>, Error> {
        let conn = self.db_pool.get().await?;
        Ok(History::get_v6(&conn)
            .await?
            .map(|ip_str| parse_ipv6_list(&ip_str)))
    }
}

#[async_trait]
impl CheckIp<Vec<Ipv6Addr>> for Params {
    async fn check_result(&self) -> Result<super::check::CheckResult<Vec<Ipv6Addr>>, Error> {
        let mut result = super::check::CheckResult::default();
        if let IpVersion::V4 = self.enable {
            return Ok(result);
        }
        debug!("check v6");

        let old_ips = self.get_old_ip().await?;
        debug!("{:?}", &old_ips);
        let new_ips = self.get_new_ip().await?;

        let changed_ips = match &old_ips {
            Some(existing) => find_new_ips(existing, &new_ips),
            None => new_ips,
        };

        result.old = old_ips;
        result.new = (!changed_ips.is_empty()).then_some(changed_ips);

        Ok(result)
    }
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

fn find_new_ips(existing: &[Ipv6Addr], new: &[Ipv6Addr]) -> Vec<Ipv6Addr> {
    let existing_set: HashSet<&Ipv6Addr> = existing.iter().collect();
    new.iter()
        .filter(|ip| !existing_set.contains(ip))
        .copied()
        .collect()
}
