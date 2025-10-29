use std::net::Ipv4Addr;

use isahc::{
    Request,
    config::{Configurable, NetworkInterface},
    prelude::AsyncReadResponseExt,
};

use crate::Error;

use super::super::http_client::HttpClient;
use super::{CheckResult, IpChecker};

const LOOKUP_URL: &str = "https://api-ipv4.ip.sb/ip";

pub type Ipv4CheckResult = CheckResult<Option<Ipv4Addr>, Option<Ipv4Addr>, Option<Ipv4Addr>>;

pub struct Ipv4Checker<'a> {
    client: &'a HttpClient,
    interface: &'a str,
    previous_ip: Option<Ipv4Addr>,
}

impl<'a> Ipv4Checker<'a> {
    pub fn new(client: &'a HttpClient, interface: &'a str, previous_ip: Option<Ipv4Addr>) -> Self {
        Self {
            client,
            interface,
            previous_ip,
        }
    }

    async fn get_external_address(client: &HttpClient, interface: &str) -> Result<Ipv4Addr, Error> {
        let request = Request::get(LOOKUP_URL)
            .interface(NetworkInterface::name(interface))
            .body(())
            .unwrap();
        let mut response = client.send_async(request).await?;
        let body = response.text().await?;

        let trimmed = body.trim();
        trimmed
            .parse()
            .map_err(|_err| Error::ipv4_parse_error(body))
    }
}

impl<'a> IpChecker for Ipv4Checker<'a> {
    type Previous = Option<Ipv4Addr>;
    type Current = Option<Ipv4Addr>;
    type External = Option<Ipv4Addr>;

    async fn check(self) -> Result<Ipv4CheckResult, Error> {
        debug!("check v4");

        let Self {
            client,
            interface,
            previous_ip,
        } = self;

        debug!("{:?}", previous_ip);
        let current_ip = Self::get_external_address(client, interface).await?;

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
