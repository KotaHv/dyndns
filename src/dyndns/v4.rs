use std::net::Ipv4Addr;

use async_trait::async_trait;
use serde::Deserialize;

use super::check::{CheckIp, CheckResult, GetIp};
use super::{Error, CLIENT_V4};
use crate::{
    db::{History, IpVersion},
    DbPool,
};

static IPV4_URL: &'static str = "https://myip4.ipip.net/ip";

#[derive(Deserialize)]
struct IpEnableJson {
    ip: Ipv4Addr,
}
pub struct Params {
    pub pool: DbPool,
    pub enable: IpVersion,
}

#[async_trait]
impl GetIp for Params {
    type Ip = Ipv4Addr;
    async fn get_new_ip(&self) -> Result<Self::Ip, Error> {
        let res = CLIENT_V4
            .get(IPV4_URL)
            .send()
            .await?
            .json::<IpEnableJson>()
            .await?;
        Ok(res.ip)
    }
    async fn get_old_ip(&self) -> Result<Option<Self::Ip>, Error> {
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
