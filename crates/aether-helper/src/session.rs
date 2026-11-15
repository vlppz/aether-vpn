//! One tunnel session: prepare the config, run Xray, set up routes and DNS,
//! and tear everything down when the app says stop or goes away. Used by
//! both the one-shot helper and the macOS daemon.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use aether_core::helper_proto::{AppMsg, HelperMsg};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::{dns, net};

pub struct Params {
    pub config: Value,
    pub assets: PathBuf,
    pub state_dir: PathBuf,
    pub ipv6: bool,
}

fn xray_path() -> Result<PathBuf> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let dir = exe.parent().context("helper has no parent directory")?;
    let name = if cfg!(windows) { "xray.exe" } else { "xray" };
    let path = dir.join(name);
    if !path.is_file() {
        bail!("xray binary not found next to helper at {}", path.display());
    }
    Ok(path)
}

/// Copies the config into a private file, removing anything that would make
/// a root process write to user-chosen paths, and binding every outbound to
/// the physical interface so Xray's own traffic never re-enters the tunnel.
fn sanitize_config(mut config: Value, physical: &str) -> Result<PathBuf> {
    if let Some(log) = config.get_mut("log").and_then(Value::as_object_mut) {
        log.remove("access");
        log.remove("error");
    }
    bind_outbounds(&mut config, physical);
    pin_server_addresses(&mut config);
    let dst = std::env::temp_dir().join(format!("aether-tun-{}.json", std::process::id()));
    write_private(&dst, &serde_json::to_vec(&config)?)?;
    Ok(dst)
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    let _ = std::fs::remove_file(path);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    use std::io::Write;
    opts.open(path)?.write_all(data)?;
    Ok(())
}

pub async fn send<W: AsyncWrite + Unpin>(tx: &mut W, msg: &HelperMsg) -> Result<()> {
    let mut line = serde_json::to_vec(msg)?;
    line.push(b'\n');
    tx.write_all(&line).await?;
    Ok(())
}

fn forward_lines<R: AsyncRead + Unpin + Send + 'static>(reader: R, out: mpsc::Sender<String>) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if out.send(line).await.is_err() {
                break;
            }
        }
    });
}

/// Asks xray to exit cleanly so it can remove its routes, then force-kills.
async fn stop_child(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        // SAFETY: plain syscall on a pid we own.
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await.is_ok() {
            return;
        }
    }
    let _ = child.kill().await;
}

/// Sets `streamSettings.sockopt.interface` on outbounds that don't choose
/// their own interface or chain through another outbound.
fn bind_outbounds(config: &mut Value, physical: &str) {
    let Some(outbounds) = config.get_mut("outbounds").and_then(Value::as_array_mut) else { return };
    for ob in outbounds {
        if matches!(ob["protocol"].as_str(), Some("blackhole" | "dns" | "loopback")) {
            continue;
        }
        let Some(obj) = ob.as_object_mut() else { continue };
        let stream = obj.entry("streamSettings").or_insert_with(|| serde_json::json!({}));
        let Some(stream) = stream.as_object_mut() else { continue };
        let sockopt = stream.entry("sockopt").or_insert_with(|| serde_json::json!({}));
        if let Some(sockopt) = sockopt.as_object_mut() {
            if !sockopt.contains_key("interface") && !sockopt.contains_key("dialerProxy") {
                sockopt.insert("interface".into(), physical.into());
            }
        }
    }
}

/// Resolves every outbound's server hostname *now*, while DNS still works
/// normally, and hands the answers to Xray via `dns.hosts`. Otherwise Xray
/// would look its own server up through the system resolver, whose queries
/// go into the tunnel, which needs the server: a deadlock.
fn pin_server_addresses(config: &mut Value) {
    use std::net::ToSocketAddrs;

    let mut hosts = serde_json::Map::new();
    if let Some(outbounds) = config.get_mut("outbounds").and_then(Value::as_array_mut) {
        for ob in outbounds {
            if matches!(ob["protocol"].as_str(), Some("freedom" | "blackhole" | "dns" | "loopback")) {
                continue;
            }
            let settings = &ob["settings"];
            let mut servers: Vec<&Value> = Vec::new();
            for key in ["vnext", "servers"] {
                if let Some(list) = settings[key].as_array() {
                    servers.extend(list);
                }
            }
            servers.push(settings); // flat form: settings.address
            let mut pinned = false;
            for server in servers {
                let Some(host) = server["address"].as_str() else { continue };
                if host.parse::<std::net::IpAddr>().is_ok() || hosts.contains_key(host) {
                    continue;
                }
                let ips: Vec<Value> = (host, 0)
                    .to_socket_addrs()
                    .map(|a| a.filter(|a| a.is_ipv4()).map(|a| a.ip().to_string().into()).collect())
                    .unwrap_or_default();
                if !ips.is_empty() {
                    hosts.insert(host.to_string(), Value::Array(ips));
                    pinned = true;
                }
            }
            if pinned {
                let sockopt = &mut ob["streamSettings"]["sockopt"];
                if sockopt.get("domainStrategy").is_none() {
                    sockopt["domainStrategy"] = "UseIPv4".into();
                }
            }
        }
    }
    if hosts.is_empty() {
        return;
    }
    if !config["dns"].is_object() {
        config["dns"] = serde_json::json!({ "servers": ["1.1.1.1"] });
    }
    let existing = config["dns"]["hosts"].as_object().cloned().unwrap_or_default();
    for (k, v) in existing {
        hosts.insert(k, v); // user-provided hosts win
    }
    config["dns"]["hosts"] = Value::Object(hosts);
}

/// Runs a session until stopped. `commands` is the rest of the app's message
/// stream (after the handshake); its EOF means the app is gone.
pub async fn supervise<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    args: &Params,
    mut commands: Lines<BufReader<R>>,
    tx: &mut W,
) -> Result<()> {
    let physical = net::physical_interface()?;
    if net::is_vpn_interface(&physical) {
        bail!("another VPN ({physical}) carries the default route — disconnect it first");
    }
    let config = sanitize_config(args.config.clone(), &physical)?;
    let tun_name = tun_interface_name(&config).context("config has no tun inbound")?;

    let mut child = Command::new(xray_path()?)
        .args(["run", "-c"])
        .arg(&config)
        .env("XRAY_LOCATION_ASSET", &args.assets)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("starting xray")?;

    let (log_tx, mut log_rx) = mpsc::channel::<String>(256);
    forward_lines(child.stdout.take().unwrap(), log_tx.clone());
    forward_lines(child.stderr.take().unwrap(), log_tx.clone());
    let _ = log_tx.send(format!("[Info] outbounds bound to {physical}")).await;

    // Xray creates the interface; routing and DNS are ours to set up.
    let routes = async {
        net::wait_for_interface(&tun_name).await?;
        let (name, ipv6) = (tun_name.clone(), args.ipv6);
        tokio::task::spawn_blocking(move || net::configure(&name, ipv6)).await?
    };
    let routes = tokio::select! {
        r = routes => r,
        status = child.wait() => Err(anyhow::anyhow!("xray exited during setup ({:?})", status.ok().and_then(|s| s.code()))),
    };
    let routes = match routes {
        Ok(r) => r,
        Err(e) => {
            stop_child(&mut child).await;
            // forward whatever xray printed before failing
            while let Ok(line) = log_rx.try_recv() {
                let _ = send(tx, &HelperMsg::Log { line }).await;
            }
            let _ = std::fs::remove_file(&config);
            return Err(e);
        }
    };
    let _ = log_tx.send(format!("[Info] routes installed via {tun_name}")).await;
    dns::apply(&args.state_dir, Some(&tun_name));
    send(tx, &HelperMsg::Started { pid: child.id().unwrap_or_default() }).await?;

    let exit = loop {
        tokio::select! {
            line = log_rx.recv() => {
                if let Some(line) = line {
                    // the GUI may have gone away; the next read will notice
                    let _ = send(tx, &HelperMsg::Log { line }).await;
                }
            }
            cmd = commands.next_line() => {
                match cmd {
                    Ok(Some(line)) => {
                        if let Ok(AppMsg::Stop) = serde_json::from_str::<AppMsg>(&line) {
                            stop_child(&mut child).await;
                            break child.wait().await.ok();
                        }
                    }
                    // EOF or error: the GUI is gone — tear down.
                    _ => {
                        stop_child(&mut child).await;
                        break child.wait().await.ok();
                    }
                }
            }
            status = child.wait() => break status.ok(),
        }
    };
    net::teardown(&tun_name, &routes);
    let _ = std::fs::remove_file(&config);

    // Drain remaining log lines so the GUI sees why xray exited.
    while let Ok(line) = log_rx.try_recv() {
        let _ = send(tx, &HelperMsg::Log { line }).await;
    }
    let _ = send(tx, &HelperMsg::Exited { code: exit.and_then(|s| s.code()), error: None }).await;
    Ok(())
}

fn tun_interface_name(config: &Path) -> Option<String> {
    let v: Value = serde_json::from_slice(&std::fs::read(config).ok()?).ok()?;
    v["inbounds"]
        .as_array()?
        .iter()
        .find(|i| i["protocol"] == "tun")
        .and_then(|i| i["settings"]["name"].as_str())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn binds_and_pins_outbounds() {
        let mut c = json!({
            "outbounds": [
                { "tag": "proxy", "protocol": "vless",
                  "settings": { "vnext": [{ "address": "localhost", "port": 443, "users": [] }] } },
                { "tag": "ip", "protocol": "trojan", "settings": { "servers": [{ "address": "1.2.3.4", "port": 1 }] } },
                { "tag": "direct", "protocol": "freedom" },
                { "tag": "block", "protocol": "blackhole" },
                { "tag": "chained", "protocol": "vless", "streamSettings": { "sockopt": { "dialerProxy": "proxy" } } }
            ],
            "dns": { "hosts": { "localhost": "9.9.9.9" } }
        });
        bind_outbounds(&mut c, "en0");
        pin_server_addresses(&mut c);
        assert_eq!(c["outbounds"][0]["streamSettings"]["sockopt"]["interface"], "en0");
        assert_eq!(c["outbounds"][0]["streamSettings"]["sockopt"]["domainStrategy"], "UseIPv4");
        assert_eq!(c["outbounds"][2]["streamSettings"]["sockopt"]["interface"], "en0");
        assert!(c["outbounds"][3].get("streamSettings").is_none());
        assert!(c["outbounds"][4]["streamSettings"]["sockopt"].get("interface").is_none());
        // literal IPs need no pinning; user hosts are kept
        assert!(c["outbounds"][1]["streamSettings"]["sockopt"].get("domainStrategy").is_none());
        assert_eq!(c["dns"]["hosts"]["localhost"], "9.9.9.9");
    }
}
