//! Redirects the OS resolver into the tunnel while TUN mode is active.
//!
//! Without this, queries to a LAN resolver (e.g. the router) bypass the TUN
//! because the LAN route is more specific than the tunnel's default route.
//! On Windows the resolver is set on the TUN adapter itself.

use std::path::Path;

use aether_core::config::TUN_DNS_IP;

#[cfg(target_os = "macos")]
mod imp {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::TUN_DNS_IP;

    fn backup_file(state: &Path) -> PathBuf {
        state.join("dns-backup.json")
    }

    fn networksetup(args: &[&str]) -> Option<String> {
        let out = Command::new("/usr/sbin/networksetup").args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn services() -> Vec<String> {
        networksetup(&["-listallnetworkservices"])
            .unwrap_or_default()
            .lines()
            .skip(1) // "An asterisk (*) denotes that a network service is disabled."
            .filter(|l| !l.starts_with('*') && !l.trim().is_empty())
            .map(str::to_string)
            .collect()
    }

    fn current_dns(service: &str) -> Vec<String> {
        networksetup(&["-getdnsservers", service])
            .unwrap_or_default()
            .lines()
            .map(str::trim)
            .filter(|l| l.parse::<std::net::IpAddr>().is_ok())
            .map(str::to_string)
            .collect()
    }

    fn set_dns(service: &str, servers: &[String]) {
        let mut args = vec!["-setdnsservers", service];
        if servers.is_empty() {
            args.push("Empty");
        } else {
            args.extend(servers.iter().map(String::as_str));
        }
        networksetup(&args);
    }

    fn flush() {
        let _ = Command::new("/usr/bin/dscacheutil").arg("-flushcache").status();
        let _ = Command::new("/usr/bin/killall").args(["-HUP", "mDNSResponder"]).status();
    }

    pub fn apply(state: &Path, _tun: Option<&str>) {
        let file = backup_file(state);
        if !file.exists() {
            let backup: BTreeMap<String, Vec<String>> =
                services().into_iter().map(|s| { let d = current_dns(&s); (s, d) }).collect();
            if let Ok(json) = serde_json::to_vec(&backup) {
                let _ = std::fs::write(&file, json);
            }
        }
        for service in services() {
            set_dns(&service, &[TUN_DNS_IP.to_string()]);
        }
        flush();
    }

    pub fn restore(state: &Path) {
        let file = backup_file(state);
        let Ok(data) = std::fs::read(&file) else { return };
        if let Ok(backup) = serde_json::from_slice::<BTreeMap<String, Vec<String>>>(&data) {
            for (service, servers) in backup {
                set_dns(&service, &servers);
            }
            flush();
        }
        let _ = std::fs::remove_file(file);
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use std::path::Path;
    use std::process::Command;

    use super::TUN_DNS_IP;

    /// systemd-resolved: make the TUN link the default DNS route. The
    /// settings disappear together with the interface, so there is nothing
    /// to restore.
    pub fn apply(_state: &Path, tun: Option<&str>) {
        let Some(tun) = tun else { return };
        let run = |args: &[&str]| Command::new("resolvectl").args(args).status().map(|s| s.success());
        if run(&["dns", tun, TUN_DNS_IP]).unwrap_or(false) {
            let _ = run(&["domain", tun, "~."]);
            let _ = run(&["default-route", tun, "true"]);
        }
    }

    pub fn restore(_state: &Path) {}
}

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::path::Path;
    use std::process::Command;

    use super::TUN_DNS_IP;

    /// The DNS server lives on the TUN adapter itself, which disappears on
    /// disconnect, so nothing needs restoring.
    pub fn apply(_state: &Path, tun: Option<&str>) {
        let Some(tun) = tun else { return };
        let _ = Command::new("netsh")
            .args(["interface", "ipv4", "set", "dnsservers", &format!("name={tun}"), "static", TUN_DNS_IP, "primary", "validate=no"])
            .creation_flags(0x0800_0000)
            .status();
    }

    pub fn restore(_state: &Path) {}
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod imp {
    use std::path::Path;

    pub fn apply(_state: &Path, _tun: Option<&str>) {
        let _ = super::TUN_DNS_IP;
    }

    pub fn restore(_state: &Path) {}
}

pub fn apply(state: &Path, tun: Option<&str>) {
    imp::apply(state, tun)
}

pub fn restore(state: &Path) {
    imp::restore(state)
}

/// Called at startup: a previous helper may have died before restoring.
pub fn restore_stale(state: &Path) {
    imp::restore(state)
}
