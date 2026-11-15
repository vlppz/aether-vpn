//! TUN mode: Xray runs inside the privileged `aether-helper`.
//!
//! * macOS: the helper runs as a launchd daemon, installed once with a single
//!   password prompt (and reinstalled automatically when the bundled helper or
//!   Xray changes). Connecting needs no password.
//! * Linux/Windows: the helper is launched per connection through the native
//!   elevation prompt and connects back to a local socket that *we* own
//!   (see `aether_core::helper_proto`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use aether_core::helper_proto::{socket_name, AppMsg, HelperMsg};
use anyhow::{anyhow, bail, Context, Result};
use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::ListenerOptions;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, Lines};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};

use super::process::OnExit;
use crate::logs::Logs;
use crate::paths::{write_atomic, Paths};

/// How long we wait for the user to answer the password/UAC prompt.
const AUTH_TIMEOUT: Duration = Duration::from_secs(180);

/// Interface the OS currently uses to reach the internet (probing 1.1.1.1).
pub fn route_interface() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let out = std::process::Command::new("/sbin/route").args(["-n", "get", "1.1.1.1"]).output().ok()?;
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find_map(|l| l.trim().strip_prefix("interface:").map(|i| i.trim().to_string()))
    }
    #[cfg(target_os = "linux")]
    {
        // "1.1.1.1 via 192.168.1.1 dev wlan0 src …"
        let out = std::process::Command::new("ip").args(["route", "get", "1.1.1.1"]).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let mut words = text.split_whitespace();
        words.by_ref().find(|w| *w == "dev")?;
        words.next().map(str::to_string)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        None
    }
}

/// Tunnel-style interface names used by VPN clients.
pub fn is_vpn_interface(name: &str) -> bool {
    ["utun", "tun", "wg", "ppp", "ipsec", "tap"].iter().any(|p| name.starts_with(p))
}

/// Fails early if another full-tunnel VPN owns the default route: its more
/// specific routes would keep winning over ours and Aether's tunnel would
/// silently carry nothing.
pub fn check_no_other_vpn(own_name: &str) -> Result<()> {
    match route_interface() {
        Some(iface) if iface != own_name && is_vpn_interface(&iface) => bail!(
            "another VPN is active ({iface} carries all traffic). Disconnect it — e.g. Happ — \
             before using TUN mode, or use Proxy mode alongside it"
        ),
        _ => Ok(()),
    }
}

pub struct TunSession {
    stop_tx: mpsc::Sender<()>,
    done: watch::Receiver<bool>,
    files: Vec<PathBuf>,
}

impl TunSession {
    pub async fn start(paths: &Paths, config: &Path, ipv6: bool, logs: Arc<Logs>, on_exit: OnExit) -> Result<Self> {
        // AETHER_TUN_NO_ELEVATE keeps the one-shot path testable on macOS.
        if cfg!(target_os = "macos") && std::env::var_os("AETHER_TUN_NO_ELEVATE").is_none() {
            #[cfg(target_os = "macos")]
            return daemon::start(paths, config, ipv6, logs, on_exit).await;
        }
        Self::start_one_shot(paths, config, ipv6, logs, on_exit).await
    }

    async fn start_one_shot(paths: &Paths, config: &Path, ipv6: bool, logs: Arc<Logs>, on_exit: OnExit) -> Result<Self> {
        let helper = paths.sidecar("aether-helper");
        if !helper.is_file() {
            bail!("aether-helper not found at {}", helper.display());
        }
        let session = uuid::Uuid::new_v4().simple().to_string();
        let token = uuid::Uuid::new_v4().simple().to_string();
        let token_file = paths.run.join(format!("helper-{session}.token"));
        write_atomic(&token_file, token.as_bytes())?;

        let ipc_id = if cfg!(windows) {
            format!("aether-helper-{session}")
        } else {
            // keep it short: unix socket paths are limited to ~104 bytes
            let p = paths.run.join(format!("h-{}.sock", &session[..8]));
            let _ = std::fs::remove_file(&p);
            p.to_string_lossy().into_owned()
        };
        let mut files = vec![token_file.clone()];
        if !cfg!(windows) {
            files.push(PathBuf::from(&ipc_id));
        }
        let listener = ListenerOptions::new()
            .name(socket_name(&ipc_id)?)
            .create_tokio()
            .context("creating helper socket")?;

        let args: Vec<String> = vec![
            "--ipc".into(),
            ipc_id.clone(),
            "--token-file".into(),
            token_file.to_string_lossy().into(),
            "--config".into(),
            config.to_string_lossy().into(),
            "--assets".into(),
            paths.assets.to_string_lossy().into(),
            "--state-dir".into(),
            paths.run.to_string_lossy().into(),
            "--ipv6".into(),
            if ipv6 { "1" } else { "0" }.into(),
        ];
        logs.app("Requesting administrator rights for TUN mode…");
        let mut launcher = elevate(&helper, &args)?;

        // Wait for the helper to connect while watching the launcher: if the
        // user cancels the prompt, the launcher exits with an error first.
        let conn = {
            let accept = listener.accept();
            tokio::pin!(accept);
            let deadline = tokio::time::sleep(AUTH_TIMEOUT);
            tokio::pin!(deadline);
            let mut launcher_done = false;
            loop {
                tokio::select! {
                    conn = &mut accept => break conn.context("accepting helper connection")?,
                    status = launcher.wait(), if !launcher_done => {
                        launcher_done = true;
                        let status = status?;
                        if !status.success() {
                            let mut stderr = String::new();
                            if let Some(mut e) = launcher.stderr.take() {
                                use tokio::io::AsyncReadExt;
                                let _ = e.read_to_string(&mut stderr).await;
                            }
                            cleanup(&files);
                            bail!("{}", describe_launch_failure(status.code(), &stderr));
                        }
                    }
                    _ = &mut deadline => {
                        cleanup(&files);
                        bail!("timed out waiting for administrator approval");
                    }
                }
            }
        };

        let (rx, tx) = conn.split();
        let mut lines = BufReader::new(rx).lines();
        let hello = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
            .await
            .map_err(|_| anyhow!("helper did not identify itself"))??
            .context("helper closed the connection")?;
        match serde_json::from_str::<HelperMsg>(&hello) {
            Ok(HelperMsg::Hello { token: t }) if t == token => {}
            _ => {
                cleanup(&files);
                bail!("helper failed authentication");
            }
        }

        if let Err(e) = await_started(&mut lines, &logs).await {
            cleanup(&files);
            return Err(e);
        }

        let (stop_tx, stop_rx) = mpsc::channel(1);
        let (done_tx, done) = watch::channel(false);
        tokio::spawn(pump(lines, tx, stop_rx, done_tx, logs, on_exit));
        Ok(Self { stop_tx, done, files })
    }

    pub fn is_running(&self) -> bool {
        !*self.done.borrow()
    }

    pub async fn stop(mut self) {
        let _ = self.stop_tx.send(()).await;
        let _ = tokio::time::timeout(Duration::from_secs(8), self.done.wait_for(|d| *d)).await;
        cleanup(&self.files);
    }
}

/// Reads helper messages until xray is running; fails if it exits first.
async fn await_started<R: AsyncRead + Unpin>(lines: &mut Lines<BufReader<R>>, logs: &Logs) -> Result<()> {
    loop {
        let line = tokio::time::timeout(Duration::from_secs(20), lines.next_line())
            .await
            .map_err(|_| anyhow!("helper did not start xray in time"))??
            .context("helper exited unexpectedly")?;
        match serde_json::from_str::<HelperMsg>(&line) {
            Ok(HelperMsg::Started { pid }) => {
                logs.app(format!("TUN helper started xray (pid {pid})"));
                return Ok(());
            }
            Ok(HelperMsg::Log { line }) => logs.xray(&line),
            Ok(HelperMsg::Exited { error, .. }) => {
                bail!("helper failed: {}", error.unwrap_or_else(|| "xray exited".into()))
            }
            _ => {}
        }
    }
}

async fn pump<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut lines: Lines<BufReader<R>>,
    mut tx: W,
    mut stop_rx: mpsc::Receiver<()>,
    done_tx: watch::Sender<bool>,
    logs: Arc<Logs>,
    on_exit: OnExit,
) {
    let mut stopping = false;
    let mut exit_code = None;
    loop {
        tokio::select! {
            _ = stop_rx.recv(), if !stopping => {
                stopping = true;
                let mut msg = serde_json::to_vec(&AppMsg::Stop).unwrap();
                msg.push(b'\n');
                if tx.write_all(&msg).await.is_err() {
                    break;
                }
            }
            line = lines.next_line() => match line {
                Ok(Some(line)) => match serde_json::from_str::<HelperMsg>(&line) {
                    Ok(HelperMsg::Log { line }) => logs.xray(&line),
                    Ok(HelperMsg::Exited { code, error }) => {
                        if let Some(e) = error {
                            logs.push("error", format!("helper: {e}"));
                        }
                        exit_code = code;
                        break;
                    }
                    _ => {}
                },
                _ => break,
            }
        }
    }
    let _ = done_tx.send(true);
    if !stopping {
        on_exit(exit_code);
    }
}

/// Removes the macOS background service (one administrator prompt).
#[cfg(target_os = "macos")]
pub async fn uninstall_service(paths: &Paths) -> Result<()> {
    let helper = paths.sidecar("aether-helper");
    let shell = format!("{} uninstall", sh_quote(&helper.to_string_lossy()));
    let script = format!(
        "do shell script \"{}\" with prompt \"Aether will remove its VPN service.\" with administrator privileges",
        shell.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let out = Command::new("/usr/bin/osascript").arg("-e").arg(script).output().await?;
    if !out.status.success() {
        bail!("{}", describe_launch_failure(out.status.code(), &String::from_utf8_lossy(&out.stderr)));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
mod daemon {
    use aether_core::helper_proto::{DAEMON_DIR, DAEMON_SOCKET};
    use tokio::net::UnixStream;

    use super::*;

    /// The installed daemon is current if its binaries match the bundled ones
    /// and are root-owned and not writable by anyone else.
    fn installed_is_current(paths: &Paths) -> bool {
        use std::os::unix::fs::MetadataExt;
        ["aether-helper", "xray"].iter().all(|name| {
            let (bundled, installed) = (paths.sidecar(name), Path::new(DAEMON_DIR).join(name));
            match (std::fs::metadata(&bundled), std::fs::metadata(&installed)) {
                (Ok(a), Ok(b)) if a.len() == b.len() && b.uid() == 0 && b.mode() & 0o022 == 0 => {
                    matches!((std::fs::read(&bundled), std::fs::read(&installed)), (Ok(x), Ok(y)) if x == y)
                }
                _ => false,
            }
        })
    }

    async fn reachable() -> bool {
        UnixStream::connect(DAEMON_SOCKET).await.is_ok()
    }

    /// Installs or updates the daemon — the only time a password is asked.
    async fn ensure_installed(paths: &Paths, logs: &Logs) -> Result<()> {
        let current = {
            let paths = paths.clone();
            tokio::task::spawn_blocking(move || installed_is_current(&paths)).await?
        };
        if current && reachable().await {
            return Ok(());
        }
        let helper = paths.sidecar("aether-helper");
        if !helper.is_file() {
            bail!("aether-helper not found at {}", helper.display());
        }
        logs.app("Installing Aether's VPN service (one-time administrator prompt)…");
        // SAFETY: plain syscall.
        let uid = unsafe { libc::getuid() };
        let shell = format!("{} install --uid {uid}", sh_quote(&helper.to_string_lossy()));
        let script = format!(
            "do shell script \"{}\" with prompt \"Aether needs to install its VPN service once. After that, TUN mode connects without asking for your password.\" with administrator privileges",
            shell.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let out = tokio::time::timeout(
            AUTH_TIMEOUT,
            Command::new("/usr/bin/osascript").arg("-e").arg(script).output(),
        )
        .await
        .map_err(|_| anyhow!("timed out waiting for administrator approval"))??;
        if !out.status.success() {
            bail!("{}", describe_launch_failure(out.status.code(), &String::from_utf8_lossy(&out.stderr)));
        }
        for _ in 0..50 {
            if reachable().await {
                logs.app("VPN service installed");
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        bail!("the VPN service was installed but is not responding (see /Library/Logs/aether-helper.log)")
    }

    pub async fn start(paths: &Paths, config: &Path, ipv6: bool, logs: Arc<Logs>, on_exit: OnExit) -> Result<TunSession> {
        ensure_installed(paths, &logs).await?;
        let stream = UnixStream::connect(DAEMON_SOCKET).await.context("connecting to the VPN service")?;
        let (rx, mut tx) = stream.into_split();
        let mut lines = BufReader::new(rx).lines();

        let hello = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
            .await
            .map_err(|_| anyhow!("the VPN service is busy — is another Aether window connected?"))??
            .context("the VPN service closed the connection")?;
        match serde_json::from_str::<HelperMsg>(&hello) {
            Ok(HelperMsg::Daemon { .. }) => {}
            _ => bail!("unexpected greeting from the VPN service"),
        }

        let config: serde_json::Value = serde_json::from_slice(&std::fs::read(config)?)?;
        let start = AppMsg::Start { config, assets: paths.assets.to_string_lossy().into_owned(), ipv6 };
        let mut msg = serde_json::to_vec(&start)?;
        msg.push(b'\n');
        tx.write_all(&msg).await?;

        await_started(&mut lines, &logs).await?;
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let (done_tx, done) = watch::channel(false);
        tokio::spawn(pump(lines, tx, stop_rx, done_tx, logs, on_exit));
        Ok(TunSession { stop_tx, done, files: Vec::new() })
    }
}

fn cleanup(files: &[PathBuf]) {
    for f in files {
        let _ = std::fs::remove_file(f);
    }
}

fn describe_launch_failure(code: Option<i32>, stderr: &str) -> String {
    let stderr = stderr.trim();
    if stderr.contains("-128") || stderr.contains("User canceled") || matches!(code, Some(126)) {
        "administrator authorization was cancelled".into()
    } else if matches!(code, Some(127)) && cfg!(target_os = "linux") {
        "not authorized (is polkit / pkexec installed?)".into()
    } else if stderr.is_empty() {
        format!("failed to obtain administrator rights (exit code {code:?})")
    } else {
        format!("failed to obtain administrator rights: {stderr}")
    }
}

/// Single-quotes a string for POSIX `sh`.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Starts the helper elevated. The returned process is the *launcher*: on
/// macOS/Windows it exits as soon as the helper is running; with pkexec it
/// lives as long as the helper.
fn elevate(helper: &Path, args: &[String]) -> Result<tokio::process::Child> {
    let mut cmd;
    // Already privileged (or told not to elevate, e.g. Linux with
    // CAP_NET_ADMIN granted to xray): run the helper directly.
    #[cfg(unix)]
    let already_root = unsafe { libc::geteuid() } == 0;
    #[cfg(not(unix))]
    let already_root = false;
    if already_root || std::env::var_os("AETHER_TUN_NO_ELEVATE").is_some() {
        cmd = Command::new(helper);
        cmd.args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(false);
        return Ok(cmd.spawn()?);
    }
    #[cfg(target_os = "macos")]
    {
        let shell = std::iter::once(helper.to_string_lossy().into_owned())
            .chain(args.iter().cloned())
            .map(|a| sh_quote(&a))
            .collect::<Vec<_>>()
            .join(" ");
        // Background the helper so `do shell script` returns immediately.
        let shell = format!("{shell} > /dev/null 2>&1 &");
        let script = format!(
            "do shell script \"{}\" with prompt \"Aether needs administrator rights to create the VPN network interface.\" with administrator privileges",
            shell.replace('\\', "\\\\").replace('"', "\\\"")
        );
        cmd = Command::new("/usr/bin/osascript");
        cmd.arg("-e").arg(script);
    }
    #[cfg(target_os = "linux")]
    {
        let _ = sh_quote;
        cmd = Command::new("pkexec");
        cmd.arg(helper).args(args);
    }
    #[cfg(windows)]
    {
        let _ = sh_quote;
        let ps_quote = |s: &str| format!("'{}'", s.replace('\'', "''"));
        // Each argument is quoted for the Windows command line, then the
        // whole list is passed to Start-Process as a PowerShell array.
        let win_args = args
            .iter()
            .map(|a| ps_quote(&format!("\"{}\"", a.replace('"', "\\\""))))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            "Start-Process -FilePath {} -ArgumentList {} -Verb RunAs -WindowStyle Hidden",
            ps_quote(&helper.to_string_lossy()),
            win_args
        );
        cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &script]);
        cmd.creation_flags(0x0800_0000);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    Ok(cmd.spawn()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vpn_interface_names() {
        assert!(is_vpn_interface("utun225"));
        assert!(is_vpn_interface("wg0"));
        assert!(!is_vpn_interface("en0"));
        assert!(!is_vpn_interface("eth0"));
    }

    #[test]
    fn quoting() {
        assert_eq!(sh_quote("/a b/c"), "'/a b/c'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn launch_failure_messages() {
        assert!(describe_launch_failure(Some(1), "execution error: User canceled. (-128)").contains("cancelled"));
        assert!(describe_launch_failure(Some(1), "").contains("exit code"));
    }
}
