use std::collections::BTreeSet;

use axum::http::{header, HeaderMap};
use local_ip_address::list_afinet_netifas;

use crate::Error;

pub fn get_header(headers: &HeaderMap, key: impl header::AsHeaderName) -> Option<String> {
    if let Some(header) = headers.get(key) {
        if let Ok(header) = header.to_str() {
            return Some(header.to_string());
        }
    }
    None
}

pub fn get_ua(headers: &HeaderMap) -> String {
    match get_header(headers, header::USER_AGENT) {
        Some(ua) => ua,
        None => "-".to_string(),
    }
}

pub fn get_interfaces() -> Result<BTreeSet<String>, Error> {
    let netifas = list_afinet_netifas()?;
    let mut interfaces = BTreeSet::new();
    for (s, _) in netifas {
        interfaces.insert(s);
    }
    Ok(interfaces)
}
