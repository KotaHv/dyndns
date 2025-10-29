use std::{
    collections::HashSet,
    net::{IpAddr, Ipv6Addr},
};

use isahc::{
    HttpClient, Request,
    config::{Configurable, NetworkInterface},
    prelude::AsyncReadResponseExt,
};
use local_ip_address::list_afinet_netifas;

use crate::Error;

use super::{CheckResult, IpChecker};

const LOOKUP_URL: &str = "https://api-ipv6.ip.sb/ip";

pub type Ipv6CheckResult =
    CheckResult<Option<Vec<Ipv6Addr>>, Option<Vec<Ipv6Addr>>, Option<Ipv6Addr>>;

pub struct Ipv6Checker<'a> {
    client: &'a HttpClient,
    interface: &'a str,
    history: Option<Ipv6HistorySnapshot>,
}

impl<'a> Ipv6Checker<'a> {
    pub fn new(
        client: &'a HttpClient,
        interface: &'a str,
        history: Option<Ipv6HistorySnapshot>,
    ) -> Self {
        Self {
            client,
            interface,
            history,
        }
    }

    fn get_interface_addresses(interface: &str) -> Result<Vec<Ipv6Addr>, Error> {
        let ifaces = list_afinet_netifas()?;
        let mut ipv6_addresses = vec![];
        for (name, ip) in ifaces {
            if let IpAddr::V6(addr) = ip {
                if name == interface && (addr.segments()[0] & 0xffc0) != 0xfe80 {
                    ipv6_addresses.push(addr);
                }
            }
        }

        if ipv6_addresses.is_empty() {
            return Err(Error::Ipv6NotFound);
        }

        Ok(ipv6_addresses)
    }

    async fn get_external_address(client: &HttpClient, interface: &str) -> Result<Ipv6Addr, Error> {
        let request = Request::get(LOOKUP_URL)
            .interface(NetworkInterface::name(interface))
            .body(())
            .unwrap();
        let mut response = client.send_async(request).await?;
        let ip = response.text().await?;
        ip.trim().parse().map_err(|_err| Error::IPv6ParseError(ip))
    }
}

impl<'a> IpChecker for Ipv6Checker<'a> {
    type Previous = Option<Vec<Ipv6Addr>>;
    type Current = Option<Vec<Ipv6Addr>>;
    type External = Option<Ipv6Addr>;

    async fn check(self) -> Result<Ipv6CheckResult, Error> {
        debug!("check v6");

        let Self {
            client,
            interface,
            history,
        } = self;

        let interface_addresses = Self::get_interface_addresses(interface)?;

        let (previous_addresses, current_addresses) =
            if let Some(Ipv6HistorySnapshot { previous, latest }) = history {
                let estimated = latest.len() + previous.as_ref().map_or(0, |p| p.len());
                let mut known_addresses: HashSet<&Ipv6Addr> = HashSet::with_capacity(estimated);

                for addr in &latest {
                    known_addresses.insert(addr);
                }

                if let Some(ref prev) = previous {
                    for addr in prev {
                        known_addresses.insert(addr);
                    }
                }

                let (current, retained): (Vec<Ipv6Addr>, Vec<Ipv6Addr>) = interface_addresses
                    .into_iter()
                    .partition(|addr| !known_addresses.contains(addr));

                let current = (!current.is_empty()).then_some(current);
                let previous = if retained.is_empty() {
                    Some(latest)
                } else {
                    Some(retained)
                };

                (previous, current)
            } else {
                (None, Some(interface_addresses))
            };

        let external = match current_addresses.as_ref() {
            Some(current) if current.len() == 1 => Some(current[0]),
            Some(_) => Some(Self::get_external_address(client, interface).await?),
            None => None,
        };

        if external.is_some() {
            debug!("external ipv6 address: {:?}", &external);
        }

        Ok(Ipv6CheckResult::new(
            previous_addresses,
            current_addresses,
            external,
        ))
    }
}

#[derive(Clone, Debug)]
pub struct Ipv6HistorySnapshot {
    pub previous: Option<Vec<Ipv6Addr>>,
    pub latest: Vec<Ipv6Addr>,
}

impl Ipv6HistorySnapshot {
    pub fn new(previous: Option<Vec<Ipv6Addr>>, latest: Vec<Ipv6Addr>) -> Self {
        Self { previous, latest }
    }
}

pub fn parse_ipv6_list(input: &str) -> Vec<Ipv6Addr> {
    input
        .split(',')
        .filter_map(|value| value.trim().parse().ok())
        .collect()
}
