use std::collections::BTreeSet;

use axum::{
    routing::{get, Router},
    Json,
};
use local_ip_address::list_afinet_netifas;

use crate::Error;

pub fn routes() -> Router {
    Router::new().route("/", get(get_interfaces))
}

async fn get_interfaces() -> Result<Json<BTreeSet<String>>, Error> {
    let netifas = list_afinet_netifas()?;
    let mut interfaces = BTreeSet::new();
    for (s, _) in netifas {
        interfaces.insert(s);
    }
    Ok(Json(interfaces))
}
