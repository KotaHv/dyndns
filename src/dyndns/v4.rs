use std::net::Ipv4Addr;

use async_trait::async_trait;

use super::check::{CheckIpTrait, CheckResultTrait, GetIpTrait};
use super::{Error, CLIENT};
use crate::{
    db::{History, IpVersion},
    DbPool,
};

static LOOKUP_URL: &'static str = "https://api-ipv4.ip.sb/ip";

#[derive(Debug, Default)]
pub struct Ipv4CheckResult {
    old: Option<Ipv4Addr>,
    new: Option<Ipv4Addr>,
}

impl CheckResultTrait for Ipv4CheckResult {
    type IpType = Option<Ipv4Addr>;

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

pub struct Params {
    pub pool: DbPool,
    pub enable: IpVersion,
}

#[async_trait]
impl GetIpTrait for Params {
    type NewIp = Ipv4Addr;
    type OldIp = Ipv4Addr;
    async fn get_new_ip(&self) -> Result<Self::NewIp, Error> {
        let res = CLIENT.get(LOOKUP_URL).send().await?;
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
impl CheckIpTrait for Params {
    type ResultType = Ipv4CheckResult;
    async fn check_result(&self) -> Result<Ipv4CheckResult, Error> {
        let mut result = Ipv4CheckResult::default();
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
