use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use figment::{Figment, providers::Env};
use is_terminal::IsTerminal;
use once_cell::sync::Lazy;
use serde::Deserialize;

const PREFIX: &'static str = "DYNDNS_";

pub static CONFIG: Lazy<Config> = Lazy::new(|| init_config());

#[derive(Debug)]
pub enum LogStyle {
    Auto,
    Always,
    Never,
}

impl Default for LogStyle {
    fn default() -> Self {
        Self::Auto
    }
}

impl LogStyle {
    pub fn is_color(&self) -> bool {
        match self {
            LogStyle::Auto => std::io::stdout().is_terminal(),
            LogStyle::Always => true,
            LogStyle::Never => false,
        }
    }
}

impl<'de> Deserialize<'de> for LogStyle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?.to_lowercase();
        match s.as_str() {
            "auto" => Ok(LogStyle::Auto),
            "always" => Ok(LogStyle::Always),
            "never" => Ok(LogStyle::Never),
            _ => Err(serde::de::Error::unknown_field(
                &s,
                &["auto", "always", "never"],
            )),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Log {
    pub level: String,
    pub style: LogStyle,
}

impl Default for Log {
    fn default() -> Self {
        Log {
            level: Self::level(),
            style: LogStyle::default(),
        }
    }
}

impl Log {
    fn level() -> String {
        String::from("dyndns=info")
    }
}

#[derive(Deserialize, Debug)]
#[serde(default)]
pub struct Config {
    pub log: Log,
    pub addr: SocketAddr,
    pub database_url: String,
    pub web_dir: String,
    pub debug: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            log: Log::default(),
            addr: Self::addr(),
            database_url: Self::database_url(),
            web_dir: Self::web_dir(),
            debug: true,
        }
    }
}

impl Config {
    fn addr() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3030)
    }
    fn database_url() -> String {
        String::from("data/dyndns.db")
    }

    fn web_dir() -> String {
        String::from("dist")
    }
}

pub fn init_config() -> Config {
    let config = Figment::from(Env::prefixed(PREFIX))
        .merge(Env::prefixed(PREFIX).split("_"))
        .extract::<Config>();
    match config {
        Ok(config) => {
            println!("{:#?}", config);
            config
        }
        Err(err) => {
            panic!("{:?}", err);
        }
    }
}
