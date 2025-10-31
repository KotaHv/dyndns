use std::{
    marker::PhantomData,
    net::{Ipv4Addr, Ipv6Addr},
};

use isahc::{
    Request,
    config::{Configurable, NetworkInterface},
    prelude::AsyncReadResponseExt,
};

use crate::Error;
use crate::dyndns::http_client::HttpClient;

use super::{IpLookup, IpLookupParser, PlainTextIpParser};

pub struct HttpIpLookup<'a, P, T> {
    client: &'a HttpClient,
    interface: &'a str,
    url: &'static str,
    parser: P,
    _marker: PhantomData<T>,
}

impl<'a, P, T> HttpIpLookup<'a, P, T> {
    pub fn new(client: &'a HttpClient, interface: &'a str, url: &'static str, parser: P) -> Self {
        Self {
            client,
            interface,
            url,
            parser,
            _marker: PhantomData,
        }
    }
}

impl<'a, T, P> IpLookup<T> for HttpIpLookup<'a, P, T>
where
    T: Send + Sync + 'a,
    P: IpLookupParser<T> + Send + Sync,
{
    async fn lookup(&self) -> Result<T, Error> {
        let request = Request::get(self.url)
            .interface(NetworkInterface::name(self.interface))
            .body(())
            .unwrap();

        let mut response = self.client.send_async(request).await?;
        let body = response.text().await?;
        self.parser.parse(&body)
    }
}

pub type IpsbLookup<'a, T> = HttpIpLookup<'a, PlainTextIpParser, T>;

impl<'a> HttpIpLookup<'a, PlainTextIpParser, Ipv4Addr> {
    pub fn ipsb(client: &'a HttpClient, interface: &'a str) -> Self {
        Self::new(
            client,
            interface,
            "https://api-ipv4.ip.sb/ip",
            PlainTextIpParser,
        )
    }
}

impl<'a> HttpIpLookup<'a, PlainTextIpParser, Ipv6Addr> {
    pub fn ipsb(client: &'a HttpClient, interface: &'a str) -> Self {
        Self::new(
            client,
            interface,
            "https://api-ipv6.ip.sb/ip",
            PlainTextIpParser,
        )
    }
}
