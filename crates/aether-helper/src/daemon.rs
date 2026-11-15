//! Long-running privileged service (macOS launchd daemon), so TUN mode does
//! not ask for the administrator password on every connect.
//!
//! `install` (run once, elevated) copies this helper and Xray into a
//! root-owned directory and registers a launchd daemon. The daemon listens
//! on a socket in `/var/run` (only root can create files there, so it cannot
//! be spoofed), serves only the uid that installed it, and runs one session
//! per connection — the session ends when the app disconnects or quits.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use aether_core::helper_proto::{AppMsg, HelperMsg, DAEMON_DIR, DAEMON_LABEL, DAEMON_PLIST, DAEMON_SOCKET};
use anyhow::{bail, Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

use crate::dns;
use crate::session::{send, supervise, Params};

const STATE_DIR: &str = "/Library/Application Support/app.aether.vpn.helper";
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn uid_arg(mut args: impl Iterator<Item = String>) -> Result<u32> {
    match (args.next().as_deref(), args.next()) {
        (Some("--uid"), Some(uid)) => uid.parse().context("--uid must be a number"),
        _ => bail!("expected --uid <uid>"),
    }
}

fn require_root() -> Result<()> {
    // SAFETY: plain syscall.
    if unsafe { libc::geteuid() } != 0 {
        bail!("must run as root");
    }
    Ok(())
}

pub async fn serve(uid: u32) -> Result<()> {
    require_root()?;
    std::fs::create_dir_all(STATE_DIR)?;
    // A previous instance may have been killed mid-session.
    dns::restore_stale(Path::new(STATE_DIR));

    let _ = std::fs::remove_file(DAEMON_SOCKET);
    let listener = UnixListener::bind(DAEMON_SOCKET).context("binding daemon socket")?;
    // Anyone may connect; the peer uid is checked per connection.
    std::fs::set_permissions(DAEMON_SOCKET, std::fs::Permissions::from_mode(0o666))?;
    eprintln!("aether-helper daemon {VERSION} serving uid {uid}");

    loop {
        let stream = match listener.accept().await {
            Ok((s, _)) => s,
            Err(e) => {
                eprintln!("accept: {e}");
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            }
        };
        match stream.peer_cred().map(|c| c.uid()) {
            Ok(peer) if peer == uid || peer == 0 => {}
            Ok(peer) => {
                eprintln!("rejected connection from uid {peer}");
                continue;
            }
            // the peer already hung up (Aether's "is the service up?" probe)
            Err(_) => continue,
        }
        // One session at a time: a second app instance waits its turn.
        if let Err(e) = handle(stream).await {
            eprintln!("session: {e:#}");
        }
        dns::restore(Path::new(STATE_DIR));
    }
}

async fn handle(stream: UnixStream) -> Result<()> {
    let (rx, mut tx) = stream.into_split();
    send(&mut tx, &HelperMsg::Daemon { version: VERSION.into() }).await?;
    let mut lines = BufReader::new(rx).lines();
    let first = match tokio::time::timeout(Duration::from_secs(30), lines.next_line()).await {
        Ok(Ok(Some(line))) => line,
        _ => return Ok(()), // probe connection or idle client
    };
    let Ok(AppMsg::Start { config, assets, ipv6 }) = serde_json::from_str::<AppMsg>(&first) else {
        return Ok(());
    };
    let params = Params { config, assets: assets.into(), state_dir: STATE_DIR.into(), ipv6 };
    let result = supervise(&params, lines, &mut tx).await;
    if let Err(e) = &result {
        let _ = send(&mut tx, &HelperMsg::Exited { code: None, error: Some(format!("{e:#}")) }).await;
    }
    result
}

/// `std::fs::copy` may clone the source's owner, so ownership is set
/// explicitly: anything root executes must not be writable by the user.
fn make_root_owned(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    // SAFETY: valid NUL-terminated path; uid/gid 0 = root:wheel.
    if unsafe { libc::chown(c.as_ptr(), 0, 0) } != 0 {
        bail!("chown {}: {}", path.display(), std::io::Error::last_os_error());
    }
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

fn launchctl(args: &[&str]) -> Result<()> {
    let out = std::process::Command::new("/bin/launchctl").args(args).output()?;
    if !out.status.success() {
        bail!("launchctl {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

fn plist(uid: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{DAEMON_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{DAEMON_DIR}/aether-helper</string>
    <string>daemon</string>
    <string>--uid</string>
    <string>{uid}</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardErrorPath</key><string>/Library/Logs/aether-helper.log</string>
</dict>
</plist>
"#
    )
}

/// Copies helper + Xray into a root-owned directory and (re)starts the daemon.
pub fn install(uid: u32) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("the background service is only available on macOS");
    }
    require_root()?;
    let exe = std::env::current_exe()?.canonicalize()?;
    let src = exe.parent().context("helper has no parent directory")?;
    let dir = Path::new(DAEMON_DIR);
    std::fs::create_dir_all(dir)?;
    make_root_owned(dir, 0o755)?;
    for name in ["aether-helper", "xray"] {
        let from = src.join(name);
        let tmp = dir.join(format!("{name}.new"));
        // copy-then-rename: the running daemon keeps its old inode
        let _ = std::fs::remove_file(&tmp);
        std::fs::copy(&from, &tmp).with_context(|| format!("copying {}", from.display()))?;
        make_root_owned(&tmp, 0o755)?;
        std::fs::rename(&tmp, dir.join(name))?;
    }
    std::fs::write(DAEMON_PLIST, plist(uid))?;
    make_root_owned(Path::new(DAEMON_PLIST), 0o644)?;

    let _ = launchctl(&["bootout", &format!("system/{DAEMON_LABEL}")]);
    // bootout is asynchronous; retry bootstrap briefly
    let mut last = Ok(());
    for _ in 0..20 {
        last = launchctl(&["bootstrap", "system", DAEMON_PLIST]);
        if last.is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    last
}

pub fn uninstall() -> Result<()> {
    require_root()?;
    let _ = launchctl(&["bootout", &format!("system/{DAEMON_LABEL}")]);
    let _ = std::fs::remove_file(DAEMON_PLIST);
    let _ = std::fs::remove_file(DAEMON_SOCKET);
    let _ = std::fs::remove_dir_all(DAEMON_DIR);
    let _ = std::fs::remove_dir_all(STATE_DIR);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_runs_root_owned_copy() {
        let p = plist(501);
        assert!(p.contains("<string>/Library/PrivilegedHelperTools/app.aether.vpn/aether-helper</string>"));
        assert!(p.contains("<string>--uid</string>\n    <string>501</string>"));
    }

    #[test]
    fn uid_argument() {
        assert_eq!(uid_arg(["--uid".to_string(), "501".to_string()].into_iter()).unwrap(), 501);
        assert!(uid_arg(std::iter::empty()).is_err());
    }
}
