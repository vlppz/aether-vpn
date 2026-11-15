//! OS-level proxy configuration.
//!
//! Xray's SOCKS inbound also speaks HTTP, so every proxy kind (HTTP, HTTPS,
//! SOCKS) points at the same port. Before changing anything we capture the
//! previous configuration so disconnecting restores exactly what the user
//! had (e.g. a corporate proxy), not just "off".

use anyhow::Result;
use serde_json::Value;

pub const BYPASS: &[&str] = &[
    "localhost",
    "127.0.0.0/8",
    "::1",
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "169.254.0.0/16",
    "*.local",
    "<local>",
];

/// Whether the OS proxy currently points at `127.0.0.1:port`.
pub fn points_to(port: u16) -> bool {
    imp::points_to(port).unwrap_or(false)
}

/// Points the OS proxy at `127.0.0.1:port`, returning the previous settings.
pub fn enable(port: u16) -> Result<Value> {
    let backup = imp::backup()?;
    imp::enable(port)?;
    Ok(backup)
}

/// Restores settings captured by [`enable`]; falls back to switching off.
pub fn restore(backup: Option<&Value>) -> Result<()> {
    match backup {
        Some(b) => imp::restore(b).or_else(|_| imp::disable()),
        None => imp::disable(),
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::process::Command;

    use anyhow::{bail, Result};
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    use super::BYPASS;

    fn networksetup(args: &[&str]) -> Result<String> {
        let out = Command::new("/usr/sbin/networksetup").args(args).output()?;
        if !out.status.success() {
            bail!(
                "networksetup {} failed: {}",
                args.first().unwrap_or(&""),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// All enabled network services (Wi-Fi, Ethernet, Thunderbolt Bridge…).
    fn services() -> Result<Vec<String>> {
        Ok(networksetup(&["-listallnetworkservices"])?
            .lines()
            .skip(1)
            .filter(|l| !l.starts_with('*') && !l.trim().is_empty())
            .map(str::to_string)
            .collect())
    }

    pub fn enable(port: u16) -> Result<()> {
        let port = port.to_string();
        let mut last_err = None;
        let mut ok = 0;
        for s in services()? {
            let r = (|| -> Result<()> {
                networksetup(&["-setwebproxy", &s, "127.0.0.1", &port])?;
                networksetup(&["-setsecurewebproxy", &s, "127.0.0.1", &port])?;
                networksetup(&["-setsocksfirewallproxy", &s, "127.0.0.1", &port])?;
                let mut args = vec!["-setproxybypassdomains", s.as_str()];
                args.extend(BYPASS.iter().filter(|b| **b != "<local>"));
                networksetup(&args)?;
                Ok(())
            })();
            match r {
                Ok(()) => ok += 1,
                Err(e) => last_err = Some(e),
            }
        }
        match (ok, last_err) {
            (0, Some(e)) => Err(e),
            _ => Ok(()),
        }
    }

    pub fn disable() -> Result<()> {
        for s in services()? {
            let _ = networksetup(&["-setwebproxystate", &s, "off"]);
            let _ = networksetup(&["-setsecurewebproxystate", &s, "off"]);
            let _ = networksetup(&["-setsocksfirewallproxystate", &s, "off"]);
        }
        Ok(())
    }

    #[derive(Serialize, Deserialize)]
    struct Proxy {
        enabled: bool,
        server: String,
        port: u16,
    }

    #[derive(Serialize, Deserialize)]
    struct ServiceBackup {
        service: String,
        web: Proxy,
        secure: Proxy,
        socks: Proxy,
        bypass: Vec<String>,
    }

    fn read_proxy(kind: &str, service: &str) -> Result<Proxy> {
        let out = networksetup(&[kind, service])?;
        let field = |name: &str| {
            out.lines()
                .find_map(|l| l.strip_prefix(name))
                .map(|v| v.trim().to_string())
                .unwrap_or_default()
        };
        Ok(Proxy {
            enabled: field("Enabled:") == "Yes",
            server: field("Server:"),
            port: field("Port:").parse().unwrap_or(0),
        })
    }

    fn write_proxy(set: &str, state: &str, service: &str, p: &Proxy) -> Result<()> {
        if !p.server.is_empty() && p.port != 0 {
            networksetup(&[set, service, &p.server, &p.port.to_string()])?;
        }
        networksetup(&[state, service, if p.enabled { "on" } else { "off" }])?;
        Ok(())
    }

    pub fn backup() -> Result<Value> {
        let mut all = Vec::new();
        for s in services()? {
            let Ok(web) = read_proxy("-getwebproxy", &s) else { continue };
            let bypass = networksetup(&["-getproxybypassdomains", &s])
                .unwrap_or_default()
                .lines()
                .filter(|l| !l.contains(' '))
                .map(str::to_string)
                .collect();
            all.push(ServiceBackup {
                web,
                secure: read_proxy("-getsecurewebproxy", &s)?,
                socks: read_proxy("-getsocksfirewallproxy", &s)?,
                bypass,
                service: s,
            });
        }
        Ok(serde_json::to_value(all)?)
    }

    pub fn restore(backup: &Value) -> Result<()> {
        let all: Vec<ServiceBackup> = serde_json::from_value(backup.clone())?;
        for b in all {
            let s = b.service.as_str();
            let _ = write_proxy("-setwebproxy", "-setwebproxystate", s, &b.web);
            let _ = write_proxy("-setsecurewebproxy", "-setsecurewebproxystate", s, &b.secure);
            let _ = write_proxy("-setsocksfirewallproxy", "-setsocksfirewallproxystate", s, &b.socks);
            let mut args = vec!["-setproxybypassdomains", s];
            if b.bypass.is_empty() {
                args.push("Empty");
            } else {
                args.extend(b.bypass.iter().map(String::as_str));
            }
            let _ = networksetup(&args);
        }
        Ok(())
    }

    pub fn points_to(port: u16) -> Result<bool> {
        let port = port.to_string();
        for s in services()? {
            let out = networksetup(&["-getwebproxy", &s])?;
            let enabled = out.lines().any(|l| l.trim() == "Enabled: Yes");
            let server = out.lines().any(|l| l.trim() == "Server: 127.0.0.1");
            let p = out.lines().any(|l| l.trim() == format!("Port: {port}"));
            if enabled && server && p {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(any(windows, target_os = "linux"))]
mod imp {
    use anyhow::Result;
    use serde_json::{json, Value};
    use sysproxy::Sysproxy;

    use super::BYPASS;

    fn bypass() -> String {
        if cfg!(windows) {
            // WinINet syntax: wildcards, ';'-separated
            "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.2*;172.30.*;172.31.*;192.168.*;169.254.*;<local>"
                .into()
        } else {
            BYPASS.iter().filter(|b| **b != "<local>").cloned().collect::<Vec<_>>().join(",")
        }
    }

    pub fn enable(port: u16) -> Result<()> {
        Sysproxy { enable: true, host: "127.0.0.1".into(), port, bypass: bypass() }.set_system_proxy()?;
        Ok(())
    }

    pub fn disable() -> Result<()> {
        let mut current = Sysproxy::get_system_proxy().unwrap_or_default();
        current.enable = false;
        current.set_system_proxy()?;
        Ok(())
    }

    pub fn backup() -> Result<Value> {
        let p = Sysproxy::get_system_proxy().unwrap_or_default();
        Ok(json!({ "enable": p.enable, "host": p.host, "port": p.port, "bypass": p.bypass }))
    }

    pub fn restore(b: &Value) -> Result<()> {
        Sysproxy {
            enable: b["enable"].as_bool().unwrap_or(false),
            host: b["host"].as_str().unwrap_or_default().to_string(),
            port: b["port"].as_u64().unwrap_or(0) as u16,
            bypass: b["bypass"].as_str().unwrap_or_default().to_string(),
        }
        .set_system_proxy()?;
        Ok(())
    }

    pub fn points_to(port: u16) -> Result<bool> {
        let p = Sysproxy::get_system_proxy()?;
        Ok(p.enable && p.port == port && (p.host == "127.0.0.1" || p.host == "localhost"))
    }
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
mod imp {
    use anyhow::{bail, Result};

    pub fn enable(_port: u16) -> Result<()> {
        bail!("system proxy is not supported on this platform")
    }
    pub fn disable() -> Result<()> {
        Ok(())
    }
    pub fn points_to(_port: u16) -> Result<bool> {
        Ok(false)
    }
    pub fn backup() -> Result<serde_json::Value> {
        Ok(serde_json::Value::Null)
    }
    pub fn restore(_b: &serde_json::Value) -> Result<()> {
        Ok(())
    }
}
