use std::net::Ipv4Addr;

use serde::Serialize;

use super::{Error, CLIENT_V4, CLIENT_V6};

static DYNDNS_GOOD: &'static str = "good";

#[derive(Serialize)]
pub struct Params {
    hostname: String,
    my_ip: Option<String>,
}

impl Params {
    fn new(hostname: String) -> Self {
        Self {
            hostname,
            my_ip: None,
        }
    }
}

pub struct DynDNSAPI {
    server: String,
    username: String,
    password: String,
    params: Params,
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
    pub async fn update_v6(&mut self) -> Result<bool, Error> {
        let url = format!("https://{}/nic/update", &self.server);
        self.params.my_ip = None;
        let res = CLIENT_V6
            .get(url)
            .basic_auth(&self.username, Some(&self.password))
            .query(&self.params)
            .send()
            .await?;
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
    pub async fn update_v4(&mut self, v4: &Ipv4Addr) -> Result<bool, Error> {
        let url = format!("https://{}/nic/update", &self.server);
        self.params.my_ip = Some(v4.to_string());
        let res = CLIENT_V4
            .get(url)
            .basic_auth(&self.username, Some(&self.password))
            .query(&self.params)
            .send()
            .await?;
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
