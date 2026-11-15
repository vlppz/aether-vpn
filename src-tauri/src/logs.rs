//! In-memory log ring buffer shared by Xray output and app messages.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

const CAPACITY: usize = 5000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub ts: u64,
    pub level: &'static str,
    pub message: String,
}

pub struct Logs {
    app: AppHandle,
    lines: Mutex<VecDeque<LogLine>>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

/// Splits an Xray log line into (level, message), dropping its timestamp.
pub fn parse_xray_line(raw: &str) -> (&'static str, String) {
    // "2026/10/20 10:58:38.035153 [Warning] core: Xray started"
    let mut rest = raw.trim();
    let mut parts = rest.splitn(3, ' ');
    if let (Some(date), Some(time), Some(tail)) = (parts.next(), parts.next(), parts.next()) {
        if date.len() == 10 && date.as_bytes()[4] == b'/' && time.contains(':') {
            rest = tail;
        }
    }
    for (tag, level) in [
        ("[Debug] ", "debug"),
        ("[Info] ", "info"),
        ("[Warning] ", "warning"),
        ("[Error] ", "error"),
    ] {
        if let Some(msg) = rest.strip_prefix(tag) {
            return (level, msg.to_string());
        }
    }
    if rest.contains(" accepted ") || rest.contains(" rejected ") {
        return ("access", rest.to_string());
    }
    if rest.starts_with("Failed") || rest.contains("panic") {
        return ("error", rest.to_string());
    }
    ("info", rest.to_string())
}

impl Logs {
    pub fn new(app: AppHandle) -> Self {
        Self { app, lines: Mutex::new(VecDeque::with_capacity(CAPACITY)) }
    }

    pub fn push(&self, level: &'static str, message: impl Into<String>) {
        let line = LogLine { ts: now_ms(), level, message: message.into() };
        match level {
            "error" => tracing::error!("{}", line.message),
            "warning" => tracing::warn!("{}", line.message),
            _ => tracing::debug!("[{}] {}", line.level, line.message),
        }
        {
            let mut lines = self.lines.lock().unwrap();
            if lines.len() == CAPACITY {
                lines.pop_front();
            }
            lines.push_back(line.clone());
        }
        let _ = self.app.emit("log", line);
    }

    pub fn app(&self, message: impl Into<String>) {
        let message = message.into();
        tracing::info!("{message}");
        self.push("app", message);
    }

    pub fn xray(&self, raw: &str) {
        if raw.trim().is_empty() {
            return;
        }
        let (level, message) = parse_xray_line(raw);
        self.push(level, message);
    }

    pub fn all(&self) -> Vec<LogLine> {
        self.lines.lock().unwrap().iter().cloned().collect()
    }

    /// Last few error-ish lines, used to explain why Xray failed to start.
    /// Prefers real errors; falls back to warnings when there are none.
    pub fn recent_errors(&self, n: usize) -> Vec<String> {
        let lines = self.lines.lock().unwrap();
        let pick = |pred: &dyn Fn(&LogLine) -> bool| -> Vec<String> {
            let mut v: Vec<String> =
                lines.iter().rev().filter(|l| pred(l)).take(n).map(|l| l.message.clone()).collect();
            v.reverse();
            v
        };
        let errors = pick(&|l| l.level == "error");
        if errors.is_empty() {
            pick(&|l| l.level == "warning")
        } else {
            errors
        }
    }

    pub fn clear(&self) {
        self.lines.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::parse_xray_line;

    #[test]
    fn parses_xray_lines() {
        assert_eq!(
            parse_xray_line("2026/10/20 10:58:38.035153 [Warning] core: Xray 26.3.27 started"),
            ("warning", "core: Xray 26.3.27 started".into())
        );
        assert_eq!(
            parse_xray_line("2026/10/20 10:58:39.544132 from tcp:127.0.0.1:61634 accepted tcp:www.gstatic.com:443 [socks-in >> direct]").0,
            "access"
        );
        assert_eq!(parse_xray_line("Failed to start: main: x").0, "error");
        assert_eq!(parse_xray_line("A unified platform for anti-censorship.").0, "info");
    }
}
