use std::collections::BTreeSet;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use local_ip_address::list_afinet_netifas;
use rand_core::{OsRng, RngCore};

use crate::Error;

pub fn get_interfaces() -> Result<BTreeSet<String>, Error> {
    let netifas = list_afinet_netifas()?;
    let mut interfaces = BTreeSet::new();
    for (s, _) in netifas {
        interfaces.insert(s);
    }
    Ok(interfaces)
}

pub fn random_urlsafe_string(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
