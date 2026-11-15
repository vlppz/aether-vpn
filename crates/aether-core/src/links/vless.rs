use super::{build_query, display_name, format_host, non_empty, pct_decode, pct_encode, split_link, stream};
use crate::error::{malformed, Result};
use crate::model::{Outbound, Server};

pub(super) fn parse(link: &str) -> Result<Server> {
    let p = split_link(link)?;
    let id = pct_decode(&p.userinfo);
    if id.is_empty() {
        return Err(malformed("vless link without user id"));
    }
    let outbound = Outbound::Vless {
        address: p.host.clone(),
        port: p.port,
        id,
        flow: non_empty(p.query.get("flow")),
        encryption: non_empty(p.query.get("encryption")).unwrap_or_else(|| "none".into()),
        stream: stream::parse(&p.query, "none")?,
    };
    Ok(Server::new(display_name(p.name, &p.host, p.port), outbound))
}

pub(super) fn export(server: &Server) -> Result<String> {
    let Outbound::Vless { address, port, id, flow, encryption, stream } = &server.outbound else {
        unreachable!()
    };
    let mut q = vec![("encryption", encryption.clone()), ("flow", flow.clone().unwrap_or_default())];
    q.extend(stream::export(stream));
    Ok(format!(
        "vless://{}@{}:{port}?{}#{}",
        pct_encode(id),
        format_host(address),
        build_query(&q),
        pct_encode(&server.name)
    ))
}
