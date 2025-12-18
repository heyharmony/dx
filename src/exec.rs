use std::env;
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::Result;
use portable_pty::{
    Child as PtyChild, CommandBuilder as PtyCommandBuilder, MasterPty, PtySize, native_pty_system,
};

/// Streamed output events from a running process.
#[derive(Debug)]
pub enum OutputMsg {
    /// Full line completed by a newline
    Line(String),
    /// Carriage-return update of the current line
    ReplaceCurrent(String),
    /// Raw PTY bytes chunk for terminal emulation
    Chunk(Vec<u8>),
}

/// Handles and channel for a spawned PTY command.
pub struct Spawned {
    pub child: Box<dyn PtyChild + Send>,
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub rx: tokio::sync::mpsc::Receiver<OutputMsg>,
}

/// Spawn a command under a PTY and return handles plus a channel of `OutputMsg` events.
/// The command is run via `bash -lc "<cmd>; code=$?; printf '\n__DX_EXIT_CODE:%d\n' "$code""` to capture exit code.
/// 
/// # Errors
/// Returns error if PTY creation or command spawn fails.
pub fn spawn_pty(cmd_str: &str) -> Result<Spawned> {
    spawn_pty_with_size(cmd_str, 24, 120)
}

/// Spawn a command under a PTY with specific dimensions and return handles plus a channel of `OutputMsg` events.
/// 
/// # Errors
/// Returns error if PTY creation with specific size or command spawn fails.
pub fn spawn_pty_with_size(cmd_str: &str, rows: u16, cols: u16) -> Result<Spawned> {
    // Create PTY pair with specified size
    let pty_system = native_pty_system();
    let pair = pty_system.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    // Prepare shell command (use user's shell when available)
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let mut cmd = PtyCommandBuilder::new(shell);
    cmd.arg("-lc");
    let wrapped = format!("{cmd_str}; code=$?; printf '\n__DX_EXIT_CODE:%d\n' \"$code\"");
    cmd.arg(wrapped);
    
    // Set working directory to where dx was invoked
    if let Ok(original_cwd) = env::current_dir() {
        cmd.cwd(&original_cwd);
    }
    // Environment suitable for color/TUI apps - enhanced for better terminal detection
    cmd.env(
        "TERM",
        env::var("TERM").unwrap_or_else(|_| "xterm-256color".to_string()),
    );
    cmd.env(
        "COLORTERM",
        env::var("COLORTERM").unwrap_or_else(|_| "truecolor".to_string()),
    );
    cmd.env("CLICOLOR_FORCE", "1");
    cmd.env("FORCE_COLOR", "1");
    // Additional environment variables to improve terminal detection
    cmd.env("TERM_PROGRAM", "dx");
    cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
    cmd.env("LINES", rows.to_string());
    cmd.env("COLUMNS", cols.to_string());
    // Make applications think they're in a real interactive terminal
    cmd.env("PS1", r"\u@\h:\w\$ "); // Set a proper prompt
    cmd.env("INTERACTIVE", "1");
    // Force unbuffered output for many programs
    cmd.env("PYTHONUNBUFFERED", "1");
    cmd.env("RUST_BACKTRACE", "1");
    // Terminal capability flags
    cmd.env("TERM_FEATURES", "256color:mouse:utf8");
    cmd.env("TERMINAL_EMULATOR", "dx");
    // Prevent programs from detecting they're in a wrapper
    if env::var("_").is_err() {
        cmd.env("_", "/usr/bin/dx");
    }
    // Ensure cargo is found when installed in ~/.cargo/bin
    if let (Ok(home), Ok(path)) = (env::var("HOME"), env::var("PATH")) {
        let cargo_bin = format!("{home}/.cargo/bin");
        if !path.split(':').any(|p| p == cargo_bin) {
            cmd.env("PATH", format!("{cargo_bin}:{path}"));
        }
    }

    let child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);

    // Take writer for key forwarding
    let writer = pair.master.take_writer()?;

    // Reader task to convert PTY bytes into OutputMsg stream
    let mut reader = pair.master.try_clone_reader()?;
    let (tx, rx) = tokio::sync::mpsc::channel::<OutputMsg>(2048); // Increased buffer size
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 8192]; // Larger read buffer for better performance
        let mut cur = String::new();
        let mut last_activity = std::time::Instant::now();

        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    if !cur.is_empty() {
                        let _ = tx.blocking_send(OutputMsg::ReplaceCurrent(cur.clone()));
                    }
                    break;
                }
                Ok(n) => {
                    // Always send raw chunks first for terminal emulators
                    let _ = tx.blocking_send(OutputMsg::Chunk(buf[..n].to_vec()));

                    // Process text for line-based display
                    let s = String::from_utf8_lossy(&buf[..n]);
                    let mut chars = s.chars().peekable();

                    while let Some(ch) = chars.next() {
                        match ch {
                            '\u{0000}' => {}
                            '\r' => {
                                // Handle carriage return - check if followed by newline
                                if chars.peek() == Some(&'\n') {
                                    // CR+LF sequence
                                    let _ = chars.next(); // consume the LF
                                    let line = std::mem::take(&mut cur);
                                    let _ = tx.blocking_send(OutputMsg::Line(line));
                                } else {
                                    // Just CR - replace current line
                                    cur.clear();
                                }
                            }
                            '\n' => {
                                let line = std::mem::take(&mut cur);
                                let _ = tx.blocking_send(OutputMsg::Line(line));
                            }
                            c => cur.push(c),
                        }
                    }

                    // Send current line state if we have content and some time has passed
                    // This helps with progress indicators and live output
                    let now = std::time::Instant::now();
                    if !cur.is_empty()
                        && (now.duration_since(last_activity).as_millis() > 50 || cur.len() > 100)
                    {
                        let _ = tx.blocking_send(OutputMsg::ReplaceCurrent(cur.clone()));
                        last_activity = now;
                    }
                }
                Err(_) => break,
            }
        }
    });

    Ok(Spawned {
        child,
        master: pair.master,
        writer,
        rx,
    })
}

// Convenience helpers so UI code does not touch PTY primitives directly
pub fn pty_write(writer: &mut Option<Box<dyn Write + Send>>, bytes: &[u8]) {
    if let Some(w) = writer {
        let _ = w.write_all(bytes);
        let _ = w.flush();
    }
}

#[allow(dead_code)]
pub fn pty_kill(child: &mut Option<Box<dyn PtyChild + Send>>) {
    if let Some(c) = child {
        let _ = c.kill();
    }
}

pub fn pty_resize(master: &mut Option<Box<dyn MasterPty + Send>>, size: PtySize) {
    if let Some(m) = master {
        let _ = m.resize(size);
    }
}

/// Find the project root directory by searching for dx.yaml, dx.toml, menu.yaml, etc.
/// Search priority: ./.dx/, ~/.dx/, current directory, then parent directories.
/// Returns the directory containing the config file, or current_dir if not found.
pub fn find_project_root() -> PathBuf {
    let menu_candidates = [
        "dx.yaml", "dx.yml", "dx.toml", "dx.json",
        "DX.yaml", "DX.yml", "DX.toml", "DX.json", 
        "menu.yaml", "menu.yml", "menu.toml", "menu.json",
        "Menu.yaml", "Menu.yml", "Menu.toml", "Menu.json",
    ];

    let current = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let high_priority = ["dx.yaml", "dx.yml", "dx.toml", "dx.json", "DX.yaml", "DX.yml", "DX.toml", "DX.json"];
    
    // FIRST: Check local .dx/ subdirectory (highest priority)
    let local_dx_dir = current.join(".dx");
    for candidate in &high_priority {
        if local_dx_dir.join(candidate).exists() {
            return local_dx_dir;
        }
    }
    // Also check for menu.* in local .dx directory
    for candidate in &menu_candidates {
        if local_dx_dir.join(candidate).exists() {
            return local_dx_dir;
        }
    }
    
    // SECOND: Check global ~/.dx/ directory
    if let Ok(home) = env::var("HOME") {
        let global_dir = PathBuf::from(home).join(".dx");
        for candidate in &high_priority {
            if global_dir.join(candidate).exists() {
                return global_dir;
            }
        }
        // Also check for menu.* in global directory
        for candidate in &menu_candidates {
            if global_dir.join(candidate).exists() {
                return global_dir;
            }
        }
    }
    
    // THIRD: Check current directory for dx.* files
    for candidate in &high_priority {
        if current.join(candidate).exists() {
            return current;
        }
    }
    
    // FOURTH: Check current directory for menu.* files  
    for candidate in &menu_candidates {
        if current.join(candidate).exists() {
            return current;
        }
    }
    
    // FINALLY: Search parent directories if nothing found
    let mut search_dir = current.clone();
    loop {
        match search_dir.parent() {
            Some(parent) => {
                if parent == search_dir {
                    // Reached filesystem root
                    break;
                }
                search_dir = parent.to_path_buf();
                
                // Only search for dx.* (not menu.*) in parent directories to avoid conflicts
                for candidate in &high_priority {
                    if search_dir.join(candidate).exists() {
                        return search_dir;
                    }
                }
            }
            None => break,
        }
    }
    
    // Fallback to current working directory if no config found
    current
}
