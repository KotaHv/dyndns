use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use config::{Config as ConfigLoader, Environment};
use is_terminal::IsTerminal;
use once_cell::sync::Lazy;
use serde::Deserialize;

const PREFIX: &str = "DYNDNS";

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

#[derive(Deserialize)]
#[serde(default)]
pub struct Auth {
    pub username: String,
    pub password: String,
    pub token_ttl_seconds: u64,
    pub refresh_token_ttl_seconds: u64,
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            token_ttl_seconds: 3600,
            refresh_token_ttl_seconds: 86_400,
        }
    }
}

impl Auth {
    pub fn validate(&self) -> Result<(), String> {
        if self.username.trim().is_empty() {
            return Err("authentication username must be set".into());
        }
        if self.password.len() < 8 {
            return Err("authentication password must be at least 8 characters".into());
        }
        if self.token_ttl_seconds == 0 {
            return Err("authentication token ttl must be greater than zero".into());
        }
        if self.refresh_token_ttl_seconds == 0 {
            return Err("authentication refresh token ttl must be greater than zero".into());
        }
        if self.refresh_token_ttl_seconds <= self.token_ttl_seconds {
            return Err(
                "authentication refresh token ttl must be greater than access token ttl".into(),
            );
        }
        Ok(())
    }
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Auth")
            .field("username", &self.username)
            .field("password_set", &!self.password.is_empty())
            .field("token_ttl_seconds", &self.token_ttl_seconds)
            .field("refresh_token_ttl_seconds", &self.refresh_token_ttl_seconds)
            .finish()
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
    pub auth: Auth,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            log: Log::default(),
            addr: Self::addr(),
            database_url: Self::database_url(),
            web_dir: Self::web_dir(),
            debug: true,
            auth: Auth::default(),
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
    let config = ConfigLoader::builder()
        .add_source(
            Environment::with_prefix(PREFIX)
                .separator("_")
                .try_parsing(true),
        )
        .add_source(
            Environment::with_prefix(PREFIX)
                .separator("__")
                .prefix_separator("_")
                .try_parsing(true),
        )
        .build()
        .and_then(|cfg| cfg.try_deserialize::<Config>());

    match config {
        Ok(config) => {
            if let Err(err) = config.auth.validate() {
                panic!("{}", err);
            }
            println!("{:#?}", config);
            config
        }
        Err(err) => {
            panic!("{:?}", err);
        }
    }
}
