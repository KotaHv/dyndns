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
    pub pool: DbPool,
    pub enable: IpVersion,
    pub interface: String,
}

#[async_trait]
impl GetIp for Params {
    type Ip = Ipv6Addr;
    async fn get_new_ip(&self) -> Result<Self::Ip, Error> {
        let interface = self.interface.clone();
        spawn_blocking(move || get_new_ip_blocking(interface)).await?
    }
    async fn get_old_ip(&self) -> Result<Option<Self::Ip>, Error> {
        let conn = self.pool.get().await?;
        let ip = History::get_v6(&conn).await?;
        if let Some(ip) = ip {
            return Ok(ip.parse::<Ipv6Addr>().ok());
        }
        Ok(None)
    }
}

#[async_trait]
impl CheckIp<Ipv6Addr> for Params {
    async fn check_result(&self) -> Result<super::check::CheckResult<Ipv6Addr>, Error> {
        let mut result = super::check::CheckResult::default();
        if let IpVersion::V4 = self.enable {
            return Ok(result);
        }
        debug!("check v6");

        let old_ip = self.get_old_ip().await?;
        debug!("{:?}", old_ip);
        let new_ip = self.get_new_ip().await?;

        if let Some(old_ip) = old_ip {
            if old_ip == new_ip {
                return Ok(result);
            }
        }

        result.old = old_ip;
        result.new = Some(new_ip);

        Ok(result)
    }
}

fn get_new_ip_blocking(interface: String) -> Result<Ipv6Addr, Error> {
    let ifas = list_afinet_netifas().unwrap();
    #[cfg(not(target_os = "macos"))]
    {
        for (name, ip) in ifas.iter() {
            if let IpAddr::V6(v6) = ip {
                if name == &interface && (v6.segments()[0] & 0xffc0) != 0xfe80 {
                    return Ok(v6.to_owned());
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut ipv6_list = vec![];
        for (name, ip) in ifas.iter() {
            if let IpAddr::V6(v6) = ip {
                if name == &interface && (v6.segments()[0] & 0xffc0) != 0xfe80 {
                    ipv6_list.push(v6.to_owned());
                }
            }
        }
        if !ipv6_list.is_empty() {
            if ipv6_list.len() == 1 {
                return Ok(ipv6_list[0]);
            }
            return Ok(ipv6_list[ipv6_list.len() - 2]);
        }
    }
    Err(Error::Ipv6NotFound)
}
