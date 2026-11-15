//! "Real delay" testing: instead of a TCP ping (useless for UDP protocols
//! like Hysteria2 and misleading for CDN-fronted servers), we start one
//! throw-away Xray instance with a SOCKS inbound per server and time an
//! HTTPS request through each of them.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use aether_core::config::outbound_json;
use aether_core::{Outbound, Server};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use tokio::sync::Semaphore;

use crate::core::process::terminate;
use crate::paths::write_atomic;

pub const TEST_URL: &str = "https://www.gstatic.com/generate_204";
const TIMEOUT: Duration = Duration::from_secs(6);
const CONCURRENCY: usize = 16;

fn free_port() -> Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// Outbound used for testing: the server itself, or the first outbound of a custom config.
fn test_outbound(server: &Server, tag: &str) -> Option<Value> {
    match &server.outbound {
        Outbound::Custom { config } => {
            let mut ob = config.get("outbounds")?.as_array()?.first()?.clone();
            ob["tag"] = tag.into();
            // dialer proxies would reference tags that don't exist here
            if let Some(s) = ob.get_mut("streamSettings").and_then(|s| s.get_mut("sockopt")) {
                if let Some(o) = s.as_object_mut() {
                    o.remove("dialerProxy");
                }
            }
            Some(ob)
        }
        o => outbound_json(o, tag).ok(),
    }
}

/// Measures one GET through a SOCKS proxy; returns milliseconds.
pub async fn measure(socks_port: u16) -> Result<u64> {
    let client = reqwest::Client::builder()
        .proxy(reqwest::Proxy::all(format!("socks5h://127.0.0.1:{socks_port}"))?)
        .timeout(TIMEOUT)
        .build()?;
    // The first request pays for the proxy handshake; report the second
    // like other clients do, but fall back to the first if only it succeeds.
    let start = Instant::now();
    client.get(TEST_URL).send().await?.error_for_status()?;
    let first = start.elapsed().as_millis() as u64;
    let start = Instant::now();
    match client.get(TEST_URL).send().await {
        Ok(r) if r.status().is_success() || r.status().as_u16() == 204 => Ok(start.elapsed().as_millis() as u64),
        _ => Ok(first),
    }
}

pub type Reporter = Arc<dyn Fn(&str, i64) + Send + Sync>;

/// Tests all `servers`, calling `report(id, ms_or_minus_one)` as results arrive.
pub async fn test_servers(xray: PathBuf, assets: PathBuf, run_dir: PathBuf, servers: Vec<Server>, report: Reporter) {
    run_batch(Arc::new((xray, assets, run_dir)), servers, report).await
}

/// If Xray refuses a batch (one malformed server poisons the whole config),
/// split it in halves until the culprit is isolated.
fn run_batch(
    dirs: Arc<(PathBuf, PathBuf, PathBuf)>,
    servers: Vec<Server>,
    report: Reporter,
) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        match start_batch(&dirs.0, &dirs.1, &dirs.2, &servers, report.clone()).await {
            Ok(()) => {}
            Err(e) if servers.len() > 1 => {
                tracing::debug!("latency batch of {} failed ({e:#}); splitting", servers.len());
                let mut left = servers;
                let right = left.split_off(left.len() / 2);
                run_batch(dirs.clone(), left, report.clone()).await;
                run_batch(dirs, right, report).await;
            }
            Err(_) => {
                for s in &servers {
                    report(&s.id, -1);
                }
            }
        }
    })
}

async fn start_batch(xray: &Path, assets: &Path, run_dir: &Path, servers: &[Server], report: Reporter) -> Result<()> {
    let mut inbounds = Vec::new();
    let mut outbounds = Vec::new();
    let mut rules = Vec::new();
    let mut jobs = Vec::new();
    for (i, server) in servers.iter().enumerate() {
        let Some(ob) = test_outbound(server, &format!("out-{i}")) else {
            report(&server.id, -1);
            continue;
        };
        let port = free_port()?;
        inbounds.push(json!({
            "tag": format!("in-{i}"),
            "listen": "127.0.0.1",
            "port": port,
            "protocol": "socks",
            "settings": { "udp": false },
        }));
        outbounds.push(ob);
        rules.push(json!({ "inboundTag": [format!("in-{i}")], "outboundTag": format!("out-{i}") }));
        jobs.push((server.id.clone(), port));
    }
    if jobs.is_empty() {
        return Ok(());
    }
    let config = json!({
        "log": { "loglevel": "none", "access": "none" },
        "inbounds": inbounds,
        "outbounds": outbounds,
        "routing": { "rules": rules },
    });
    let path = run_dir.join(format!("latency-{}.json", uuid::Uuid::new_v4().simple()));
    write_atomic(&path, &serde_json::to_vec(&config)?)?;

    let mut cmd = tokio::process::Command::new(xray);
    cmd.args(["run", "-c"])
        .arg(&path)
        .env("XRAY_LOCATION_ASSET", assets)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let mut child = cmd.spawn().context("starting xray for latency test")?;

    // Wait until every inbound accepts connections (or xray dies).
    let ports: Vec<u16> = jobs.iter().map(|j| j.1).collect();
    let mut ready = false;
    for _ in 0..50 {
        if let Ok(Some(_)) = child.try_wait() {
            break;
        }
        let mut all = true;
        for p in &ports {
            if tokio::net::TcpStream::connect(("127.0.0.1", *p)).await.is_err() {
                all = false;
                break;
            }
        }
        if all {
            ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !ready {
        terminate(&mut child).await;
        let _ = std::fs::remove_file(&path);
        anyhow::bail!("xray rejected the latency test config");
    }

    let sem = Arc::new(Semaphore::new(CONCURRENCY));
    let mut handles = Vec::new();
    for (id, port) in jobs {
        let sem = sem.clone();
        let report = report.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            let ms = measure(port).await.map(|v| v as i64).unwrap_or(-1);
            report(&id, ms);
        }));
    }
    for h in handles {
        let _ = h.await;
    }
    terminate(&mut child).await;
    let _ = std::fs::remove_file(&path);
    Ok(())
}
