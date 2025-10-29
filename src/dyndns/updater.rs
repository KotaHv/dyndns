use std::{
    fmt::Display,
    net::{Ipv4Addr, Ipv6Addr},
};

use isahc::{
    Request,
    auth::{Authentication, Credentials},
    config::Configurable,
    prelude::AsyncReadResponseExt,
};
use serde::{Serialize, Serializer};

use crate::Error;

use crate::db::DynDNS;

use super::{
    checker::{ipv4::Ipv4CheckResult, ipv6::Ipv6CheckResult},
    http_client::HttpClient,
};

const DYNDNS_GOOD: &str = "good";

pub struct DynDnsUpdater<'a> {
    client: &'a HttpClient,
    auth: DynDnsAuth<'a>,
    hostname: &'a str,
}

impl<'a> DynDnsUpdater<'a> {
    pub fn new(client: &'a HttpClient, auth: DynDnsAuth<'a>, hostname: &'a str) -> Self {
        Self {
            client,
            auth,
            hostname,
        }
    }

    pub async fn apply(
        &self,
        ipv4: &Ipv4CheckResult,
        ipv6: &Ipv6CheckResult,
    ) -> Result<bool, Error> {
        let ipv4_changed = ipv4.external.is_some();
        let ipv6_changed = ipv6.external.is_some();

        if !ipv4_changed && !ipv6_changed {
            return Ok(false);
        }

        let myip = MyIp::new(ipv4.external.as_ref(), ipv6.external.as_ref());
        let params = DynDnsParams::new(self.hostname, myip);

        let ip_summary = params.myip.to_string();
        let client = DynDnsApiClient::new(
            self.client,
            self.auth.server,
            self.auth.username,
            self.auth.password,
            params,
        );

        info!("ip address changed, start update: {}", ip_summary);
        if client.update().await? {
            info!("Successful update!");
            return Ok(true);
        }

        Ok(false)
    }
}

#[derive(Default)]
struct MyIp<'a> {
    v4: Option<&'a Ipv4Addr>,
    v6: Option<&'a Ipv6Addr>,
}

impl<'a> MyIp<'a> {
    fn new(v4: Option<&'a Ipv4Addr>, v6: Option<&'a Ipv6Addr>) -> Self {
        Self { v4, v6 }
    }

    fn serialize<S>(value: &Self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value.to_string().as_str())
    }
}

impl<'a> Display for MyIp<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = vec![];
        if let Some(ip) = self.v4 {
            parts.push(ip.to_string());
        }
        if let Some(ip) = self.v6 {
            parts.push(ip.to_string());
        }
        write!(f, "{}", parts.join(","))
    }
}

#[derive(Serialize)]
struct DynDnsParams<'a, 'b> {
    hostname: &'a str,
    #[serde(serialize_with = "MyIp::serialize")]
    myip: MyIp<'b>,
}

impl<'a, 'b> DynDnsParams<'a, 'b> {
    fn new(hostname: &'a str, myip: MyIp<'b>) -> Self {
        Self { hostname, myip }
    }
}

struct DynDnsApiClient<'a, 'b> {
    client: &'a HttpClient,
    server: &'a str,
    username: &'a str,
    password: &'a str,
    params: DynDnsParams<'a, 'b>,
}

pub struct DynDnsAuth<'a> {
    pub server: &'a str,
    pub username: &'a str,
    pub password: &'a str,
}

impl<'a> From<&'a DynDNS> for DynDnsAuth<'a> {
    fn from(value: &'a DynDNS) -> Self {
        Self {
            server: value.server.as_str(),
            username: value.username.as_str(),
            password: value.password.as_str(),
        }
    }
}

impl<'a, 'b> DynDnsApiClient<'a, 'b> {
    fn new(
        client: &'a HttpClient,
        server: &'a str,
        username: &'a str,
        password: &'a str,
        params: DynDnsParams<'a, 'b>,
    ) -> Self {
        Self {
            client,
            server,
            username,
            password,
            params,
        }
    }

    async fn update(&self) -> Result<bool, Error> {
        let url = format!(
            "https://{server}/nic/update?hostname={hostname}&myip={myip}",
            server = self.server,
            hostname = self.params.hostname,
            myip = self.params.myip
        );
        let request = Request::get(url)
            .authentication(Authentication::basic())
            .credentials(Credentials::new(self.username, self.password))
            .body(())
            .unwrap();
        let mut response = self.client.send_async(request).await?;
        let status = response.status();
        let message = response.text().await?;
        let message = message.trim().to_string();
        if status.is_success() && message == DYNDNS_GOOD {
            debug!("{}", DYNDNS_GOOD);
            Ok(true)
        } else {
            error!("code: {status}, msg: {message}");
            Ok(false)
        }
    }
}
