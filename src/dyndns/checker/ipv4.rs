use std::net::Ipv4Addr;

use crate::Error;

use super::super::{
    http_client::HttpClient,
    lookup::{HttpIpLookup, IpLookup, IpsbLookup, PlainTextIpParser},
};
use super::{CheckResult, IpChecker};

pub type Ipv4CheckResult = CheckResult<Option<Ipv4Addr>, Option<Ipv4Addr>, Option<Ipv4Addr>>;

pub struct Ipv4Checker<'a> {
    lookup: IpsbLookup<'a, Ipv4Addr>,
    previous_ip: Option<Ipv4Addr>,
}

impl<'a> Ipv4Checker<'a> {
    pub fn new(client: &'a HttpClient, interface: &'a str, previous_ip: Option<Ipv4Addr>) -> Self {
        Self {
            lookup: HttpIpLookup::<PlainTextIpParser, Ipv4Addr>::ipsb(client, interface),
            previous_ip,
        }
    }
}

impl<'a> IpChecker for Ipv4Checker<'a> {
    type Previous = Option<Ipv4Addr>;
    type Current = Option<Ipv4Addr>;
    type External = Option<Ipv4Addr>;

    async fn check(self) -> Result<Ipv4CheckResult, Error> {
        debug!("check v4");

        let Self {
            lookup,
            previous_ip,
        } = self;

        debug!("{:?}", previous_ip);
        let current_ip = lookup.lookup().await?;

        if let Some(existing) = previous_ip {
            if existing == current_ip {
                return Ok(Ipv4CheckResult::default());
            }
        }

        let previous = previous_ip;
        let current = Some(current_ip);
        let external = current;

        Ok(Ipv4CheckResult::new(previous, current, external))
    }
}
