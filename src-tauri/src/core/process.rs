//! Supervision of a local (unprivileged) Xray process.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, watch};

use crate::logs::Logs;

/// Callback invoked when the process exits on its own (not via [`XrayProcess::stop`]).
pub type OnExit = Box<dyn FnOnce(Option<i32>) + Send>;

pub struct XrayProcess {
    stop_tx: Option<oneshot::Sender<()>>,
    done: watch::Receiver<bool>,
}

pub fn pipe_lines<R: AsyncRead + Unpin + Send + 'static>(reader: R, logs: Arc<Logs>) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            logs.xray(&line);
        }
    });
}

/// SIGTERM first so Xray can clean up, SIGKILL after a grace period.
pub async fn terminate(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        // SAFETY: plain syscall on a pid we own.
        unsafe { libc::kill(pid as i32, libc::SIGTERM) };
        if tokio::time::timeout(Duration::from_secs(2), child.wait()).await.is_ok() {
            return;
        }
    }
    let _ = child.kill().await;
}

impl XrayProcess {
    pub fn spawn(xray: &Path, config: &Path, assets: &Path, logs: Arc<Logs>, on_exit: OnExit) -> Result<Self> {
        let mut cmd = Command::new(xray);
        cmd.args(["run", "-c"])
            .arg(config)
            .env("XRAY_LOCATION_ASSET", assets)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        {
            // CREATE_NO_WINDOW: no console flashing up
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn().with_context(|| format!("starting {}", xray.display()))?;
        pipe_lines(child.stdout.take().unwrap(), logs.clone());
        pipe_lines(child.stderr.take().unwrap(), logs);

        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        let (done_tx, done) = watch::channel(false);
        tokio::spawn(async move {
            tokio::select! {
                status = child.wait() => {
                    let _ = done_tx.send(true);
                    on_exit(status.ok().and_then(|s| s.code()));
                }
                _ = stop_rx => {
                    terminate(&mut child).await;
                    let _ = done_tx.send(true);
                }
            }
        });
        Ok(Self { stop_tx: Some(stop_tx), done })
    }

    pub fn is_running(&self) -> bool {
        !*self.done.borrow()
    }

    pub async fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        let _ = self.done.wait_for(|d| *d).await;
    }
}
