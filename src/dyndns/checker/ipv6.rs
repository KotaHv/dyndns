use std::{collections::HashSet, net::Ipv6Addr};

use crate::Error;

use super::super::{
    http_client::HttpClient,
    lookup::{HttpIpLookup, IpLookup, IpsbLookup, LocalIpv6Lookup, PlainTextIpParser},
};
use super::{CheckResult, IpChecker};

pub type Ipv6CheckResult =
    CheckResult<Option<Vec<Ipv6Addr>>, Option<Vec<Ipv6Addr>>, Option<Ipv6Addr>>;

pub struct Ipv6Checker<'a> {
    local_lookup: LocalIpv6Lookup<'a>,
    external_lookup: IpsbLookup<'a, Ipv6Addr>,
    history: Option<Ipv6HistorySnapshot>,
}

impl<'a> Ipv6Checker<'a> {
    pub fn new(
        client: &'a HttpClient,
        interface: &'a str,
        history: Option<Ipv6HistorySnapshot>,
    ) -> Self {
        Self {
            local_lookup: LocalIpv6Lookup::new(interface),
            external_lookup: HttpIpLookup::<PlainTextIpParser, Ipv6Addr>::ipsb(client, interface),
            history,
        }
    }
}

impl<'a> IpChecker for Ipv6Checker<'a> {
    type Previous = Option<Vec<Ipv6Addr>>;
    type Current = Option<Vec<Ipv6Addr>>;
    type External = Option<Ipv6Addr>;

    async fn check(self) -> Result<Ipv6CheckResult, Error> {
        debug!("check v6");

        let Self {
            local_lookup,
            external_lookup,
            history,
        } = self;

        let interface_addresses = local_lookup.lookup().await?;

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
            Some(_) => Some(external_lookup.lookup().await?),
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
