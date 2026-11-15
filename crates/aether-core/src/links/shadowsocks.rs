use super::{
    build_query, decode_base64, display_name, encode_base64_url, format_host, pct_decode, pct_encode,
    split_host_port, split_link, stream,
};
use crate::error::{malformed, Result};
use crate::model::{Outbound, Server, Stream};

/// Supports SIP002 (`ss://base64(method:pass)@host:port`, also with a
/// percent-encoded plain `method:pass` as used by 2022 ciphers) and the
/// legacy `ss://base64(method:pass@host:port)#name` form.
pub(super) fn parse(link: &str) -> Result<Server> {
    let body = link.split_once("://").map(|x| x.1).unwrap_or_default();
    let before_fragment = body.split('#').next().unwrap_or_default();
    let has_at = before_fragment.split('?').next().unwrap_or_default().contains('@');

    let (method, password, host, port, name, query) = if has_at {
        let p = split_link(link)?;
        let userinfo = pct_decode(&p.userinfo);
        let creds = if userinfo.contains(':') {
            userinfo
        } else {
            decode_base64(&userinfo).ok_or_else(|| malformed("invalid ss credentials"))?
        };
        let (m, pw) = creds.split_once(':').ok_or_else(|| malformed("ss credentials need method:password"))?;
        (m.to_string(), pw.to_string(), p.host, p.port, p.name, p.query)
    } else {
        let (encoded, name) = match body.split_once('#') {
            Some((e, n)) => (e, pct_decode(n)),
            None => (body, String::new()),
        };
        let encoded = encoded.split('?').next().unwrap_or_default();
        let decoded = decode_base64(encoded).ok_or_else(|| malformed("invalid legacy ss link"))?;
        let (creds, hostport) = decoded.rsplit_once('@').ok_or_else(|| malformed("invalid legacy ss link"))?;
        let (m, pw) = creds.split_once(':').ok_or_else(|| malformed("ss credentials need method:password"))?;
        let (host, port) = split_host_port(hostport)?;
        (m.to_string(), pw.to_string(), host, port, name, Default::default())
    };

    if query.get("plugin").is_some_and(|p| !p.is_empty()) {
        return Err(malformed("SIP003 plugins are not supported by Xray"));
    }
    let stream = if query.contains_key("type") || query.contains_key("security") {
        stream::parse(&query, "none")?
    } else {
        Stream::default()
    };
    let outbound = Outbound::Shadowsocks {
        address: host.clone(),
        port,
        method: method.to_ascii_lowercase(),
        password,
        stream,
    };
    Ok(Server::new(display_name(name, &host, port), outbound))
}

pub(super) fn export(server: &Server) -> Result<String> {
    let Outbound::Shadowsocks { address, port, method, password, stream } = &server.outbound else {
        unreachable!()
    };
    let userinfo = if method.starts_with("2022-") {
        format!("{}:{}", pct_encode(method), pct_encode(password))
    } else {
        encode_base64_url(&format!("{method}:{password}"))
    };
    let query = if *stream == Stream::default() {
        String::new()
    } else {
        format!("?{}", build_query(&stream::export(stream)))
    };
    Ok(format!(
        "ss://{userinfo}@{}:{port}{query}#{}",
        format_host(address),
        pct_encode(&server.name)
    ))
}
