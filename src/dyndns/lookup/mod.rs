mod http;
mod local;
mod parser;

use crate::Error;

pub use http::{HttpIpLookup, IpsbLookup};
#[allow(unused_imports)]
pub use local::{LocalIpv4Lookup, LocalIpv6Lookup};
pub use parser::{IpLookupParser, PlainTextIpParser};

pub trait IpLookup<T>: Send + Sync {
    async fn lookup(&self) -> Result<T, Error>;
}
