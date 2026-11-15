//! Filesystem locations used by the app.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone)]
pub struct Paths {
    /// Persistent data: profiles, settings.
    pub data: PathBuf,
    /// Runtime files: generated configs, IPC sockets, tokens.
    pub run: PathBuf,
    /// Directory containing geoip.dat / geosite.dat.
    pub assets: PathBuf,
    /// Directory containing the sidecar executables.
    pub bin: PathBuf,
}

impl Paths {
    pub fn resolve(app: &AppHandle) -> Result<Self> {
        // AETHER_DATA_DIR isolates test runs from the real profile.
        let data = match std::env::var_os("AETHER_DATA_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => app.path().app_data_dir().context("no app data dir")?,
        };
        let run = data.join("run");
        std::fs::create_dir_all(&run)?;

        let bin = std::env::current_exe()?
            .parent()
            .context("executable has no parent dir")?
            .to_path_buf();

        let resource_dir = app.path().resource_dir().ok();
        let dev_resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let assets = resource_dir
            .into_iter()
            .chain([dev_resources])
            .find(|d| d.join("geosite.dat").is_file())
            .context("geoip.dat/geosite.dat not found — run scripts/fetch-core")?;

        Ok(Self { data, run, assets, bin })
    }

    pub fn sidecar(&self, name: &str) -> PathBuf {
        let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
        self.bin.join(file)
    }
}

/// Writes a file atomically (tmp + rename) with owner-only permissions on unix.
pub fn write_atomic(path: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension("tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}
