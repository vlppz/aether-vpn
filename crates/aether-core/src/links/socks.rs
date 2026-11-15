use super::{decode_base64, display_name, encode_base64_url, format_host, pct_decode, pct_encode, split_link};
use crate::error::Result;
use crate::model::{Outbound, Server};

pub(super) fn parse(link: &str) -> Result<Server> {
    let p = split_link(link)?;
    let userinfo = pct_decode(&p.userinfo);
    // Either `user:pass` or base64(`user:pass`)
    let creds = if userinfo.contains(':') {
        Some(userinfo)
    } else if userinfo.is_empty() {
        None
    } else {
        decode_base64(&userinfo).or(Some(userinfo))
    };
    let (username, password) = match creds {
        Some(c) => match c.split_once(':') {
            Some((u, pw)) => (Some(u.to_string()), Some(pw.to_string())),
            None => (Some(c), None),
        },
        None => (None, None),
    };
    let outbound = Outbound::Socks { address: p.host.clone(), port: p.port, username, password };
    Ok(Server::new(display_name(p.name, &p.host, p.port), outbound))
}

pub(super) fn export(server: &Server) -> Result<String> {
    let Outbound::Socks { address, port, username, password } = &server.outbound else {
        unreachable!()
    };
    let auth = match (username, password) {
        (Some(u), pw) => format!("{}@", encode_base64_url(&format!("{u}:{}", pw.as_deref().unwrap_or("")))),
        _ => String::new(),
    };
    Ok(format!("socks://{auth}{}:{port}#{}", format_host(address), pct_encode(&server.name)))
}
