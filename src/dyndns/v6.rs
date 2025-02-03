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
impl CheckIp<Vec<Ipv6Addr>> for Params {
    async fn check_result(&self) -> Result<super::check::CheckResult<Vec<Ipv6Addr>>, Error> {
        let mut check_result = super::check::CheckResult::default();
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

        Ok(check_result)
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
