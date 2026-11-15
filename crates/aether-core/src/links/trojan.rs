use super::{build_query, display_name, format_host, pct_decode, pct_encode, split_link, stream};
use crate::error::{malformed, Result};
use crate::model::{Outbound, Server};

pub(super) fn parse(link: &str) -> Result<Server> {
    let p = split_link(link)?;
    let password = pct_decode(&p.userinfo);
    if password.is_empty() {
        return Err(malformed("trojan link without password"));
    }
    let outbound = Outbound::Trojan {
        address: p.host.clone(),
        port: p.port,
        password,
        // trojan is TLS by definition unless the link says otherwise
        stream: stream::parse(&p.query, "tls")?,
    };
    Ok(Server::new(display_name(p.name, &p.host, p.port), outbound))
}

pub(super) fn export(server: &Server) -> Result<String> {
    let Outbound::Trojan { address, port, password, stream } = &server.outbound else {
        unreachable!()
    };
    Ok(format!(
        "trojan://{}@{}:{port}?{}#{}",
        pct_encode(password),
        format_host(address),
        build_query(&stream::export(stream)),
        pct_encode(&server.name)
    ))
}
