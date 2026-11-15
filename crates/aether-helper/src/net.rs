//! Network plumbing for TUN mode.
//!
//! Xray-core 26.x only *creates* the TUN interface (on macOS it also assigns
//! the fixed point-to-point address 169.254.10.2 → 169.254.10.1). Routing is
//! left to the embedding app, so the helper:
//!
//! 1. finds the physical interface carrying the default route, so Xray's own
//!    outbound connections can be bound to it (no routing loop);
//! 2. gives the interface an address where Xray does not (Linux, Windows);
//! 3. adds split routes that are more specific than the default route, and
//!    removes them again on shutdown.

use std::process::Command;
use std::time::Duration;

use aether_core::config::{TUN_ROUTES_V4, TUN_ROUTES_V6};
use anyhow::{bail, Context, Result};

/// Remote end of the TUN point-to-point link (Xray's fixed macOS address).
pub const TUN_PEER: &str = "169.254.10.1";
/// Local end; Xray assigns it itself on macOS.
#[cfg(not(target_os = "macos"))]
pub const TUN_LOCAL: &str = "169.254.10.2";

fn run(cmd: &str, args: &[&str]) -> Result<String> {
    let mut c = Command::new(cmd);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = c.output().with_context(|| format!("running {cmd}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = if err.trim().is_empty() { String::from_utf8_lossy(&out.stdout).into_owned() } else { err.into_owned() };
        bail!("{cmd} {}: {}", args.join(" "), msg.trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Tunnel-style interface names used by VPN clients.
pub fn is_vpn_interface(name: &str) -> bool {
    ["utun", "tun", "wg", "ppp", "ipsec", "tap", "aether"].iter().any(|p| name.starts_with(p))
}

/// Name of the interface that currently carries the default route.
pub fn physical_interface() -> Result<String> {
    #[cfg(target_os = "macos")]
    {
        let out = run("/sbin/route", &["-n", "get", "default"])?;
        out.lines()
            .find_map(|l| l.trim().strip_prefix("interface:").map(|i| i.trim().to_string()))
            .context("no default route — are you online?")
    }
    #[cfg(target_os = "linux")]
    {
        // "default via 192.168.1.1 dev wlan0 proto dhcp metric 600"
        let out = run("ip", &["-4", "route", "show", "default"])?;
        let mut words = out.split_whitespace();
        words.by_ref().find(|w| *w == "dev").context("no default route — are you online?")?;
        words.next().map(str::to_string).context("no default route device")
    }
    #[cfg(windows)]
    {
        let out = run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                "Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Sort-Object RouteMetric | \
                 Select-Object -First 1 -ExpandProperty InterfaceAlias",
            ],
        )?;
        let name = out.trim().to_string();
        if name.is_empty() {
            bail!("no default route — are you online?");
        }
        Ok(name)
    }
}

fn interface_exists(name: &str) -> bool {
    #[cfg(target_os = "macos")]
    return run("/sbin/ifconfig", &[name]).is_ok();
    #[cfg(target_os = "linux")]
    return run("ip", &["link", "show", name]).is_ok();
    #[cfg(windows)]
    return run("netsh", &["interface", "ipv4", "show", "interface"])
        .map(|o| o.contains(name))
        .unwrap_or(false);
}

/// Interface the OS would use for `addr` (diagnostics only).
pub fn route_interface_for(addr: &str) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let family = if addr.contains(':') { "-inet6" } else { "-inet" };
        let out = run("/sbin/route", &["-n", "get", family, addr]).ok()?;
        out.lines().find_map(|l| l.trim().strip_prefix("interface:").map(|i| i.trim().to_string()))
    }
    #[cfg(target_os = "linux")]
    {
        let out = run("ip", &["route", "get", addr]).ok()?;
        let mut words = out.split_whitespace();
        words.by_ref().find(|w| *w == "dev")?;
        words.next().map(str::to_string)
    }
    #[cfg(windows)]
    {
        let _ = addr;
        None
    }
}

pub async fn wait_for_interface(name: &str) -> Result<()> {
    for _ in 0..50 {
        let n = name.to_string();
        if tokio::task::spawn_blocking(move || interface_exists(&n)).await.unwrap_or(false) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    bail!("TUN interface {name} did not appear")
}

/// Routes installed by [`configure`], for [`teardown`].
#[derive(Debug, Default)]
pub struct Installed {
    v4: Vec<&'static str>,
    v6: Vec<&'static str>,
}

/// Assigns addresses (where Xray doesn't) and installs the split routes.
pub fn configure(name: &str, ipv6: bool) -> Result<Installed> {
    let mut installed = Installed::default();
    let result = (|| -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            let _ = run("ip", &["addr", "add", TUN_LOCAL, "peer", TUN_PEER, "dev", name]);
            run("ip", &["link", "set", name, "up"])?;
        }
        #[cfg(windows)]
        {
            run(
                "netsh",
                &["interface", "ipv4", "set", "address", &format!("name={name}"), "static", TUN_LOCAL, "255.255.255.252"],
            )?;
        }
        for cidr in TUN_ROUTES_V4 {
            add_v4(name, cidr).with_context(|| format!("adding route {cidr}"))?;
            installed.v4.push(cidr);
        }
        // IPv6 is always captured, even when disabled in settings: otherwise
        // it bypasses the tunnel (Xray then proxies or refuses it, see the
        // config builder). Machines without IPv6 simply skip this.
        let _ = ipv6;
        for cidr in TUN_ROUTES_V6 {
            match add_v6(name, cidr) {
                Ok(()) => installed.v6.push(cidr),
                Err(e) => eprintln!("IPv6 route {cidr} not installed: {e:#}"),
            }
        }
        eprintln!(
            "routes via {name}: {} IPv4, {} IPv6; probe → v4 {:?}, v6 {:?}",
            installed.v4.len(),
            installed.v6.len(),
            route_interface_for("1.1.1.1"),
            route_interface_for("2001:4860:4860::8888"),
        );
        Ok(())
    })();
    if let Err(e) = result {
        teardown(name, &installed);
        return Err(e);
    }
    Ok(installed)
}

fn add_v4(name: &str, cidr: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = name;
        run("/sbin/route", &["-q", "-n", "add", "-net", cidr, TUN_PEER]).map(drop)
    }
    #[cfg(target_os = "linux")]
    return run("ip", &["route", "replace", cidr, "dev", name]).map(drop);
    #[cfg(windows)]
    return run(
        "netsh",
        &["interface", "ipv4", "add", "route", cidr, name, TUN_PEER, "metric=1", "store=active"],
    )
    .map(drop);
}

fn add_v6(name: &str, cidr: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    return run("/sbin/route", &["-q", "-n", "add", "-inet6", "-net", cidr, "-interface", name]).map(drop);
    #[cfg(target_os = "linux")]
    return run("ip", &["-6", "route", "replace", cidr, "dev", name]).map(drop);
    #[cfg(windows)]
    return run("netsh", &["interface", "ipv6", "add", "route", cidr, name, "metric=1", "store=active"]).map(drop);
}

/// Removes our routes. They also vanish with the interface, so errors are ignored.
pub fn teardown(name: &str, installed: &Installed) {
    for cidr in &installed.v4 {
        #[cfg(target_os = "macos")]
        let _ = run("/sbin/route", &["-q", "-n", "delete", "-net", cidr, TUN_PEER]);
        #[cfg(target_os = "linux")]
        let _ = run("ip", &["route", "del", cidr, "dev", name]);
        #[cfg(windows)]
        let _ = run("netsh", &["interface", "ipv4", "delete", "route", cidr, name, TUN_PEER, "store=active"]);
    }
    for cidr in &installed.v6 {
        #[cfg(target_os = "macos")]
        let _ = run("/sbin/route", &["-q", "-n", "delete", "-inet6", "-net", cidr, "-interface", name]);
        #[cfg(target_os = "linux")]
        let _ = run("ip", &["-6", "route", "del", cidr, "dev", name]);
        #[cfg(windows)]
        let _ = run("netsh", &["interface", "ipv6", "delete", "route", cidr, name, "store=active"]);
    }
    let _ = name;
}
