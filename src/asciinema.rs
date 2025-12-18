// TODO: Fix clippy warnings for better code quality
#![allow(clippy::collapsible_if)] // TODO: Simplify nested if statements
#![allow(clippy::uninlined_format_args)] // TODO: Use {var} format syntax

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(clippy::struct_excessive_bools)] // Configuration flags are justified as bools
pub struct AsciinemaConfig {
    #[serde(default)]
    pub enabled: bool,
    /// When true, wrap external (passthrough) commands with asciinema rec
    #[serde(default)]
    pub external: bool,
    /// When true, wrap update relaunch with asciinema rec
    #[serde(default)]
    pub on_relaunch: bool,
    /// Optional directory to save recordings into
    pub dir: Option<String>,
    /// Optional filename prefix; final name becomes: <prefix>-<`unix_ts`>.cast
    pub file_prefix: Option<String>,
    /// Optional recording title shown by asciinema
    pub title: Option<String>,
    /// Quiet mode (suppresses asciinema prompts)
    #[serde(default)]
    pub quiet: bool,
    /// Overwrite file if exists (emulated by pre-deleting the file)
    #[serde(default)]
    pub overwrite: bool,
    /// Use live streaming (asciinema stream) instead of recording
    #[serde(default)]
    pub stream: bool,
    /// Streaming mode: "local" (default) or "remote"
    #[serde(default = "default_stream_mode")]
    pub stream_mode: String,
    /// For local mode: optional address like "127.0.0.1:9000"
    pub local_addr: Option<String>,
    /// For remote mode: STREAM-ID or ws:// URL
    pub remote: Option<String>,
}

#[must_use]
pub fn default_stream_mode() -> String {
    "remote".to_string()
}

// Helper: minimal shell-quote for sh -lc
#[must_use]
pub fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[allow(dead_code)]
#[must_use]
pub fn os_str_shell_quote(s: &std::ffi::OsStr) -> String {
    match s.to_str() {
        Some(t) => shell_quote(t),
        None => shell_quote(""),
    }
}

// Build asciinema record command line
#[must_use]
pub fn build_asciinema_cmd(cfg: &AsciinemaConfig, file_path: &str, inner_cmd: &str) -> String {
    let mut parts: Vec<String> = vec!["asciinema".to_string(), "record".to_string()];
    if cfg.quiet {
        parts.push("-q".to_string());
    }
    // '-y' is not a valid flag in many asciinema versions; emulate overwrite by pre-deleting the file
    if let Some(t) = &cfg.title {
        parts.push("-t".to_string());
        parts.push(t.clone());
    }
    parts.push(file_path.to_string());
    parts.push("-c".to_string());
    parts.push(inner_cmd.to_string());
    let joined: Vec<String> = parts.into_iter().map(|p| shell_quote(&p)).collect();
    joined.join(" ")
}

#[must_use]
pub fn build_asciinema_stream_cmd(cfg: &AsciinemaConfig, inner_cmd: &str) -> String {
    let mut parts: Vec<String> = vec!["asciinema".to_string(), "stream".to_string()];
    if cfg.quiet {
        parts.push("--quiet".to_string());
    }
    if let Some(t) = &cfg.title {
        parts.push("-t".to_string());
        parts.push(t.clone());
    }
    // Required: either --local [addr] or --remote <id|ws-url>
    if cfg.stream_mode.eq_ignore_ascii_case("remote") {
        if let Some(r) = &cfg.remote {
            parts.push("--remote".to_string());
            parts.push(r.clone());
        } else {
            // Fallback to local mode when remote target not provided
            parts.push("--local".to_string());
            if let Some(addr) = &cfg.local_addr {
                parts.push(addr.clone());
            }
        }
    } else {
        parts.push("--local".to_string());
        if let Some(addr) = &cfg.local_addr {
            parts.push(addr.clone());
        }
    }
    parts.push("--command".to_string());
    parts.push(inner_cmd.to_string());
    let joined: Vec<String> = parts.into_iter().map(|p| shell_quote(&p)).collect();
    joined.join(" ")
}

// Create filename based on config
#[must_use]
pub fn generate_asciinema_filename(cfg: &AsciinemaConfig) -> String {
    let dir = cfg.dir.clone().unwrap_or_else(|| ".".to_string());
    let prefix = cfg.file_prefix.clone().unwrap_or_else(|| "dx".to_string());
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{dir}/{prefix}-{ts}.cast")
}

#[allow(dead_code)]
#[must_use]
pub fn viewer_url_from_remote(remote: &str) -> Option<String> {
    let r = remote.trim();
    if r.is_empty() {
        return None;
    }
    if r.starts_with("http://") || r.starts_with("https://") {
        return Some(r.to_string());
    }
    if r.starts_with("ws://") || r.starts_with("wss://") {
        // Heuristic: if it's an asciinema host, take the last path segment as ID
        if let Ok(url) = url::Url::parse(r) {
            if let Some(host) = url.host_str() {
                if host.contains("asciinema.org") {
                    if let Some(id) = url
                        .path_segments()
                        .and_then(|mut s| s.next_back())
                        .filter(|s| !s.is_empty())
                    {
                        return Some(format!("https://asciinema.org/s/{id}"));
                    }
                }
            }
        }
        return None;
    }
    // Assume it's a bare stream ID
    Some(format!("https://asciinema.org/s/{r}"))
}

pub fn first_url_in(text: &str) -> Option<String> {
    let t = text.trim();
    let starts = ["https://", "http://", "wss://", "ws://"];
    let mut start_idx: Option<usize> = None;
    for s in &starts {
        if let Some(i) = t.find(s) {
            start_idx = Some(i);
            break;
        }
    }
    let i = start_idx?;
    let rest = &t[i..];
    // URL ends at whitespace
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}
