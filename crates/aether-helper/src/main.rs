//! `aether-helper` — the only part of Aether that runs with elevated
//! privileges. Creating a TUN interface and changing routes and DNS needs
//! root/administrator rights, and we do not want to run the whole GUI
//! elevated.
//!
//! Hardening:
//! * it only ever executes the `xray` binary that sits next to itself
//!   (for the daemon: a root-owned copy in /Library/PrivilegedHelperTools);
//! * the config is copied into a private temp file and any log file paths
//!   are stripped, so a crafted config cannot make root write arbitrary files;
//! * a session lives exactly as long as its IPC connection to the GUI;
//! * the daemon only serves the uid of the user who installed it.
//!
//! Modes:
//! * one-shot, spawned per connection through osascript / pkexec / UAC:
//!   `aether-helper --ipc <socket> --token-file <path> --config <path> --assets <dir> --state-dir <dir> [--ipv6 1]`
//! * macOS launchd daemon (no password per connect):
//!   `aether-helper daemon --uid <uid>`, set up by `aether-helper install --uid <uid>`
//!   and removed by `aether-helper uninstall`.

#[cfg(unix)]
mod daemon;
mod dns;
mod net;
mod session;

use std::path::PathBuf;

use aether_core::helper_proto::{socket_name, HelperMsg};
use anyhow::{bail, Context, Result};
use interprocess::local_socket::tokio::{prelude::*, Stream};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::session::{send, supervise, Params};

struct Args {
    ipc: String,
    token_file: PathBuf,
    config: PathBuf,
    assets: PathBuf,
    state_dir: PathBuf,
    ipv6: bool,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args> {
    let mut it = args;
    let (mut ipc, mut token_file, mut config, mut assets, mut state_dir) = (None, None, None, None, None);
    let mut ipv6 = false;
    while let Some(flag) = it.next() {
        let value = it.next().with_context(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--ipc" => ipc = Some(value),
            "--token-file" => token_file = Some(PathBuf::from(value)),
            "--config" => config = Some(PathBuf::from(value)),
            "--assets" => assets = Some(PathBuf::from(value)),
            "--state-dir" => state_dir = Some(PathBuf::from(value)),
            "--ipv6" => ipv6 = value == "1",
            other => bail!("unknown argument {other}"),
        }
    }
    Ok(Args {
        ipc: ipc.context("--ipc is required")?,
        token_file: token_file.context("--token-file is required")?,
        config: config.context("--config is required")?,
        assets: assets.context("--assets is required")?,
        state_dir: state_dir.context("--state-dir is required")?,
        ipv6,
    })
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let code = match run().await {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("aether-helper: {e:#}");
            1
        }
    };
    std::process::exit(code);
}

async fn run() -> Result<()> {
    let mut argv = std::env::args().skip(1).peekable();
    match argv.peek().map(String::as_str) {
        #[cfg(unix)]
        Some("daemon") => return daemon::serve(daemon::uid_arg(argv.skip(1))?).await,
        #[cfg(unix)]
        Some("install") => return daemon::install(daemon::uid_arg(argv.skip(1))?),
        #[cfg(unix)]
        Some("uninstall") => return daemon::uninstall(),
        _ => {}
    }
    one_shot(parse_args(argv)?).await
}

/// Per-connection mode: connect back to the app's socket, authenticate with
/// the token, run one session.
async fn one_shot(args: Args) -> Result<()> {
    std::fs::create_dir_all(&args.state_dir).ok();
    // Undo leftovers from a previous helper that was killed hard.
    dns::restore_stale(&args.state_dir);

    let token = std::fs::read_to_string(&args.token_file).context("reading token file")?;
    let raw = std::fs::read(&args.config).with_context(|| format!("reading {}", args.config.display()))?;
    let config = serde_json::from_slice(&raw).context("config is not valid JSON")?;
    let params = Params { config, assets: args.assets, state_dir: args.state_dir, ipv6: args.ipv6 };

    let conn = Stream::connect(socket_name(&args.ipc)?).await.context("connecting to Aether")?;
    let (rx, mut tx) = conn.split();
    send(&mut tx, &HelperMsg::Hello { token: token.trim().to_string() }).await?;

    let result = supervise(&params, BufReader::new(rx).lines(), &mut tx).await;
    if let Err(e) = &result {
        let _ = send(&mut tx, &HelperMsg::Exited { code: None, error: Some(format!("{e:#}")) }).await;
    }
    dns::restore(&params.state_dir);
    result
}
