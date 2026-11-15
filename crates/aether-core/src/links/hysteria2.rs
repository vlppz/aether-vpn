use super::{build_query, display_name, format_host, non_empty, pct_decode, pct_encode, split_link, stream};
use crate::error::Result;
use crate::model::{Outbound, Server};

/// `hysteria2://auth@host:port/?sni=..&obfs=salamander&obfs-password=..&insecure=1&mport=..#name`
pub(super) fn parse(link: &str) -> Result<Server> {
    let p = split_link(link)?;
    let q = &p.query;

    // Port-hopping may be expressed as `host:443,20000-30000` or `mport=`.
    let hostport_ports = link
        .split_once("://")
        .and_then(|(_, r)| r.split(['/', '?', '#']).next())
        .and_then(|hp| hp.rsplit_once(':'))
        .map(|(_, ports)| ports.to_string())
        .filter(|ports| ports.contains(',') || ports.contains('-'));
    let hop_ports = non_empty(q.get("mport")).or(hostport_ports);

    let obfs_password = (q.get("obfs").map(String::as_str) == Some("salamander"))
        .then(|| non_empty(q.get("obfs-password")))
        .flatten();

    let outbound = Outbound::Hysteria2 {
        address: p.host.clone(),
        port: p.port,
        auth: pct_decode(&p.userinfo),
        tls: stream::parse_tls(q),
        obfs_password,
        hop_ports,
        up: non_empty(q.get("up")),
        down: non_empty(q.get("down")),
    };
    Ok(Server::new(display_name(p.name, &p.host, p.port), outbound))
}

pub(super) fn export(server: &Server) -> Result<String> {
    let Outbound::Hysteria2 { address, port, auth, tls, obfs_password, hop_ports, up, down } = &server.outbound
    else {
        unreachable!()
    };
    let mut q = stream::export_tls(tls);
    // hysteria uses `insecure` rather than `allowInsecure`
    for (k, _) in q.iter_mut() {
        if *k == "allowInsecure" {
            *k = "insecure";
        }
    }
    if let Some(pw) = obfs_password {
        q.push(("obfs", "salamander".into()));
        q.push(("obfs-password", pw.clone()));
    }
    q.push(("mport", hop_ports.clone().unwrap_or_default()));
    q.push(("up", up.clone().unwrap_or_default()));
    q.push(("down", down.clone().unwrap_or_default()));
    Ok(format!(
        "hysteria2://{}@{}:{port}/?{}#{}",
        pct_encode(auth),
        format_host(address),
        build_query(&q),
        pct_encode(&server.name)
    ))
}
