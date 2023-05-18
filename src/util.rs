use std::collections::BTreeSet;

use local_ip_address::list_afinet_netifas;

use crate::Error;

pub fn get_interfaces() -> Result<BTreeSet<String>, Error> {
    let netifas = list_afinet_netifas()?;
    let mut interfaces = BTreeSet::new();
    for (s, _) in netifas {
        interfaces.insert(s);
    }
    Ok(interfaces)
}
