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

use super::{CLIENT, Error};

static DYNDNS_GOOD: &'static str = "good";

#[derive(Default)]
pub struct MyIp {
    pub v4: Option<Ipv4Addr>,
    pub v6: Option<Ipv6Addr>,
}

impl MyIp {
    fn serialize<S>(myip: &Self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(myip.to_string().as_str())
    }
}

impl Display for MyIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = vec![];
        if let Some(ip) = self.v4 {
            s.push(ip.to_string())
        }
        if let Some(ip) = self.v6 {
            s.push(ip.to_string())
        }
        let s = s.join(",");
        write!(f, "{}", s)
    }
}

#[derive(Serialize)]
pub struct Params {
    hostname: String,
    #[serde(serialize_with = "MyIp::serialize")]
    pub myip: MyIp,
}

impl Params {
    fn new(hostname: String) -> Self {
        Self {
            hostname,
            myip: MyIp::default(),
        }
    }
}

pub struct DynDNSAPI {
    server: String,
    username: String,
    password: String,
    pub params: Params,
}

impl DynDNSAPI {
    pub fn new(server: String, username: String, password: String, hostname: String) -> Self {
        Self {
            server,
            username,
            password,
            params: Params::new(hostname),
        }
    }
    pub async fn update(&self) -> Result<bool, Error> {
        let url = format!(
            "https://{server}/nic/update?hostname={hostname}&myip={myip}",
            server = self.server,
            hostname = self.params.hostname,
            myip = self.params.myip
        );
        let req = Request::get(url)
            .authentication(Authentication::basic())
            .credentials(Credentials::new(&self.username, &self.password))
            .body(())
            .unwrap();
        let mut res = CLIENT.send_async(req).await?;
        let status = res.status();
        let text = match res.text().await {
            Ok(text) => text.trim().to_string(),
            Err(err) => format!("{err:?}"),
        };
        if status.is_success() && text == DYNDNS_GOOD {
            debug!("{DYNDNS_GOOD}");
            Ok(true)
        } else {
            error!("code: {status}, msg: {text}");
            Ok(false)
        }
    }
}
