use std::net::Ipv4Addr;

use async_trait::async_trait;

use super::check::{CheckIp, CheckResult, GetIp};
use super::{Error, CLIENT_V4};
use crate::{
    db::{History, IpVersion},
    DbPool,
};

static IPV4_URL: &'static str = "https://api-ipv4.ip.sb/ip";

pub struct Params {
    pub pool: DbPool,
    pub enable: IpVersion,
}

#[async_trait]
impl GetIp for Params {
    type NewIp = Ipv4Addr;
    type OldIp = Ipv4Addr;
    async fn get_new_ip(&self) -> Result<Self::NewIp, Error> {
        let res = CLIENT_V4.get(IPV4_URL).send().await?;
        let ip_str = res.text().await?;
        Ok(ip_str
            .trim()
            .parse()
            .map_err(|_e| Error::IPv4ParseError(ip_str))?)
    }
    async fn get_old_ip(&self) -> Result<Option<Self::OldIp>, Error> {
        let conn = self.pool.get().await?;
        let ip = History::get_v4(&conn).await?;
        if let Some(ip) = ip {
            return Ok(ip.parse::<Ipv4Addr>().ok());
        }
        Ok(None)
    }
}

#[async_trait]
impl CheckIp<Ipv4Addr> for Params {
    async fn check_result(&self) -> Result<CheckResult<Ipv4Addr>, Error> {
        let mut result = CheckResult::default();
        if let IpVersion::V6 = self.enable {
            return Ok(result);
        }
        debug!("check v4");

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
