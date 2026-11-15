//! Wire protocol between the GUI and the privileged TUN helper.
//!
//! The GUI owns a local socket (unix socket file / Windows named pipe) and
//! the elevated helper connects to it, proving its identity with a token the
//! GUI wrote to a 0600 file. Messages are newline-delimited JSON. When the
//! GUI's end of the socket closes, the helper tears everything down, so a
//! crashed GUI never leaves a root process or hijacked routes behind.

//!
//! On macOS the helper can also run as a launchd daemon (installed once with
//! a single password prompt). Then the roles flip: the daemon owns a
//! root-created socket in `/var/run`, checks the connecting user's uid, greets
//! with [`HelperMsg::Daemon`] and waits for [`AppMsg::Start`].

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Socket of the macOS launchd daemon (created by root, so it can't be spoofed).
pub const DAEMON_SOCKET: &str = "/var/run/app.aether.vpn.helper.sock";
/// Where the daemon's root-owned binaries live.
pub const DAEMON_DIR: &str = "/Library/PrivilegedHelperTools/app.aether.vpn";
pub const DAEMON_LABEL: &str = "app.aether.vpn.helper";
pub const DAEMON_PLIST: &str = "/Library/LaunchDaemons/app.aether.vpn.helper.plist";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum HelperMsg {
    Hello { token: String },
    /// Daemon greeting on connect.
    Daemon { version: String },
    Started { pid: u32 },
    Log { line: String },
    Exited { code: Option<i32>, error: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AppMsg {
    Stop,
    /// Daemon mode: run this config (the one-shot helper gets it as a file).
    Start { config: Value, assets: String, ipv6: bool },
}

/// Builds the platform socket name: a filesystem path on unix, a pipe name on Windows.
#[cfg(feature = "ipc")]
pub fn socket_name(id: &str) -> std::io::Result<interprocess::local_socket::Name<'static>> {
    #[cfg(windows)]
    {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        id.to_string().to_ns_name::<GenericNamespaced>()
    }
    #[cfg(not(windows))]
    {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        id.to_string().to_fs_name::<GenericFilePath>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format() {
        let m = HelperMsg::Exited { code: Some(1), error: None };
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(s, r#"{"type":"exited","code":1,"error":null}"#);
        assert_eq!(serde_json::from_str::<AppMsg>(r#"{"type":"stop"}"#).unwrap(), AppMsg::Stop);
    }
}
