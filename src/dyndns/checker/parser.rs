use std::net::{Ipv4Addr, Ipv6Addr};

use crate::Error;

pub trait IpLookupParser<T>: Send + Sync {
    fn parse(&self, body: &str) -> Result<T, Error>;
}

#[derive(Default, Clone, Copy)]
pub struct PlainTextIpParser;

impl IpLookupParser<Ipv4Addr> for PlainTextIpParser {
    fn parse(&self, body: &str) -> Result<Ipv4Addr, Error> {
        let trimmed = body.trim();
        trimmed
            .parse()
            .map_err(|_err| Error::ipv4_parse_error(trimmed))
    }
}

impl IpLookupParser<Ipv6Addr> for PlainTextIpParser {
    fn parse(&self, body: &str) -> Result<Ipv6Addr, Error> {
        let trimmed = body.trim();
        trimmed
            .parse()
            .map_err(|_err| Error::ipv6_parse_error(trimmed))
    }
}
