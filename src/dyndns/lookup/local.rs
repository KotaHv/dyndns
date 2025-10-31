use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use local_ip_address::list_afinet_netifas;

use crate::Error;

use super::IpLookup;

pub struct LocalIpv6Lookup<'a> {
    interface: &'a str,
}

impl<'a> LocalIpv6Lookup<'a> {
    pub fn new(interface: &'a str) -> Self {
        Self { interface }
    }
}

impl<'a> IpLookup<Vec<Ipv6Addr>> for LocalIpv6Lookup<'a> {
    async fn lookup(&self) -> Result<Vec<Ipv6Addr>, Error> {
        let ifaces = list_afinet_netifas()?;
        let mut ipv6_addresses = vec![];
        for (name, ip) in ifaces {
            if let IpAddr::V6(addr) = ip {
                if name == self.interface && (addr.segments()[0] & 0xffc0) != 0xfe80 {
                    ipv6_addresses.push(addr);
                }
            }
        }

        if ipv6_addresses.is_empty() {
            Err(Error::ipv6_not_found())
        } else {
            Ok(ipv6_addresses)
        }
    }
}

#[allow(dead_code)]
pub struct LocalIpv4Lookup<'a> {
    interface: &'a str,
}

#[allow(dead_code)]
impl<'a> LocalIpv4Lookup<'a> {
    pub fn new(interface: &'a str) -> Self {
        Self { interface }
    }
}

impl<'a> IpLookup<Ipv4Addr> for LocalIpv4Lookup<'a> {
    async fn lookup(&self) -> Result<Ipv4Addr, Error> {
        let ifaces = list_afinet_netifas()?;
        for (name, ip) in ifaces {
            if name == self.interface {
                if let IpAddr::V4(addr) = ip {
                    if addr.is_private() || addr.is_loopback() || addr.is_link_local() {
                        continue;
                    }

                    return Ok(addr);
                }
            }
        }
        Err(Error::ipv4_not_found())
    }
}
