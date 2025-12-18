// TODO: Fix clippy warnings for better code quality
#![allow(clippy::uninlined_format_args)] // TODO: Use {var} format syntax instead of format!("{}", var)
#![allow(clippy::collapsible_else_if)] // TODO: Simplify nested if-else chains
#![allow(clippy::large_enum_variant)] // TODO: Box large enum variants like OutputView
#![allow(clippy::match_single_binding)] // TODO: Remove unnecessary matches that just bind values
#![allow(clippy::lines_filter_map_ok)] // TODO: Use map_while(Result::ok) instead of lines().flatten()
#![allow(clippy::drop_non_drop)] // TODO: Remove unnecessary drop() calls on non-Drop types
#![allow(clippy::unnecessary_cast)] // TODO: Remove casts like (u16 as u16)
#![allow(clippy::get_first)] // TODO: Use .first() instead of .get(0)
#![allow(clippy::collapsible_match)] // TODO: Simplify nested match patterns

use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Read;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser as ClapParser;
use clap::Subcommand;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode, KeyEvent,
};
use crossterm::event::{MouseEvent, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
// no direct widgets imports needed here
use ratatui::Terminal;
use serde::Deserialize;
use serde::Serialize;
use tracing::{error, warn};
use tracing_subscriber::EnvFilter;
// use ansi_to_tui::IntoText;
// use crate::markdown::markdown_to_text_with_links;
use crate::exec::OutputMsg;
use crate::menu::{
    MenuState, collect_aliases, collect_unaliased_commands, find_item_by_alias, load_menu,
    prepend_readme_item, submenu_at, validate_menu,
};
use portable_pty::{Child as PtyChild, MasterPty, PtySize};
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

mod asciinema;
mod config;
mod exec;
mod frame;
mod markdown;
mod menu;
mod motd;
mod term;
mod theme;
mod view;
pub mod overlay {
    pub mod cpu;
}
mod screens {
    pub mod config;
    pub mod form;
    pub mod menu;
    pub mod output;
}
mod plugin;

fn parse_color(spec: &str) -> Option<Color> {
    theme::parse_color(spec)
}

#[allow(dead_code)]
fn open_default_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg(url).status()?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open").arg(url).status()?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Ok(())
}

// moved to asciinema.rs: viewer_url_from_remote, first_url_in

// Heuristic: detect TUI/alt-screen activation in PTY byte stream
fn bytes_look_like_tui(bytes: &[u8]) -> bool {
    // Detect alternate screen enable sequences commonly used by TUIs
    const ALT1: &[u8] = b"\x1b[?1049h";
    const ALT2: &[u8] = b"\x1b[?47h";
    const ALT3: &[u8] = b"\x1b[?1047h";
    bytes.windows(ALT1.len()).any(|w| w == ALT1)
        || bytes.windows(ALT2.len()).any(|w| w == ALT2)
        || bytes.windows(ALT3.len()).any(|w| w == ALT3)
}

// moved to motd.rs

// Spinner frames used to indicate ongoing activity while a task is running
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]; // Braille spinner

#[derive(ClapParser, Debug)]
#[command(name = "dx", version = env!("DX_VERSION_STRING"), about = "Scrollable TUI: view files or run menu commands")]
struct Cli {
    /// Path to a menu config (TOML/YAML/JSON) with items and commands
    #[arg(long, value_name = "MENU_CONFIG")]
    menu: Option<PathBuf>,

    /// Subcommand "aliases" to list, or alias to run, or path to open
    #[arg(value_name = "COMMAND_OR_ALIAS_OR_PATH")]
    target: Option<String>,

    /// Additional arguments to pass to the target command
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,

    /// Enable live streaming via 'asciinema stream'
    #[arg(long, default_value_t = false)]
    live: bool,

    /// With --live, stay in console (no TUI); stream a shell directly
    #[arg(long, default_value_t = false)]
    console: bool,

    /// Record alias execution via asciinema (direct alias mode)
    #[arg(long, default_value_t = false)]
    record: bool,

    /// Print concise non-interactive usage instructions for LLM agents and exit
    #[arg(long, default_value_t = false)]
    llm: bool,

    /// When used with --live, do not auto-open the streaming URL in a browser
    #[arg(long, default_value_t = false)]
    disable_auto_open: bool,

    /// Subcommands: stream/record passthrough to asciinema
    #[command(subcommand)]
    cmd: Option<DxCmd>,
}

#[derive(Subcommand, Debug)]
enum DxCmd {
    /// Start live streaming via asciinema (remote). Optional STREAM_ID to attach to.
    Stream {
        /// STREAM-ID or ws:// URL (optional). When omitted, a new remote stream is created.
        stream_id: Option<String>,
    },
    /// Start recording a shell session via asciinema record.
    Record,
    /// Diagnose configuration and environment
    Doctor {
        /// Print full details (config sources, plugin search, env)
        #[arg(long, default_value_t = false)]
        full: bool,
    },
}

// moved to menu.rs

#[derive(Debug)]
struct OutputView {
    title: String,
    lines: Vec<String>,
    scroll_y: u16,
    running: bool,
    auto_scroll: bool,
    viewport_height: u16,
    pending_line: Option<String>,
    input_buffer: String,
    exit_status: Option<i32>,
    started_at: Option<Instant>,
    ended_at: Option<Instant>,
    md_content: Option<String>,
    file_path: Option<PathBuf>,
    md_links: Vec<String>,
    md_footnote_start: Option<u16>,
    wrap_enabled: bool,
    render_markdown: bool,
    spinner_idx: usize,
    scroller: Scroller,
    // Terminal emulator (optional when running PTY TUIs)
    term: Option<term::Emulator>,
}

impl OutputView {
    fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            lines: Vec::new(),
            scroll_y: 0,
            running: false,
            auto_scroll: true,
            viewport_height: 0,
            pending_line: None,
            input_buffer: String::new(),
            exit_status: None,
            started_at: None,
            ended_at: None,
            md_content: None,
            file_path: None,
            md_links: Vec::new(),
            md_footnote_start: None,
            wrap_enabled: true,
            render_markdown: true,
            spinner_idx: 0,
            scroller: Scroller::new(),
            term: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Scroller {
    scroll_y: u16,
    viewport: u16,
    total: u16,
    auto: bool,
}

impl Scroller {
    fn new() -> Self {
        Self {
            scroll_y: 0,
            viewport: 1,
            total: 0,
            auto: true,
        }
    }
    fn set_viewport(&mut self, h: u16) {
        self.viewport = h.max(1);
        self.clamp();
    }
    fn set_total(&mut self, total: u16) {
        self.total = total;
        self.clamp();
    }
    fn set_auto(&mut self, auto: bool) {
        self.auto = auto;
        if auto {
            self.end();
        }
    }
    fn max_scroll(&self) -> u16 {
        self.total.saturating_sub(self.viewport)
    }
    fn clamp(&mut self) {
        let max = self.max_scroll();
        if self.scroll_y > max {
            self.scroll_y = max;
        }
    }
    fn end(&mut self) {
        self.scroll_y = self.max_scroll();
    }
    fn home(&mut self) {
        self.scroll_y = 0;
        self.auto = false;
    }
    fn line_up(&mut self) {
        self.scroll_y = self.scroll_y.saturating_sub(1);
        self.auto = false;
    }
    fn line_down(&mut self) {
        self.scroll_y = (self.scroll_y.saturating_add(1)).min(self.max_scroll());
        self.auto = false;
    }
    #[allow(dead_code)]
    fn page_up(&mut self) {
        let step = self.viewport.saturating_sub(1).max(1);
        self.scroll_y = self.scroll_y.saturating_sub(step);
        self.auto = false;
    }
    #[allow(dead_code)]
    fn page_down(&mut self) {
        let step = self.viewport.saturating_sub(1).max(1);
        self.scroll_y = (self.scroll_y.saturating_add(step)).min(self.max_scroll());
        self.auto = false;
    }
}

// moved to menu.rs

// OutputMsg type is provided by exec.rs

#[derive(Debug)]
enum Screen {
    Menu(MenuState),
    Output(OutputView),
    Config(ConfigState),
    Form(screens::form::FormState),
}

use crate::config::ConfigState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    KillProcess { yes_selected: bool },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct StatusConfig {
    text: Option<String>,
    command: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct AppConfig {
    #[serde(default)]
    status: Option<StatusConfig>,
    #[serde(default)]
    allow_project_override: bool,
    #[serde(default)]
    motd_wrap: Option<bool>,
    #[serde(default)]
    motd_color: Option<String>,
    #[serde(default)]
    markdown_enabled: Option<bool>,
    #[serde(default)]
    output_dim: Option<bool>,
    #[serde(default)]
    theme: Option<String>, // "dark" or "light" or custom name
    #[serde(default)]
    theme_file: Option<String>, // path to external YAML .dx-theme
    #[serde(default)]
    theme_overrides: Option<HashMap<String, String>>, // token->color overrides
    #[serde(default)]
    theme_dir: Option<String>, // directory with *.dx-theme files
    #[serde(default)]
    telemetry: Option<TelemetryConfig>,
    #[serde(default)]
    update: Option<UpdateConfig>,
    #[serde(default)]
    asciinema: Option<AsciinemaConfig>,
    #[serde(default)]
    show_fps: Option<bool>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
struct TelemetryConfig {
    #[serde(default)]
    enabled: bool,
    endpoint: Option<String>,
}

#[derive(Debug, Serialize)]
struct TelemetryPayload {
    title: String,
    exit_code: i32,
    lines: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct UpdateConfig {
    #[serde(default)]
    on_start: bool,
    #[serde(default = "default_build_cmd")]
    build_cmd: String,
    relaunch_path: Option<String>,
    #[serde(default = "default_preserve_args")]
    preserve_args: bool,
}

fn default_build_cmd() -> String {
    "cargo build --release".to_string()
}
fn default_preserve_args() -> bool {
    true
}

use crate::asciinema::{
    AsciinemaConfig, build_asciinema_cmd, build_asciinema_stream_cmd, default_stream_mode,
    first_url_in, generate_asciinema_filename, shell_quote,
};

struct App {
    screen: Screen,
    screen_stack: Vec<Screen>,
    child: Option<Child>,
    child_stdin: Option<std::process::ChildStdin>,
    rx: Option<tokio::sync::mpsc::Receiver<OutputMsg>>,
    menu_path: Option<PathBuf>,
    confirm: Option<Confirm>,
    needs_clear: bool,
    motd_lines: Vec<String>,
    last_content_area: Option<Rect>,
    // Status line
    status_text: Option<String>,
    status_rx: Option<tokio::sync::mpsc::Receiver<String>>,
    #[allow(dead_code)]
    status_child: Option<Child>,
    // MOTD options
    motd_wrap: bool,
    motd_force_raw: bool,
    motd_color: Option<Color>,
    // Markdown global toggle
    markdown_enabled: bool,
    // Menu command buffer (for :q)
    menu_cmd: Option<String>,
    // Output dimming
    output_dim: bool,
    // Theme (true=dark, false=light)
    #[allow(dead_code)]
    theme_dark: bool,
    // Theme tokens for rendering
    theme: theme::ThemeTokens,
    // PTY integration
    pty_child: Option<Box<dyn PtyChild + Send>>, // PTY child handle
    pty_master: Option<Box<dyn MasterPty + Send>>, // for resizing
    pty_writer: Option<Box<dyn Write + Send>>,   // to forward keys
    // Selection mode (release mouse to terminal for text selection)
    selection_mode: bool,
    mouse_captured: bool,
    // Start a command immediately after TUI initializes (alias support)
    startup_cmd: Option<(String, String, bool)>, // (title, cmd, external)
    // Telemetry configuration
    telemetry: Option<TelemetryConfig>,
    // Asciinema configuration
    asciinema: Option<AsciinemaConfig>,
    // Live streaming toggle from CLI or config
    asciinema_live: bool,
    // Badge shown in status bar if running under asciinema (record/stream)
    asciinema_badge: Option<String>,
    // Whether dx should auto-open URLs (e.g., asciinema stream) in a browser
    auto_open: bool,
    // Blink state for animating status icons (e.g., live stream recorder dot)
    blink_on: bool,
    blink_tick: u8,
    // Mouse double-click tracking for menu activation
    last_click_at: Option<Instant>,
    last_click_index: Option<usize>,
    // Plugin overlay runtimes (CPU overlay and others via plugins)
    plugin_overlays: Vec<plugin::OverlayRuntime>,
    // FPS counter
    fps_frames: u32,
    fps_last_instant: Instant,
    fps: f32,
    show_fps: bool,
}

const PAD_X: u16 = 2; // left/right padding inside boxes
const PAD_Y: u16 = 1; // top/bottom padding inside boxes

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap_or_else(|_| EnvFilter::new("info"));
    let is_json = matches!(
        std::env::var("DX_LOG_FORMAT").ok().as_deref(),
        Some("json") | Some("JSON")
    );
    if is_json {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    }
}

#[tokio::main]
async fn main() {
    init_tracing();
    let code = match cli_main() {
        Ok(code) => code,
        Err(e) => {
            error!(error = %e, "dx error");
            1
        }
    };
    std::process::exit(code);
}

fn cli_main() -> Result<i32> {
    let cli = Cli::parse();

    // Fast-path subcommands that bypass TUI and exec asciinema directly
    if let Some(cmd) = &cli.cmd {
        match cmd {
            DxCmd::Stream { stream_id } => {
                // Equivalent to: asciinema stream -r [STREAM_ID]
                let mut parts: Vec<String> = vec![
                    "asciinema".to_string(),
                    "stream".to_string(),
                    "-r".to_string(),
                ];
                if let Some(id) = stream_id.as_ref() {
                    parts.push(id.clone());
                }
                let joined: Vec<String> = parts
                    .into_iter()
                    .map(|p| crate::asciinema::shell_quote(&p))
                    .collect();
                let cmdline = joined.join(" ");
                let status = Command::new("sh")
                    .arg("-lc")
                    .arg(cmdline)
                    .current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
                    .status();
                match status {
                    Ok(st) => {
                        return Ok(st.code().unwrap_or(0));
                    }
                    Err(e) => {
                        error!(error = %e, "dx: failed to start stream");
                        return Ok(1);
                    }
                }
            }
            DxCmd::Record => {
                // Equivalent to: asciinema record (start interactive shell)
                let cmdline = "asciinema record";
                let status = Command::new("sh")
                    .arg("-lc")
                    .arg(cmdline)
                    .current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
                    .status();
                match status {
                    Ok(st) => {
                        return Ok(st.code().unwrap_or(0));
                    }
                    Err(e) => {
                        error!(error = %e, "dx: failed to start recording");
                        return Ok(1);
                    }
                }
            }
            DxCmd::Doctor { full } => {
                println!("DX Doctor\n=========");
                // Basic config validation in CWD
                let _path = PathBuf::from("config.toml");
                println!("\nConfig:");
                // Detect multiple local configs (toml/yaml/json)
                let local_cfg_candidates =
                    ["config.toml", "config.yaml", "config.yml", "config.json"];
                let mut local_found: Vec<&str> = Vec::new();
                let project_root = crate::exec::find_project_root();
                for c in &local_cfg_candidates {
                    if project_root.join(c).exists() {
                        local_found.push(c);
                    }
                }
                if local_found.is_empty() {
                    println!("  \x1b[90m[default]\x1b[0m local: (none)");
                } else {
                    println!("  Local candidates:");
                    for c in &local_found {
                        println!("    - {}", c);
                    }
                    // Current loader uses only config.toml; validate it when present
                    if project_root.join("config.toml").exists() {
                        match fs::read_to_string(project_root.join("config.toml"))
                            .ok()
                            .and_then(|s| toml::from_str::<AppConfig>(&s).ok())
                        {
                            Some(_) => {
                                println!("  \x1b[32m[project]\x1b[0m using: config.toml (valid)")
                            }
                            None => {
                                println!(
                                    "  \x1b[32m[project]\x1b[0m using: config.toml \x1b[31m(invalid)\x1b[0m"
                                );
                                return Ok(1);
                            }
                        }
                    } else {
                        println!(
                            "  \x1b[33m[warn]\x1b[0m no config.toml found; other formats are currently ignored by dx runtime"
                        );
                    }
                    if local_found.len() > 1 {
                        println!(
                            "  \x1b[33m[warn]\x1b[0m multiple local config files detected; prefer a single config.toml to avoid confusion"
                        );
                    }
                }
                // Detect menu files (dx.* / menu.*) and show selection
                println!("\nMenu files:");
                let menu_candidates = [
                    "dx.yaml",
                    "dx.yml",
                    "dx.toml",
                    "dx.json",
                    "menu.yaml",
                    "menu.yml",
                    "menu.toml",
                    "menu.json",
                ];
                let mut menu_found: Vec<&str> = Vec::new();
                for m in &menu_candidates {
                    if project_root.join(m).exists() {
                        menu_found.push(*m);
                    }
                }
                if menu_found.is_empty() {
                    println!(
                        "  \x1b[34m[note]\x1b[0m no dx.* or menu.* found in current directory (dx will still run, but create dx.yaml for best experience)"
                    );
                } else {
                    println!("  Found:");
                    for m in &menu_found {
                        println!("    - {}", m);
                    }
                    let mut selected: Option<&str> = None;
                    for c in &menu_candidates {
                        if project_root.join(c).exists() {
                            selected = Some(*c);
                            break;
                        }
                    }
                    if let Some(sel) = selected {
                        println!("  Will use: \x1b[32m{}\x1b[0m (by precedence)", sel);
                    }
                    if menu_found.len() > 1 {
                        println!(
                            "  \x1b[33m[warn]\x1b[0m multiple menu files; prefer a single dx.yaml"
                        );
                    }
                }
                // Detect multiple global configs in ~/.dx
                let home = std::env::var("HOME").ok();
                if let Some(h) = &home {
                    let dxdir = PathBuf::from(h).join(".dx");
                    let global_candidates =
                        ["config.toml", "config.yaml", "config.yml", "config.json"];
                    let mut gfound: Vec<String> = Vec::new();
                    for g in &global_candidates {
                        let p = dxdir.join(g);
                        if p.exists() {
                            gfound.push(p.to_string_lossy().to_string());
                        }
                    }
                    if !gfound.is_empty() {
                        println!("  Global candidates:");
                        for p in &gfound {
                            println!("    - {}", p);
                        }
                        let gtoml = dxdir.join("config.toml");
                        if gtoml.exists() {
                            println!("  \x1b[36m[home]\x1b[0m using: {}", gtoml.to_string_lossy());
                        }
                        if gfound.len() > 1 {
                            println!(
                                "  \x1b[33m[warn]\x1b[0m multiple global config files detected; prefer a single ~/.dx/config.toml"
                            );
                        }
                    }
                }

                if *full {
                    // Show plugin search paths and discovered candidates
                    println!("\nPlugins (search paths):");
                    let home = std::env::var("HOME").ok();
                    let exe_dir = std::env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
                    let mut entries: Vec<(String, String)> = Vec::new();
                    if let Ok(envp) = std::env::var("DX_PLUGIN_CPU") {
                        entries.push(("DX_PLUGIN_CPU".into(), envp));
                    }
                    entries.push((
                        "build debug".into(),
                        PathBuf::from("target")
                            .join("debug")
                            .join("libdx_overlay_cpu.dylib")
                            .to_string_lossy()
                            .to_string(),
                    ));
                    entries.push((
                        "build release".into(),
                        PathBuf::from("target")
                            .join("release")
                            .join("libdx_overlay_cpu.dylib")
                            .to_string_lossy()
                            .to_string(),
                    ));
                    if let Some(d) = &exe_dir {
                        entries.push((
                            "exe plugins dylib".into(),
                            d.join("plugins")
                                .join("libdx_overlay_cpu.dylib")
                                .to_string_lossy()
                                .to_string(),
                        ));
                        entries.push((
                            "exe plugins dxplugin".into(),
                            d.join("plugins")
                                .join("libdx_overlay_cpu.dxplugin")
                                .to_string_lossy()
                                .to_string(),
                        ));
                    }
                    if let Some(h) = &home {
                        entries.push((
                            "user share dylib".into(),
                            PathBuf::from(h)
                                .join(".local/share/dx/plugins/libdx_overlay_cpu.dylib")
                                .to_string_lossy()
                                .to_string(),
                        ));
                        entries.push((
                            "user share dxplugin".into(),
                            PathBuf::from(h)
                                .join(".local/share/dx/plugins/libdx_overlay_cpu.dxplugin")
                                .to_string_lossy()
                                .to_string(),
                        ));
                        entries.push((
                            "legacy dylib".into(),
                            PathBuf::from(h)
                                .join(".dx/plugins/libdx_overlay_cpu.dylib")
                                .to_string_lossy()
                                .to_string(),
                        ));
                        entries.push((
                            "legacy dxplugin".into(),
                            PathBuf::from(h)
                                .join(".dx/plugins/libdx_overlay_cpu.dxplugin")
                                .to_string_lossy()
                                .to_string(),
                        ));
                    }
                    let name_w = entries.iter().map(|(n, _)| n.len()).max().unwrap_or(10);
                    for (name, p) in entries.into_iter() {
                        let exists = Path::new(&p).exists();
                        let icon = if exists { "[✔]" } else { "[ ]" };
                        println!("  {} {:name_w$}  {}", icon, name, p, name_w = name_w);
                    }

                    // Effective configuration overview (grouped)
                    println!("\nEffective config (source):");
                    let read_cfg = |p: &Path| {
                        fs::read_to_string(p)
                            .ok()
                            .and_then(|s| toml::from_str::<AppConfig>(&s).ok())
                    };
                    let global_path = home
                        .as_ref()
                        .map(|h| PathBuf::from(h).join(".dx/config.toml"));
                    let global_cfg = global_path.as_ref().and_then(|p| read_cfg(p));
                    let local_path = PathBuf::from("config.toml");
                    let local_cfg = read_cfg(&local_path);

                    let print_bool_src =
                        |label: &str, gv: Option<bool>, lv: Option<bool>, dv: Option<bool>| {
                            let color_src = |src: &str| -> String {
                                match src {
                                    "project" => "\x1b[32m[project]\x1b[0m".to_string(),
                                    "home" => "\x1b[36m[home]\x1b[0m".to_string(),
                                    _ => "\x1b[90m[default]\x1b[0m".to_string(),
                                }
                            };
                            let (src, v) = match (lv, gv, dv) {
                                (Some(v), _, _) => ("project", Some(v)),
                                (None, Some(v), _) => ("home", Some(v)),
                                (None, None, Some(v)) => ("default", Some(v)),
                                _ => ("default", None),
                            };
                            let tag = color_src(src);
                            match v {
                                Some(b) => println!("  {} {}: {}", tag, label, b),
                                None => println!("  {} {}: (unset)", tag, label),
                            }
                        };
                    let print_str_src =
                        |label: &str, gv: Option<String>, lv: Option<String>, dv: Option<&str>| {
                            let color_src = |src: &str| -> String {
                                match src {
                                    "project" => "\x1b[32m[project]\x1b[0m".to_string(),
                                    "home" => "\x1b[36m[home]\x1b[0m".to_string(),
                                    _ => "\x1b[90m[default]\x1b[0m".to_string(),
                                }
                            };
                            let (src, v) = match (lv, gv, dv) {
                                (Some(v), _, _) => ("project", Some(v)),
                                (None, Some(v), _) => ("home", Some(v)),
                                (None, None, Some(v)) => ("default", Some(v.to_string())),
                                _ => ("default", None),
                            };
                            let tag = color_src(src);
                            match v {
                                Some(s) => println!("  {} {}: '{}'", tag, label, s),
                                None => println!("  {} {}: (unset)", tag, label),
                            }
                        };

                    // General
                    println!("\n  General:");
                    print_bool_src(
                        "markdown_enabled",
                        global_cfg.as_ref().and_then(|c| c.markdown_enabled),
                        local_cfg.as_ref().and_then(|c| c.markdown_enabled),
                        None,
                    );
                    print_bool_src(
                        "output_dim",
                        global_cfg.as_ref().and_then(|c| c.output_dim),
                        local_cfg.as_ref().and_then(|c| c.output_dim),
                        None,
                    );
                    print_bool_src(
                        "show_fps",
                        global_cfg.as_ref().and_then(|c| c.show_fps),
                        local_cfg.as_ref().and_then(|c| c.show_fps),
                        None,
                    );
                    let allow_local = local_cfg.as_ref().map(|c| c.allow_project_override);
                    let allow_global = global_cfg.as_ref().map(|c| c.allow_project_override);
                    print_bool_src(
                        "allow_project_override",
                        allow_global,
                        allow_local,
                        Some(true),
                    );
                    print_str_src(
                        "theme",
                        global_cfg.as_ref().and_then(|c| c.theme.clone()),
                        local_cfg.as_ref().and_then(|c| c.theme.clone()),
                        None,
                    );
                    print_str_src(
                        "theme_file",
                        global_cfg.as_ref().and_then(|c| c.theme_file.clone()),
                        local_cfg.as_ref().and_then(|c| c.theme_file.clone()),
                        None,
                    );
                    print_str_src(
                        "theme_dir",
                        global_cfg.as_ref().and_then(|c| c.theme_dir.clone()),
                        local_cfg.as_ref().and_then(|c| c.theme_dir.clone()),
                        None,
                    );
                    print_str_src(
                        "motd_color",
                        global_cfg.as_ref().and_then(|c| c.motd_color.clone()),
                        local_cfg.as_ref().and_then(|c| c.motd_color.clone()),
                        None,
                    );
                    print_bool_src(
                        "motd_wrap",
                        global_cfg.as_ref().and_then(|c| c.motd_wrap),
                        local_cfg.as_ref().and_then(|c| c.motd_wrap),
                        None,
                    );

                    // Telemetry
                    println!("\n  Telemetry:");
                    let tel_g = global_cfg
                        .as_ref()
                        .and_then(|c| c.telemetry.as_ref())
                        .map(|t| t.enabled);
                    let tel_l = local_cfg
                        .as_ref()
                        .and_then(|c| c.telemetry.as_ref())
                        .map(|t| t.enabled);
                    print_bool_src("telemetry.enabled", tel_g, tel_l, Some(false));
                    let tel_ep_g = global_cfg
                        .as_ref()
                        .and_then(|c| c.telemetry.as_ref())
                        .and_then(|t| t.endpoint.clone());
                    let tel_ep_l = local_cfg
                        .as_ref()
                        .and_then(|c| c.telemetry.as_ref())
                        .and_then(|t| t.endpoint.clone());
                    print_str_src("telemetry.endpoint", tel_ep_g, tel_ep_l, None);

                    // asciinema.* with defaults
                    let asc_g_ref = global_cfg.as_ref().and_then(|c| c.asciinema.as_ref());
                    let asc_l_ref = local_cfg.as_ref().and_then(|c| c.asciinema.as_ref());
                    let asc_src = if asc_l_ref.is_some() {
                        "project"
                    } else if asc_g_ref.is_some() {
                        "home"
                    } else {
                        "default"
                    };
                    let asc =
                        asc_l_ref
                            .cloned()
                            .or(asc_g_ref.cloned())
                            .unwrap_or(AsciinemaConfig {
                                enabled: false,
                                external: false,
                                on_relaunch: false,
                                dir: None,
                                file_prefix: None,
                                title: None,
                                quiet: false,
                                overwrite: false,
                                stream: false,
                                stream_mode: default_stream_mode(),
                                local_addr: None,
                                remote: None,
                            });
                    println!("\n  Asciinema:");
                    let asc_tag = match asc_src {
                        "project" => "\x1b[32m[project]\x1b[0m",
                        "home" => "\x1b[36m[home]\x1b[0m",
                        _ => "\x1b[90m[default]\x1b[0m",
                    };
                    println!("    {} enabled: {}", asc_tag, asc.enabled);
                    println!("    {} external: {}", asc_tag, asc.external);
                    println!("    {} on_relaunch: {}", asc_tag, asc.on_relaunch);
                    let pstr = |k: &str, v: Option<&str>| match v {
                        Some(s) => println!("    {} {}: '{}'", asc_tag, k, s),
                        None => println!("    {} {}: (unset)", asc_tag, k),
                    };
                    pstr("dir", asc.dir.as_deref());
                    pstr("file_prefix", asc.file_prefix.as_deref());
                    pstr("title", asc.title.as_deref());
                    println!("    {} quiet: {}", asc_tag, asc.quiet);
                    println!("    {} overwrite: {}", asc_tag, asc.overwrite);
                    println!("    {} stream: {}", asc_tag, asc.stream);
                    println!("    {} stream_mode: '{}'", asc_tag, asc.stream_mode);
                    pstr("local_addr", asc.local_addr.as_deref());
                    pstr("remote", asc.remote.as_deref());

                    // update.* with defaults
                    let upd_g = global_cfg.as_ref().and_then(|c| c.update.as_ref());
                    let upd_l = local_cfg.as_ref().and_then(|c| c.update.as_ref());
                    let (upd_src, upd) = if let Some(u) = upd_l {
                        ("project", u)
                    } else if let Some(u) = upd_g {
                        ("home", u)
                    } else {
                        (
                            "default",
                            &UpdateConfig {
                                on_start: false,
                                build_cmd: default_build_cmd(),
                                relaunch_path: None,
                                preserve_args: default_preserve_args(),
                            },
                        )
                    };
                    println!("\n  Update:");
                    let upd_tag = match upd_src {
                        "project" => "\x1b[32m[project]\x1b[0m",
                        "home" => "\x1b[36m[home]\x1b[0m",
                        _ => "\x1b[90m[default]\x1b[0m",
                    };
                    println!("    {} on_start: {}", upd_tag, upd.on_start);
                    println!("    {} build_cmd: '{}'", upd_tag, upd.build_cmd);
                    println!("    {} preserve_args: {}", upd_tag, upd.preserve_args);
                    match upd.relaunch_path.as_deref() {
                        Some(p) => println!("    {} relaunch_path: '{}'", upd_tag, p),
                        None => println!("    {} relaunch_path: (unset)", upd_tag),
                    }

                    // Status
                    println!("\n  Status:");
                    let st_txt_l = local_cfg
                        .as_ref()
                        .and_then(|c| c.status.as_ref())
                        .and_then(|s| s.text.clone());
                    let st_txt_g = global_cfg
                        .as_ref()
                        .and_then(|c| c.status.as_ref())
                        .and_then(|s| s.text.clone());
                    let (src, val) = if let Some(v) = st_txt_l {
                        ("project", Some(v))
                    } else if let Some(v) = st_txt_g {
                        ("home", Some(v))
                    } else {
                        ("default", None)
                    };
                    let st_tag = match src {
                        "project" => "\x1b[32m[project]\x1b[0m",
                        "home" => "\x1b[36m[home]\x1b[0m",
                        _ => "\x1b[90m[default]\x1b[0m",
                    };
                    match val {
                        Some(s) => println!("    {} text: '{}'", st_tag, s),
                        None => println!("    {} text: (unset)", st_tag),
                    }
                }
                return Ok(0);
            }
        }
    }
    // Lightweight: auto-relaunch under asciinema stream on start when --live or config requests it
    if !cli.console && std::env::var("DX_RELAUNCHED").ok().as_deref() != Some("1") {
        // minimal config read to detect asciinema preferences
        fn read_app_config(path: &Path) -> Option<AppConfig> {
            fs::read_to_string(path)
                .ok()
                .and_then(|s| toml::from_str::<AppConfig>(&s).ok())
        }
        let home = std::env::var("HOME").ok();
        let global_cfg = home
            .map(|h| PathBuf::from(h).join(".dx").join("config.toml"))
            .and_then(|p| read_app_config(&p));
        let local_cfg = read_app_config(Path::new("config.toml"));
        let asciinema_cfg = local_cfg
            .as_ref()
            .and_then(|c| c.asciinema.clone())
            .or_else(|| global_cfg.as_ref().and_then(|c| c.asciinema.clone()));
        let want_stream = if let Some(ac) = &asciinema_cfg {
            ac.enabled && ac.stream
        } else {
            false
        };
        if cli.live || want_stream {
            if let Some(ac) = &asciinema_cfg {
                // Rebuild command line for current dx with preserved args
                let exe = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.into_os_string().into_string().ok())
                    .unwrap_or_else(|| "dx".to_string());
                let mut args_vec: Vec<String> = Vec::new();
                let mut args = std::env::args_os();
                let _ = args.next();
                for a in args {
                    args_vec.push(a.to_string_lossy().to_string());
                }
                let inner = if cli.console {
                    // console mode: start login shell directly (no TUI)
                    std::env::var("SHELL").unwrap_or_else(|_| "sh".to_string())
                } else if args_vec.is_empty() {
                    exe
                } else {
                    format!("{} {}", exe, args_vec.join(" "))
                };

                let cmdline = build_asciinema_stream_cmd(ac, &inner);
                let mut cmd = Command::new("sh");
                cmd.arg("-lc").arg(cmdline);
                cmd.current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
                cmd.env("DX_RELAUNCHED", "1");
                cmd.env("DX_ASCIINEMA", "stream");
                if ac.stream_mode.eq_ignore_ascii_case("remote") {
                    cmd.env("DX_ASC_MODE", "remote");
                    if let Some(r) = &ac.remote {
                        cmd.env("DX_ASC_REMOTE", r);
                    }
                } else {
                    cmd.env("DX_ASC_MODE", "local");
                    if let Some(addr) = &ac.local_addr {
                        cmd.env("DX_ASC_LOCAL_ADDR", addr);
                    }
                }
                // Pipe asciinema's outputs so we can sniff the live URL
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());
                match cmd.spawn() {
                    Ok(mut child) => {
                        if cli.console {
                            // console mode: just wait for child and exit with its code
                            let st = child.wait();
                            match st {
                                Ok(st) => {
                                    return Ok(st.code().unwrap_or(0));
                                }
                                Err(e) => {
                                    error!(error = %e, "dx: stream failed");
                                    return Ok(1);
                                }
                            }
                        }
                        // Spawn readers to capture the live stream URL when asciinema prints it
                        if let Some(stdout) = child.stdout.take() {
                            let opened = Arc::new(AtomicBool::new(false));
                            let opened2 = opened.clone();
                            let auto_open = !cli.disable_auto_open;
                            std::thread::spawn(move || {
                                let reader = BufReader::new(stdout);
                                for line in reader.lines().flatten() {
                                    if opened2.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    if let Some(url) =
                                        line.trim().strip_prefix("Live streaming at ")
                                    {
                                        if auto_open {
                                            let _ = open_default_browser(url);
                                        }
                                        opened2.store(true, Ordering::Relaxed);
                                        break;
                                    } else if let Some(url) = first_url_in(&line) {
                                        if auto_open {
                                            let _ = open_default_browser(&url);
                                        }
                                        opened2.store(true, Ordering::Relaxed);
                                        break;
                                    }
                                }
                            });
                        }
                        if let Some(stderr) = child.stderr.take() {
                            let opened = Arc::new(AtomicBool::new(false));
                            let opened2 = opened.clone();
                            let auto_open = !cli.disable_auto_open;
                            std::thread::spawn(move || {
                                let reader = BufReader::new(stderr);
                                for line in reader.lines().flatten() {
                                    if opened2.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    if let Some(url) =
                                        line.trim().strip_prefix("Live streaming at ")
                                    {
                                        if auto_open {
                                            let _ = open_default_browser(url);
                                        }
                                        opened2.store(true, Ordering::Relaxed);
                                        break;
                                    } else if let Some(url) = first_url_in(&line) {
                                        if auto_open {
                                            let _ = open_default_browser(&url);
                                        }
                                        opened2.store(true, Ordering::Relaxed);
                                        break;
                                    }
                                }
                            });
                        }
                        let _ = child.wait();
                        return Ok(0);
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to relaunch under asciinema stream");
                    }
                }
            }
        }
    }
    // Removed auto build-and-relaunch on start; explicit scripts/menu handle builds

    // Autodetect menu.toml in CWD if present (unless --menu or explicit target used)
    let _default_menu_path = PathBuf::from("menu.toml");
    // Optionally load a menu upfront to resolve aliases or to show the menu
    let mut initial_menu: Option<MenuState> = None;
    let mut menu_path: Option<PathBuf> = None;
    let mut startup_cmd: Option<(String, String, bool)> = None;
    if let Some(p) = cli.menu.as_deref() {
        let mut m = load_menu(p)?;
        prepend_readme_item(&mut m);
        crate::menu::append_dx_menu(&mut m);
        initial_menu = Some(m);
        menu_path = Some(p.to_path_buf());
    } else {
        // Try common menu filenames in order (prefer YAML, then TOML, then JSON; dx.* before menu.*)
        let candidates = [
            "dx.yaml",
            "dx.yml",
            "dx.toml",
            "dx.json",
            "DX.yaml",
            "DX.yml",
            "DX.toml",
            "DX.json",
            "menu.yaml",
            "menu.yml",
            "menu.toml",
            "menu.json",
            "Menu.yaml",
            "Menu.yml",
            "Menu.toml",
            "Menu.json",
        ];
        let mut found: Option<PathBuf> = None;
        let project_root = crate::exec::find_project_root();
        for c in candidates.iter() {
            let p = project_root.join(c);
            if p.exists() {
                found = Some(p);
                break;
            }
        }
        if let Some(p) = found {
            let mut m = load_menu(&p)?;
            prepend_readme_item(&mut m);
            crate::menu::append_dx_menu(&mut m);
            initial_menu = Some(m);
            menu_path = Some(p);
        }
    }

    // Fast path: if user called dx <alias> and it resolves to a leaf command,
    // run it directly in the user's shell (no TUI) and exit.
    if cli.llm {
        println!(
            "Use dx non-interactively. No TUI.\n- dx aliases  # list alias table\n- dx <alias>  # run leaf cmd; inherits stdio; returns exit code\n- dx <alias> --record  # record run to .cast (respects config)\n- dx <path>   # print file contents to stdout\nRules: do not expect prompts; avoid TUI; pass exact args; check exit codes."
        );
        return Ok(0);
    }
    if let Some(t) = cli.target.as_ref() {
        if t != "aliases" {
            if let Some(menu) = &initial_menu {
                if let Some(item) = find_item_by_alias(&menu.items, t) {
                    if item.items.is_empty() {
                        if let Some(cmd) = &item.cmd {
                            // Build command with arguments
                            let full_cmd = if !cli.args.is_empty() {
                                format!("{} {}", cmd, cli.args.join(" "))
                            } else {
                                cmd.clone()
                            };

                            // Execute command directly; optionally wrap with asciinema record
                            let (status, record_path) = if cli.record {
                                // Try to read recording preferences from config for dir/prefix/quiet/title
                                let home = std::env::var("HOME").ok();
                                let global_cfg = home
                                    .map(|h| PathBuf::from(h).join(".dx").join("config.toml"))
                                    .and_then(|p| read_app_config_file(&p));
                                let local_cfg = read_app_config_file(Path::new("config.toml"));
                                let acfg = local_cfg
                                    .as_ref()
                                    .and_then(|c| c.asciinema.clone())
                                    .or_else(|| {
                                        global_cfg.as_ref().and_then(|c| c.asciinema.clone())
                                    });

                                let dir = acfg
                                    .as_ref()
                                    .and_then(|a| a.dir.clone())
                                    .unwrap_or_else(|| ".".to_string());
                                let prefix = acfg
                                    .as_ref()
                                    .and_then(|a| a.file_prefix.clone())
                                    .unwrap_or_else(|| "dx".to_string());
                                let title = acfg.as_ref().and_then(|a| a.title.clone());
                                let quiet = acfg.as_ref().map(|a| a.quiet).unwrap_or(true);

                                let _ = std::fs::create_dir_all(&dir);
                                let ts = SystemTime::now()
                                    .duration_since(UNIX_EPOCH)
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                let file = format!("{}/{}-{}.cast", dir, prefix, ts);

                                let mut parts: Vec<String> =
                                    vec!["asciinema".to_string(), "record".to_string()];
                                if quiet {
                                    parts.push("-q".to_string());
                                }
                                if let Some(tit) = title {
                                    parts.push("-t".to_string());
                                    parts.push(tit);
                                }
                                parts.push(file.clone());
                                parts.push("-c".to_string());
                                parts.push(full_cmd.clone());
                                let joined: Vec<String> =
                                    parts.into_iter().map(|p| shell_quote(&p)).collect();
                                let cmdline = joined.join(" ");
                                let mut c = Command::new("sh");
                                c.arg("-lc").arg(cmdline);
                                c.current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
                                c.env("DX_ASCIINEMA", "record");
                                c.env("DX_ASC_FILE", file.clone());
                                (c.status(), Some(file))
                            } else {
                                // Execute command attached to current TTY; inherit stdio
                                (Command::new("sh")
                    .arg("-lc")
                    .arg(&full_cmd)
                    .current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
                    .status(), None)
                            };
                            match status {
                                Ok(st) => {
                                    if let Some(path) = record_path {
                                        println!("\n\x1b[92mRecording saved:\x1b[0m {}", path);
                                    }
                                    return Ok(st.code().unwrap_or(0));
                                }
                                Err(e) => {
                                    error!(target: "dx", alias = %t, error = %e, "failed to run alias");
                                    return Ok(1);
                                }
                            }
                        } else if let Some(file) = &item.file {
                            match fs::read_to_string(file) {
                                Ok(s) => {
                                    print!("{}", s);
                                    return Ok(0);
                                }
                                Err(e) => {
                                    error!(file = %file, error = %e, "dx: cannot read file");
                                    return Ok(1);
                                }
                            }
                        }
                    }
                }
            }
            // If not an alias but is a file path, print it to stdout and exit
            let p = PathBuf::from(t);
            if p.exists() {
                match fs::read(&p) {
                    Ok(bytes) => {
                        use std::io::Write as _;
                        let _ = std::io::stdout().write_all(&bytes);
                        return Ok(0);
                    }
                    Err(e) => {
                        error!(path = %p.display(), error = %e, "dx: cannot read path");
                        return Ok(1);
                    }
                }
            } else {
                error!(target: "dx", target_arg = %t, "path or alias not found");
                return Ok(1);
            }
        }
    }

    let (screen, menu_path) = if let Some(t) = cli.target {
        if t == "aliases" {
            if let Some(menu) = &initial_menu {
                let aliases = collect_aliases(&menu.items);
                // Prepare rows: (alias, name, action, details)
                let mut rows: Vec<(String, String, String, String)> = Vec::new();
                for (a, name, cmd, file) in &aliases {
                    let (action, details) = if let Some(c) = cmd {
                        ("cmd".to_string(), c.clone())
                    } else if let Some(f) = file {
                        ("file".to_string(), f.clone())
                    } else {
                        ("".to_string(), String::new())
                    };
                    rows.push((a.clone(), name.clone(), action, details));
                }
                // Compute column widths with caps
                let mut alias_w: usize = 5;
                let mut name_w: usize = 4;
                let action_w: usize = 4; // cmd/file
                for (a, n, _, _) in &rows {
                    alias_w = alias_w.max(a.len());
                    name_w = name_w.max(n.len());
                }
                alias_w = alias_w.min(24);
                name_w = name_w.min(36);
                // Header - handle broken pipe gracefully
                use std::io::Write;
                let mut stdout = io::stdout();
                let header = format!(
                    "{:<alias_w$}  {:<name_w$}  {:<action_w$}  {}\n",
                    "ALIAS",
                    "NAME",
                    "TYPE",
                    "DETAILS",
                    alias_w = alias_w,
                    name_w = name_w,
                    action_w = action_w
                );
                if stdout.write_all(header.as_bytes()).is_err() {
                    return Ok(0);
                }

                let separator = format!(
                    "{:-<alias_w$}  {:-<name_w$}  {:-<action_w$}  {:-<50}\n",
                    "",
                    "",
                    "",
                    "",
                    alias_w = alias_w,
                    name_w = name_w,
                    action_w = action_w
                );
                if stdout.write_all(separator.as_bytes()).is_err() {
                    return Ok(0);
                }

                // Data rows - handle broken pipe gracefully
                let max_details: usize = 100;
                for (a, n, act, det) in rows {
                    let d = if det.len() > max_details {
                        let mut s = det.chars().take(max_details - 1).collect::<String>();
                        s.push('…');
                        s
                    } else {
                        det
                    };
                    let row = format!(
                        "{:<alias_w$}  {:<name_w$}  {:<action_w$}  {}\n",
                        a,
                        n,
                        act,
                        d,
                        alias_w = alias_w,
                        name_w = name_w,
                        action_w = action_w
                    );
                    if stdout.write_all(row.as_bytes()).is_err() {
                        return Ok(0);
                    }
                }
                let _ = stdout.flush();
                let unaliased = collect_unaliased_commands(&menu.items);
                if !unaliased.is_empty() {
                    let unaliased_header = format!(
                        "\n{} commands without alias (consider adding 'alias' or 'aliases'):\n",
                        unaliased.len()
                    );
                    if stdout.write_all(unaliased_header.as_bytes()).is_err() {
                        return Ok(0);
                    }

                    for (name, cmd, file) in unaliased {
                        let (action, details) = if let Some(c) = cmd {
                            ("cmd", c)
                        } else if let Some(f) = file {
                            ("file", f)
                        } else {
                            ("", String::new())
                        };
                        let d = if details.len() > max_details {
                            let mut s = details.chars().take(max_details - 1).collect::<String>();
                            s.push('…');
                            s
                        } else {
                            details
                        };
                        let line = format!(
                            "  - {:<name_w$}  {:<action_w$}  {}\n",
                            name,
                            action,
                            d,
                            name_w = name_w,
                            action_w = action_w
                        );
                        if stdout.write_all(line.as_bytes()).is_err() {
                            return Ok(0);
                        }
                    }
                }
                return Ok(0);
            } else {
                println!("No menu loaded. Use --menu to specify a config file.");
                return Ok(0);
            }
        }
        // Try alias first if we have a menu
        if let Some(menu) = &initial_menu {
            if let Some(item) = find_item_by_alias(&menu.items, &t) {
                if !item.items.is_empty() {
                    // Alias should point to a leaf; fall back to opening the menu
                    (Screen::Menu(initial_menu.take().unwrap()), menu_path)
                } else if let Some(file) = &item.file {
                    let view = open_file_view(Path::new(file));
                    (Screen::Output(view), menu_path)
                } else if let Some(cmd) = &item.cmd {
                    let external = item.external.unwrap_or(false);

                    // Build command with arguments
                    let full_cmd = if !cli.args.is_empty() {
                        format!("{} {}", cmd, cli.args.join(" "))
                    } else {
                        cmd.clone()
                    };

                    // Defer command start to app loop
                    startup_cmd = Some((item.name.clone(), full_cmd.clone(), external));
                    let view = OutputView::new(format!("{}: {}", item.name, full_cmd));
                    (Screen::Output(view), menu_path)
                } else if item.name == "Configuration" || item.alias.as_deref() == Some("config") {
                    (Screen::Config(open_config_state()), menu_path)
                } else {
                    (Screen::Menu(initial_menu.take().unwrap()), menu_path)
                }
            } else {
                // Not an alias; treat as path if exists
                let p = PathBuf::from(&t);
                if p.exists() {
                    let view = open_file_view(&p);
                    (Screen::Output(view), None)
                } else {
                    // Unknown; show menu if available else fallback to README
                    if let Some(m) = initial_menu.take() {
                        (Screen::Menu(m), menu_path)
                    } else {
                        let view = open_file_view_with_search("README.md");
                        (Screen::Output(view), None)
                    }
                }
            }
        } else {
            // No menu available; treat target as path
            let p = PathBuf::from(&t);
            let view = open_file_view(&p);
            (Screen::Output(view), None)
        }
    } else if let Some(m) = initial_menu.take() {
        (Screen::Menu(m), menu_path)
    } else {
        // Fallback: README.md with global priority search
        let view = open_file_view_with_search("README.md");
        (Screen::Output(view), None)
    };

    // Load MOTD.md: prefer local .dx/, then global ~/.dx/, then local, then ancestors
    let (mut motd_lines, motd_force_raw) = {
        // First check local .dx/MOTD.md (project-specific)
        let local_dx_motd = PathBuf::from(".dx").join("MOTD.md");
        if let Some((lines, raw)) = motd::read_motd_file(&local_dx_motd) {
            (lines, raw)
        } else {
            // Then check global ~/.dx/MOTD.md
            let global_motd = std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".dx").join("MOTD.md"));
            
            if let Some(global_path) = &global_motd {
                if let Some((lines, raw)) = motd::read_motd_file(global_path) {
                    (lines, raw)
                } else if let Some((lines, raw)) = motd::read_motd_file(Path::new("MOTD.md")) {
                    (lines, raw)
                } else if let Some(p) = motd::find_motd_in_ancestors() {
                    motd::read_motd_file(&p).unwrap_or((Vec::new(), false))
                } else {
                    (Vec::new(), false)
                }
            } else {
                // Fallback if HOME not available
                if let Some((lines, raw)) = motd::read_motd_file(Path::new("MOTD.md")) {
                    (lines, raw)
                } else if let Some(p) = motd::find_motd_in_ancestors() {
                    motd::read_motd_file(&p).unwrap_or((Vec::new(), false))
                } else {
                    (Vec::new(), false)
                }
            }
        }
    };

    // Validate configuration files early and collect warnings/errors (non-silent)
    let mut startup_issues: Vec<String> = Vec::new();
    // 1) Validate menu (dx/menu)
    if let Some(menu) = &initial_menu {
        let issues = validate_menu(&menu.items);
        if !issues.is_empty() {
            startup_issues.push("Menu validation found issues:".to_string());
            for i in &issues {
                startup_issues.push(format!("  - {}", i));
            }
        }
    }
    // 2) Validate app config structure (global/local)
    if let Some(path) = std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".dx").join("config.toml"))
    {
        if let Some((errs, warns)) = validate_app_config_file(&path) {
            if !errs.is_empty() {
                startup_issues.push(format!("Config {} errors:", path.display()));
                for e in errs {
                    startup_issues.push(format!("  - {}", e));
                }
            }
            if !warns.is_empty() {
                startup_issues.push(format!("Config {} warnings:", path.display()));
                for w in warns {
                    startup_issues.push(format!("  - {}", w));
                }
            }
        }
    }
    {
        let path = PathBuf::from("config.toml");
        if path.exists() {
            if let Some((errs, warns)) = validate_app_config_file(&path) {
                if !errs.is_empty() {
                    startup_issues.push(format!("Config {} errors:", path.display()));
                    for e in errs {
                        startup_issues.push(format!("  - {}", e));
                    }
                }
                if !warns.is_empty() {
                    startup_issues.push(format!("Config {} warnings:", path.display()));
                    for w in warns {
                        startup_issues.push(format!("  - {}", w));
                    }
                }
            }
        }
    }
    // If issues exist, print to stderr and prepend to MOTD so user sees them in UI
    if !startup_issues.is_empty() {
        for line in &startup_issues {
            warn!("{}", line);
        }
        motd_lines = motd::prepend_system_banner(
            motd_lines,
            "Configuration issues detected:",
            &startup_issues,
        );
    }

    // Load configs: global then local with optional override
    let mut status_text: Option<String> = None;
    let mut status_rx: Option<tokio::sync::mpsc::Receiver<String>> = None;
    let mut status_child: Option<Child> = None;
    let mut motd_wrap_cfg = true;
    let mut motd_color_cfg: Option<Color> = None;
    let mut markdown_enabled_cfg = true;
    let mut output_dim_cfg = true;
    let mut theme_dark_cfg = true; // default assume dark terminals
    let mut telemetry_cfg: Option<TelemetryConfig> = None;
    let mut asciinema_cfg: Option<AsciinemaConfig> = None;
    let mut show_fps_cfg: bool = true;
    // Read environment to detect if we're running under asciinema (relaunch case)
    let asciinema_badge_env: Option<String> = {
        let mode = std::env::var("DX_ASCIINEMA").ok();
        if let Some(m) = mode {
            if m == "record" {
                let file = std::env::var("DX_ASC_FILE").ok();
                let label = match file {
                    Some(f) => format!("⏺ recording -> {}", f),
                    None => "⏺ recording".to_string(),
                };
                Some(label)
            } else if m == "stream" {
                let stream_mode = std::env::var("DX_ASC_MODE").ok();
                let detail = match stream_mode.as_deref() {
                    Some("remote") => std::env::var("DX_ASC_REMOTE")
                        .ok()
                        .unwrap_or_else(|| "remote".to_string()),
                    _ => std::env::var("DX_ASC_LOCAL_ADDR")
                        .ok()
                        .unwrap_or_else(|| "local".to_string()),
                };
                Some(format!("📡 Streaming ({})", detail))
            } else {
                None
            }
        } else {
            None
        }
    };

    fn read_app_config(path: &Path) -> Option<AppConfig> {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str::<AppConfig>(&s).ok())
    }

    let home = std::env::var("HOME").ok();
    let global_cfg = home
        .map(|h| PathBuf::from(h).join(".dx").join("config.toml"))
        .and_then(|p| read_app_config(&p));
    let local_cfg = read_app_config(Path::new("config.toml"));

    if let Some(g) = &global_cfg {
        if let Some(b) = g.motd_wrap {
            motd_wrap_cfg = b;
        }
    }
    if let Some(l) = &local_cfg {
        if let Some(b) = l.motd_wrap {
            motd_wrap_cfg = b;
        }
    }
    if let Some(g) = &global_cfg {
        if let Some(c) = &g.motd_color {
            motd_color_cfg = parse_color(c);
        }
    }
    if let Some(l) = &local_cfg {
        if let Some(c) = &l.motd_color {
            motd_color_cfg = parse_color(c);
        }
    }
    if let Some(g) = &global_cfg {
        if let Some(b) = g.markdown_enabled {
            markdown_enabled_cfg = b;
        }
    }
    if let Some(l) = &local_cfg {
        if let Some(b) = l.markdown_enabled {
            markdown_enabled_cfg = b;
        }
    }
    if let Some(g) = &global_cfg {
        if let Some(b) = g.output_dim {
            output_dim_cfg = b;
        }
    }
    if let Some(l) = &local_cfg {
        if let Some(b) = l.output_dim {
            output_dim_cfg = b;
        }
    }
    if let Some(g) = &global_cfg {
        if let Some(t) = &g.theme {
            theme_dark_cfg = t.eq_ignore_ascii_case("dark");
        }
    }
    if let Some(l) = &local_cfg {
        if let Some(t) = &l.theme {
            theme_dark_cfg = t.eq_ignore_ascii_case("dark");
        }
    }
    // Telemetry: project overrides global entirely if present
    if let Some(g) = &global_cfg {
        if let Some(t) = &g.telemetry {
            telemetry_cfg = Some(t.clone());
        }
    }
    if let Some(l) = &local_cfg {
        if let Some(t) = &l.telemetry {
            telemetry_cfg = Some(t.clone());
        }
    }
    if let Some(g) = &global_cfg {
        if let Some(a) = &g.asciinema {
            asciinema_cfg = Some(a.clone());
        }
    }
    if let Some(l) = &local_cfg {
        if let Some(a) = &l.asciinema {
            asciinema_cfg = Some(a.clone());
        }
    }
    if let Some(g) = &global_cfg {
        if let Some(b) = g.show_fps {
            show_fps_cfg = b;
        }
    }
    if let Some(l) = &local_cfg {
        if let Some(b) = l.show_fps {
            show_fps_cfg = b;
        }
    }

    // Decide status source
    let allow_override = global_cfg
        .as_ref()
        .map(|c| c.allow_project_override)
        .unwrap_or(true);

    let chosen_status = if allow_override {
        // Local status if present, else global
        local_cfg
            .as_ref()
            .and_then(|c| c.status.as_ref())
            .or_else(|| global_cfg.as_ref().and_then(|c| c.status.as_ref()))
    } else {
        // Only global allowed
        global_cfg.as_ref().and_then(|c| c.status.as_ref())
    };

    if let Some(status) = chosen_status {
        if let Some(t) = &status.text {
            status_text = Some(t.clone());
        }
        if let Some(cmd) = &status.command {
            let (child, rx) = spawn_status_command(cmd)?;
            status_child = Some(child);
            status_rx = Some(rx);
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // legacy CPU overlay removed in favor of plugin runtime
    // Determine theme tokens from config
    let theme_tokens = if theme_dark_cfg {
        theme::ThemeTokens::builtin_dark()
    } else {
        theme::ThemeTokens::builtin_light()
    };
    // Try dynamic plugin load from default path (if exists). Ignored on failure.
    let mut plugin_overlays: Vec<plugin::OverlayRuntime> = Vec::new();
    let env_path = std::env::var("DX_PLUGIN_CPU").ok();
    let debug_path = PathBuf::from("target")
        .join("debug")
        .join("libdx_overlay_cpu.dylib");
    let release_path = PathBuf::from("target")
        .join("release")
        .join("libdx_overlay_cpu.dylib");
    // also try next to the executable: <exe_dir>/plugins/libdx_overlay_cpu.{dylib,dxplugin}
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let exe_plugin_dylib = exe_dir
        .as_ref()
        .map(|d| d.join("plugins").join("libdx_overlay_cpu.dylib"));
    let exe_plugin_dxplugin = exe_dir
        .as_ref()
        .map(|d| d.join("plugins").join("libdx_overlay_cpu.dxplugin"));
    // Also try XDG-like user share dir and legacy ~/.dx/plugins
    let home = std::env::var("HOME").ok();
    let user_plugin_dylib = home
        .as_ref()
        .map(|h| PathBuf::from(h).join(".local/share/dx/plugins/libdx_overlay_cpu.dylib"));
    let user_plugin_dxplugin = home
        .as_ref()
        .map(|h| PathBuf::from(h).join(".local/share/dx/plugins/libdx_overlay_cpu.dxplugin"));
    let legacy_plugin_dylib = home
        .as_ref()
        .map(|h| PathBuf::from(h).join(".dx/plugins/libdx_overlay_cpu.dylib"));
    let legacy_plugin_dxplugin = home
        .as_ref()
        .map(|h| PathBuf::from(h).join(".dx/plugins/libdx_overlay_cpu.dxplugin"));
    let candidates: Vec<String> = [
        env_path.as_deref(),
        debug_path.to_str(),
        release_path.to_str(),
        exe_plugin_dylib.as_ref().and_then(|p| p.to_str()),
        exe_plugin_dxplugin.as_ref().and_then(|p| p.to_str()),
        user_plugin_dylib.as_ref().and_then(|p| p.to_str()),
        user_plugin_dxplugin.as_ref().and_then(|p| p.to_str()),
        legacy_plugin_dylib.as_ref().and_then(|p| p.to_str()),
        legacy_plugin_dxplugin.as_ref().and_then(|p| p.to_str()),
    ]
    .into_iter()
    .flatten()
    .map(|s| s.to_string())
    .collect();
    let mut loaded = false;
    for p in candidates.iter() {
        if Path::new(p).exists() {
            match crate::plugin::try_load_overlay_runtime(p) {
                Ok(rt) => {
                    plugin_overlays.push(rt);
                    loaded = true;
                    break;
                }
                Err(e) => {
                    warn!(plugin = %p, error = %e, "Failed to load plugin overlay");
                }
            }
        }
    }
    if !loaded {
        warn!(
            "CPU overlay plugin not loaded. Set DX_PLUGIN_CPU to .dylib path or build dx-overlay-cpu."
        );
    }

    let result = run_app(
        &mut terminal,
        App {
            screen,
            screen_stack: Vec::new(),
            child: None,
            child_stdin: None,
            rx: None,
            menu_path,
            confirm: None,
            needs_clear: true,
            motd_lines,
            last_content_area: None,
            status_text,
            status_rx,
            status_child,
            motd_wrap: motd_wrap_cfg,
            motd_force_raw,
            motd_color: motd_color_cfg,
            markdown_enabled: markdown_enabled_cfg,
            menu_cmd: None,
            output_dim: output_dim_cfg,
            theme_dark: theme_dark_cfg,
            theme: theme_tokens,
            pty_child: None,
            pty_master: None,
            pty_writer: None,
            selection_mode: false,
            mouse_captured: true,
            startup_cmd,
            telemetry: telemetry_cfg,
            asciinema: asciinema_cfg,
            asciinema_live: cli.live,
            asciinema_badge: asciinema_badge_env,
            auto_open: !cli.disable_auto_open,
            blink_on: true,
            blink_tick: 0,
            last_click_at: None,
            last_click_index: None,
            plugin_overlays,
            fps_frames: 0,
            fps_last_instant: Instant::now(),
            fps: 0.0,
            show_fps: show_fps_cfg,
        },
    );

    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    match result {
        Ok(()) => Ok(0),
        Err(e) => Err(e),
    }
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        // Start any deferred alias command once at the beginning
        if let Some((title, cmd, external)) = app.startup_cmd.take() {
            if external {
                let _ = passthrough_command(&mut app, &title, &cmd);
            } else {
                let _ = start_command(&mut app, &title, &cmd);
            }
        }
        // Drain any incoming output lines
        if let Some(rx) = &mut app.rx {
            while let Ok(msg) = rx.try_recv() {
                if let Screen::Output(view) = &mut app.screen {
                    match msg {
                        OutputMsg::Line(line) => {
                            // If terminal emulator is active, ignore line-mode events
                            if let Some(_t) = view.term.as_ref() {
                                continue;
                            }
                            // Commit pending then push a new full line
                            if let Some(p) = view.pending_line.take() {
                                view.lines.push(p);
                            }
                            view.lines.push(line);
                            if view.auto_scroll {
                                let vh = view.viewport_height.max(1);
                                let total = view.lines.len() as u16;
                                view.scroll_y = total.saturating_sub(vh);
                            }
                        }
                        OutputMsg::ReplaceCurrent(cur) => {
                            // If terminal emulator is active, ignore line-mode events
                            if let Some(_t) = view.term.as_ref() {
                                continue;
                            }
                            view.pending_line = Some(cur);
                        }
                        OutputMsg::Chunk(bytes) => {
                            // Prefer classic line renderer unless we detect TUI/alt-screen sequences
                            if view.term.is_none() {
                                if bytes_look_like_tui(&bytes) {
                                    let (rows, cols) = if let Some(area) = app.last_content_area {
                                        let rows = area.height.saturating_sub(2 + PAD_Y * 2).max(1);
                                        let cols = area.width.saturating_sub(2 + PAD_X * 2).max(1);
                                        (rows, cols)
                                    } else {
                                        (24, 80)
                                    };
                                    view.term = Some(term::Emulator::new(rows, cols));
                                } else {
                                    // No TUI sequence detected; let line-based path handle output
                                    continue;
                                }
                            }
                            if let Some(t) = view.term.as_mut() {
                                t.process_bytes(&bytes);
                            }
                        }
                    }
                }
            }
        }
        // Drain status updates
        if let Some(srx) = &mut app.status_rx {
            while let Ok(line) = srx.try_recv() {
                app.status_text = Some(line);
            }
        }

        // Check if running child has exited
        if let Some(child) = &mut app.child {
            if let Ok(Some(status)) = child.try_wait() {
                if let Screen::Output(view) = &mut app.screen {
                    view.running = false;
                    view.exit_status = status.code();
                    view.ended_at = Some(Instant::now());
                    // Append colored completion line
                    if let Some(p) = view.pending_line.take() {
                        view.lines.push(p);
                    }
                    view.lines.push(String::new());
                    // Suppress verbose PTY session ended line
                    // Strong, clearly visible instruction to return to menu
                    view.lines.push(
                        "\x1b[97;1mPress Esc or q to return to the main menu\x1b[0m".to_string(),
                    );
                    // Extra spacer so bottom-right overlay does not cover the instruction line
                    view.lines.push(String::new());
                    // Auto-scroll to show the summary line
                    let vh = view.viewport_height.max(1);
                    let total = view.lines.len() as u16;
                    view.scroll_y = total.saturating_sub(vh);
                    view.auto_scroll = true;
                    // Telemetry: send full log when non-zero exit and enabled
                    if let (Some(cfg), Some(code)) = (app.telemetry.as_ref(), status.code()) {
                        if cfg.enabled && code != 0 {
                            if let Some(endpoint) = cfg.endpoint.as_ref() {
                                let payload = TelemetryPayload {
                                    title: view.title.clone(),
                                    exit_code: code,
                                    lines: view.lines.clone(),
                                };
                                let endpoint = endpoint.clone();
                                let _ = std::thread::spawn(move || {
                                    let client = reqwest::blocking::Client::new();
                                    let _ = client.post(endpoint).json(&payload).send();
                                });
                            }
                        }
                    }
                }
                app.child = None;
                app.rx = None; // readers should have closed
                app.child_stdin = None;
            }
        }
        // Check PTY child
        if let Some(child) = app.pty_child.as_mut() {
            if child.try_wait()?.is_some() {
                if let Screen::Output(view) = &mut app.screen {
                    view.running = false;
                    // Parse exit code marker from last lines if present
                    let mut exit_code: Option<i32> = None;
                    for l in view.lines.iter().rev().take(8) {
                        if let Some(rest) = l.strip_prefix("__DX_EXIT_CODE:") {
                            if let Ok(n) = rest.trim().parse::<i32>() {
                                exit_code = Some(n);
                                break;
                            }
                        }
                    }
                    // Hide internal marker lines from the final output
                    view.lines.retain(|l| !l.starts_with("__DX_EXIT_CODE:"));
                    view.exit_status = exit_code;
                    view.ended_at = Some(Instant::now());
                    if let Some(p) = view.pending_line.take() {
                        view.lines.push(p);
                    }
                    view.lines.push(String::new());
                    let msg = match exit_code {
                        Some(0) => "\x1b[32m[✔] Completed successfully\x1b[0m".to_string(),
                        Some(code) => format!("\x1b[31m[✖] Failed (exit {})\x1b[0m", code),
                        None => String::new(),
                    };
                    view.lines.push(msg);
                    let vh = view.viewport_height.max(1);
                    let total = view.lines.len() as u16;
                    view.scroll_y = total.saturating_sub(vh);
                    view.auto_scroll = true;
                    // Telemetry for PTY: send when non-zero exit code marker present
                    if let (Some(cfg), Some(code)) = (app.telemetry.as_ref(), view.exit_status) {
                        if cfg.enabled {
                            if let Some(endpoint) = cfg.endpoint.as_ref() {
                                if code != 0 {
                                    let payload = TelemetryPayload {
                                        title: view.title.clone(),
                                        exit_code: code,
                                        lines: view.lines.clone(),
                                    };
                                    let endpoint = endpoint.clone();
                                    let _ = std::thread::spawn(move || {
                                        let client = reqwest::blocking::Client::new();
                                        let _ = client.post(endpoint).json(&payload).send();
                                    });
                                }
                            }
                        }
                    }
                }
                app.pty_child = None;
                app.pty_master = None;
                app.pty_writer = None;
                app.rx = None;
            }
        }

        if app.needs_clear {
            terminal.clear()?;
            app.needs_clear = false;
        }
        terminal.draw(|f| {
            let area = f.area();

            // Compute MOTD area (top) and content area (bottom)
            let motd_line_count = app.motd_lines.len() as u16;
            let motd_height = if motd_line_count > 0 {
                motd_line_count.min(area.height.saturating_sub(3))
            } else {
                0
            };
            let chunks = frame::split_main_area(area, motd_height);

            // Render MOTD if any
            if motd_height > 0 {
                motd::render_motd(
                    f,
                    chunks[0],
                    &app.motd_lines,
                    app.markdown_enabled,
                    app.motd_wrap,
                    app.motd_force_raw,
                    app.motd_color,
                    &app.theme,
                );
            }

            // Decide which chunk is the content area
            let content_idx = if motd_height > 0 { 2 } else { 0 };
            let content_area = chunks[content_idx];

            // If we have a status bar, split content area to leave 1 line at the bottom
            let want_status =
                app.status_text.is_some() || app.asciinema_badge.is_some() || app.show_fps;
            let (main_area, status_area) = if want_status {
                let parts = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(vec![Constraint::Min(2), Constraint::Length(1)])
                    .split(content_area);
                (parts[0], Some(parts[1]))
            } else {
                (content_area, None)
            };

            // Record the actual interactive content area (excludes status bar)
            app.last_content_area = Some(main_area);

            match &mut app.screen {
                Screen::Menu(_menu) => {
                    screens::menu::render(f, main_area, &mut app);
                }
                Screen::Output(_view) => {
                    screens::output::render(f, main_area, &mut app);
                }
                Screen::Config(_cfg) => {
                    screens::config::render(f, main_area, &mut app);
                }
                Screen::Form(_f) => {
                    screens::form::render(f, main_area, &mut app);
                }
            }

            // Render status bar if any
            if let Some(area) = status_area {
                let mut spans: Vec<Span> = Vec::new();
                if let Some(t) = &app.status_text {
                    spans.push(Span::raw(t.clone()));
                }
                if app.status_text.is_some() && app.asciinema_badge.is_some() {
                    spans.push(Span::raw("  |  "));
                }
                if let Some(b) = &app.asciinema_badge {
                    if b.starts_with("📡") {
                        let dot = if app.blink_on { "⏺" } else { " " };
                        spans.push(Span::styled(dot, Style::default().fg(Color::Red)));
                        if let Some((_, rest)) = b.split_once(' ') {
                            spans.push(Span::raw(format!(" {}", rest)));
                        }
                    } else {
                        spans.push(Span::raw(b.clone()));
                    }
                }
                if app.show_fps {
                    if !spans.is_empty() {
                        spans.push(Span::raw("  |  "));
                    }
                    spans.push(Span::raw(format!("FPS: {}", app.fps.round() as u32)));
                }
                if !spans.is_empty() {
                    frame::render_status_bar(Line::from(spans), area, f);
                }
            }

            // Overlay in top-right corner (scaled down)
            let scale: f32 = 0.3; // ~30% of original
            let overlay_w: u16 = ((40.0 * scale).round() as u16).max(12);
            let overlay_h: u16 = ((16.0 * scale).round() as u16).max(5);
            let ox = area.x + area.width.saturating_sub(overlay_w).saturating_sub(1);
            let oy = area.y;
            let overlay_area = Rect {
                x: ox,
                y: oy,
                width: overlay_w.min(area.width),
                height: overlay_h.min(area.height),
            };
            if let Some(rt) = app.plugin_overlays.get(0) {
                rt.render(f, overlay_area);
            }
        })?;

        // Update FPS after a successful frame draw
        app.fps_frames = app.fps_frames.saturating_add(1);
        let elapsed = app.fps_last_instant.elapsed();
        if elapsed >= Duration::from_secs(1) {
            let secs = elapsed.as_secs_f32();
            if secs > 0.0 {
                app.fps = (app.fps_frames as f32) / secs;
            }
            app.fps_frames = 0;
            app.fps_last_instant = Instant::now();
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            match event::read()? {
                CEvent::Key(key) => {
                    if handle_key_event(&mut app, key)? {
                        break;
                    }
                }
                CEvent::Mouse(me) => {
                    // Allow mouse wheel scrolling even in selection mode
                    if !app.selection_mode || matches!(me.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown) {
                        handle_mouse_event(&mut app, me);
                    }
                }
                CEvent::Paste(s) => {
                    if let Some(w) = &mut app.pty_writer {
                        let _ = w.write_all(b"\x1b[200~");
                        let _ = w.write_all(s.as_bytes());
                        let _ = w.write_all(b"\x1b[201~");
                        let _ = w.flush();
                    }
                }
                CEvent::FocusGained => {
                    if let Some(w) = &mut app.pty_writer {
                        let _ = w.write_all(b"\x1b[I");
                        let _ = w.flush();
                    }
                }
                CEvent::FocusLost => {
                    if let Some(w) = &mut app.pty_writer {
                        let _ = w.write_all(b"\x1b[O");
                        let _ = w.flush();
                    }
                }
                CEvent::Resize(_, _) => {
                    // Resize PTY to match current content area if available
                    if let Some(area) = app.last_content_area {
                        let rows = area.height.saturating_sub(2 + PAD_Y * 2);
                        let cols = area.width.saturating_sub(2 + PAD_X * 2);
                        let size = PtySize {
                            rows: rows.max(1),
                            cols: cols.max(1),
                            pixel_width: 0,
                            pixel_height: 0,
                        };
                        crate::exec::pty_resize(&mut app.pty_master, size);
                    }
                    // Resize terminal emulator viewport as well
                    if let Screen::Output(view) = &mut app.screen {
                        if let (Some(area), Some(t)) = (app.last_content_area, view.term.as_mut()) {
                            let rows = area.height.saturating_sub(2 + PAD_Y * 2);
                            let cols = area.width.saturating_sub(2 + PAD_X * 2);
                            t.resize(rows.max(1), cols.max(1));
                        }
                    }
                } // Ignore other events
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
            // advance spinner if running
            if let Screen::Output(view) = &mut app.screen {
                if view.running {
                    view.spinner_idx = (view.spinner_idx + 1) % SPINNER_FRAMES.len();
                }
            }
            // advance blink (toggle every 5 ticks ~ 500ms)
            app.blink_tick = app.blink_tick.wrapping_add(1);
            if app.blink_tick % 5 == 0 {
                app.blink_on = !app.blink_on;
            }
            // Drain plugin overlay events (scheduled via tokio)
            for rt in &mut app.plugin_overlays {
                let _ = rt.drain();
            }
            // Handle AppCommands emitted by plugins
            for rt in &mut app.plugin_overlays {
                for cmd in rt.drain_app_commands() {
                    match cmd {
                        dx_sdk::host::AppCommand::Toast { title, body } => {
                            // minimal: show in status for one tick
                            app.status_text = Some(format!("{}: {}", title, body));
                        }
                        dx_sdk::host::AppCommand::SetStatusBadge { text } => {
                            app.asciinema_badge = Some(text);
                        }
                        dx_sdk::host::AppCommand::ClearStatusBadge => {
                            app.asciinema_badge = None;
                        }
                        dx_sdk::host::AppCommand::NavigateToOutput { title } => {
                            app.screen = Screen::Output(OutputView::new(title));
                        }
                        dx_sdk::host::AppCommand::AppendOutputLine { line } => {
                            if let Screen::Output(view) = &mut app.screen {
                                view.lines.push(line);
                                view.scroller.set_total(view.lines.len() as u16);
                                view.scroller.set_auto(true);
                            }
                        }
                        dx_sdk::host::AppCommand::AppendOutputChunk { bytes } => {
                            let s = String::from_utf8_lossy(&bytes).to_string();
                            if let Screen::Output(view) = &mut app.screen {
                                view.lines.push(s);
                                view.scroller.set_total(view.lines.len() as u16);
                                view.scroller.set_auto(true);
                            }
                        }
                        dx_sdk::host::AppCommand::OpenUrl { url } => {
                            let _ = open_default_browser(&url);
                        }
                        dx_sdk::host::AppCommand::Log {
                            level: _,
                            message: _,
                        } => {}
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_key_event(app: &mut App, key: KeyEvent) -> Result<bool> {
    // If a confirmation modal is open, handle it with priority
    if let Some(Confirm::KillProcess { yes_selected }) = app.confirm {
        match key.code {
            // Navigation between Yes/No buttons
            KeyCode::Left | KeyCode::Char('h') => {
                app.confirm = Some(Confirm::KillProcess { yes_selected: true });
                return Ok(false);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                app.confirm = Some(Confirm::KillProcess { yes_selected: false });
                return Ok(false);
            }
            // Direct shortcuts
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.confirm = Some(Confirm::KillProcess { yes_selected: true });
                return Ok(false);
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                app.confirm = Some(Confirm::KillProcess { yes_selected: false });
                return Ok(false);
            }
            KeyCode::Enter => {
                // Only kill if YES is selected
                if yes_selected {
                if let Some(child) = &mut app.child {
                    let _ = child.kill();
                    if let Ok(status) = child.wait() {
                        if let Screen::Output(view) = &mut app.screen {
                            view.exit_status = status.code();
                            view.running = false;
                            view.ended_at = Some(Instant::now());
                            // Append colored completion line after kill
                            if let Some(p) = view.pending_line.take() {
                                view.lines.push(p);
                            }
                            view.lines.push(String::new());
                            let msg = match status.code() {
                                Some(0) => "\x1b[32m[✔] Successfully completed\x1b[0m".to_string(),
                                Some(code) => format!("\x1b[31m[✖] Failed (exit {})\x1b[0m", code),
                                None => "\x1b[35m[⛔] Terminated\x1b[0m".to_string(),
                            };
                            view.lines.push(msg);
                            view.lines.push(
                                "\x1b[97;1mPress Esc or q to return to the main menu\x1b[0m"
                                    .to_string(),
                            );
                            view.lines.push(String::new());
                            // Auto-scroll to show the summary line
                            let vh = view.viewport_height.max(1);
                            let total = view.lines.len() as u16;
                            view.scroll_y = total.saturating_sub(vh);
                            view.auto_scroll = true;
                        }
                    }
                }
                if let Some(child) = app.pty_child.as_mut() {
                    let _ = child.kill();
                    if let Screen::Output(view) = &mut app.screen {
                        view.running = false;
                        view.ended_at = Some(Instant::now());
                        if let Some(p) = view.pending_line.take() {
                            view.lines.push(p);
                        }
                        view.lines.push(String::new());
                        view.lines
                            .push("\x1b[35m[⛔] Terminated\x1b[0m".to_string());
                        view.lines.push(
                            "\x1b[97;1mPress Esc or q to return to the main menu\x1b[0m"
                                .to_string(),
                        );
                        view.lines.push(String::new());
                        let vh = view.viewport_height.max(1);
                        let total = view.lines.len() as u16;
                        view.scroll_y = total.saturating_sub(vh);
                        view.auto_scroll = true;
                    }
                }
                app.child = None;
                app.rx = None;
                app.child_stdin = None;
                app.pty_child = None;
                app.pty_master = None;
                app.pty_writer = None;
                app.confirm = None;
                // Stay on Output view to let user read the summary; user can press b/Backspace or Esc/q to return to menu
                return Ok(false);
                } else {
                    // User selected "No" - just close the dialog
                    app.confirm = None;
                    return Ok(false);
                }
            }
            KeyCode::Esc => {
                app.confirm = None; // cancel
                return Ok(false);
            }
            _ => return Ok(false),
        }
    }

    match &mut app.screen {
        Screen::Menu(_menu) => {
            if screens::menu::handle_event(app, key)? {
                return Ok(true);
            }
        }
        Screen::Output(_view) => {
            if screens::output::handle_event(app, key)? {
                return Ok(true);
            }
        }
        Screen::Config(_cfg) => {
            if screens::config::handle_event(app, key)? {
                return Ok(true);
            }
        }
        Screen::Form(_f) => {
            if screens::form::handle_event(app, key)? {
                return Ok(true);
            }
        }
    }
    // Global bindings
    match key.code {
        KeyCode::Char('O') => {
            return Ok(false);
        }
        KeyCode::F(10) => {
            return Ok(false);
        }
        _ => {}
    }
    Ok(false)
}

use crate::config::open_config_state;

// use crate::config::save_app_config;

use crate::config::load_app_config_file as read_app_config_file;

// Validate AppConfig file and return (errors, warnings). None if file unreadable.
use crate::config::validate_app_config_file;

fn handle_mouse_event(app: &mut App, me: MouseEvent) {
    match &mut app.screen {
        Screen::Menu(menu) => match me.kind {
            MouseEventKind::ScrollUp => {
                if menu.selected_index > 0 {
                    menu.selected_index -= 1;
                }
            }
            MouseEventKind::ScrollDown => {
                if menu.selected_index + 1 < menu.items.len() {
                    menu.selected_index += 1;
                }
            }
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                if let Some(area) = app.last_content_area {
                    let mx = me.column as u16;
                    let my = me.row as u16;
                    // Check click within content area
                    if mx >= area.x
                        && mx < area.x + area.width
                        && my >= area.y
                        && my < area.y + area.height
                    {
                        // Inner rect of the list (borders + padding)
                        let inner = Rect {
                            x: area.x.saturating_add(1 + PAD_X),
                            y: area.y.saturating_add(1 + PAD_Y),
                            width: area.width.saturating_sub(2 + PAD_X * 2),
                            height: area.height.saturating_sub(2 + PAD_Y * 2),
                        };
                        if my >= inner.y && my < inner.y + inner.height {
                            let line_in_view = my - inner.y; // 0-based line inside inner rect
                            let idx = (line_in_view / 4) as usize; // 4 lines per item (3 content + 1 spacer)
                            let current = submenu_at(&menu.items, &menu.path);
                            if idx < current.len() {
                                // Update selection
                                if menu.selected_index != idx {
                                    menu.selected_index = idx;
                                }
                                // Double-click detection
                                let now = Instant::now();
                                let is_double = if let (Some(t), Some(i)) =
                                    (app.last_click_at, app.last_click_index)
                                {
                                    i == idx
                                        && now.saturating_duration_since(t)
                                            <= Duration::from_millis(350)
                                } else {
                                    false
                                };
                                app.last_click_at = Some(now);
                                app.last_click_index = Some(idx);
                                if is_double {
                                    // Simulate Enter key behavior
                                    if let Some(item) = current.get(menu.selected_index).cloned() {
                                        if !item.items.is_empty() {
                                            menu.path.push(menu.selected_index);
                                            menu.selected_index = 0;
                                        } else if let Some(file) = item.file {
                                            let view = open_file_view(Path::new(&file));
                                            app.screen = Screen::Output(view);
                                            app.needs_clear = true;
                                        } else if let Some(cmd) = item.cmd {
                                            let external = item.external.unwrap_or(false);
                                            if external {
                                                let _ = passthrough_command(app, &item.name, &cmd);
                                            } else {
                                                let _ = start_command(app, &item.name, &cmd);
                                                app.needs_clear = true;
                                            }
                                        } else if item.name == "Configuration"
                                            || item.alias.as_deref() == Some("config")
                                        {
                                            app.screen = Screen::Config(open_config_state());
                                            app.needs_clear = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        },
        Screen::Output(view) => match me.kind {
            MouseEventKind::ScrollUp => {
                let step: u16 = 3;
                for _ in 0..step {
                    view.scroller.line_up();
                }
                view.scroll_y = view.scroller.scroll_y;
                view.auto_scroll = false;
            }
            MouseEventKind::ScrollDown => {
                let step: u16 = 3;
                for _ in 0..step {
                    view.scroller.line_down();
                }
                view.scroll_y = view.scroller.scroll_y;
                if view.scroll_y == 0 {
                    view.auto_scroll = false;
                }
            }
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                if view.md_content.is_some() {
                    if let (Some(area), Some(start)) =
                        (app.last_content_area, view.md_footnote_start)
                    {
                        let mx = me.column as u16;
                        let my = me.row as u16;
                        if mx >= area.x
                            && mx < area.x + area.width
                            && my >= area.y
                            && my < area.y + area.height
                        {
                            // Inner area (inside borders + padding)
                            let inner = Rect {
                                x: area.x.saturating_add(1 + PAD_X),
                                y: area.y.saturating_add(1 + PAD_Y),
                                width: area.width.saturating_sub(2 + PAD_X * 2),
                                height: area.height.saturating_sub(2 + PAD_Y * 2),
                            };
                            if my >= inner.y && my < inner.y + inner.height {
                                let line_in_view = my - inner.y; // 0-based within viewport
                                let doc_line = view.scroll_y.saturating_add(line_in_view);
                                // Footnote list: blank line + 'Links:' + N entries
                                let link_start = start.saturating_add(2);
                                if doc_line >= link_start {
                                    let idx = doc_line - link_start;
                                    if let Some(dest) = view.md_links.get(idx as usize).cloned() {
                                        if let Some(base) = view
                                            .file_path
                                            .as_ref()
                                            .and_then(|p| p.parent())
                                            .map(|p| p.to_path_buf())
                                        {
                                            let target = base.join(&dest);
                                            if target
                                                .extension()
                                                .and_then(|s| s.to_str())
                                                .map(|s| {
                                                    matches!(
                                                        s.to_ascii_lowercase().as_str(),
                                                        "md" | "markdown"
                                                    )
                                                })
                                                .unwrap_or(false)
                                                && target.exists()
                                            {
                                                let mut v = open_file_view(&target);
                                                v.file_path = Some(target);
                                                app.screen = Screen::Output(v);
                                                app.needs_clear = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        },
        Screen::Config(_cfg) => match me.kind {
            _ => {}
        },
        Screen::Form(_f) => match me.kind {
            _ => {}
        },
    }
}

// removed unused helper (covered by load_menu usage paths)

// moved to menu.rs

// moved to menu.rs

fn open_file_view(path: &Path) -> OutputView {
    let mut view = OutputView::new(path.display().to_string());
    if let Ok(content) = fs::read_to_string(path) {
        let is_md = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| matches!(s.to_ascii_lowercase().as_str(), "md" | "markdown"))
            .unwrap_or(false);
        if is_md {
            view.md_content = Some(content);
            view.file_path = Some(path.to_path_buf());
            view.auto_scroll = false;
            view.wrap_enabled = true;
            view.render_markdown = true;
        } else {
            view.lines = content.lines().map(|s| s.to_string()).collect();
            view.file_path = Some(path.to_path_buf());
        }
    }
    view
}

/// Find file with priority: ./.dx/, ~/.dx/, current dir, then ancestors
fn find_file_with_global_priority(filename: &str) -> Option<PathBuf> {
    // FIRST: Check local .dx/ subdirectory
    let local_dx_path = PathBuf::from(".dx").join(filename);
    if local_dx_path.exists() {
        return Some(local_dx_path);
    }
    
    // SECOND: Check global ~/.dx/ directory
    if let Ok(home) = std::env::var("HOME") {
        let global_path = PathBuf::from(home).join(".dx").join(filename);
        if global_path.exists() {
            return Some(global_path);
        }
    }
    
    // THIRD: Check current directory
    let local_path = PathBuf::from(filename);
    if local_path.exists() {
        return Some(local_path);
    }
    
    // FOURTH: Search ancestors (for MOTD.md only)
    if filename == "MOTD.md" {
        return motd::find_motd_in_ancestors();
    }
    
    None
}

/// Open file with global priority search
fn open_file_view_with_search(filename: &str) -> OutputView {
    if let Some(path) = find_file_with_global_priority(filename) {
        open_file_view(&path)
    } else {
        let mut view = OutputView::new(format!("{} (not found)", filename));
        view.lines = vec![format!("{} not found in ~/.dx/, current dir, or ancestors", filename)];
        view
    }
}

#[allow(dead_code)]
fn centered_rect(pct_x: u16, pct_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(r);

    let vertical = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(popup_layout[1]);

    vertical[1]
}

fn centered_rect_fixed(w: u16, h: u16, within: Rect) -> Rect {
    let inner = Rect {
        x: within.x.saturating_add(1),
        y: within.y.saturating_add(1),
        width: within.width.saturating_sub(2),
        height: within.height.saturating_sub(2),
    };
    let ww = w.min(inner.width);
    let hh = h.min(inner.height);
    let x = inner.x + inner.width.saturating_sub(ww) / 2;
    let y = inner.y + inner.height.saturating_sub(hh) / 2;
    Rect {
        x,
        y,
        width: ww,
        height: hh,
    }
}

fn start_command(app: &mut App, title: &str, cmd_str: &str) -> Result<()> {
    let spawned = crate::exec::spawn_pty(cmd_str)?;

    let mut view = OutputView::new(format!("{}: {}", title, cmd_str));
    view.running = true;
    view.exit_status = None;
    view.started_at = Some(Instant::now());
    view.ended_at = None;
    view.md_content = None;
    // Terminal emulator will be enabled lazily once we detect TUI/alternate-screen escape
    // sequences in the incoming byte stream (see bytes_look_like_tui()).
    // This prevents suppression of plain line output for simple commands like `ls` or `tail`.

    app.screen = Screen::Output(view);
    // Route output events
    app.rx = Some(spawned.rx);
    app.needs_clear = true;
    // Clear legacy child fields and set PTY handles
    app.child = None;
    app.child_stdin = None;
    // Set PTY handles
    app.pty_child = Some(spawned.child);
    app.pty_master = Some(spawned.master);
    app.pty_writer = Some(spawned.writer);
    // Apply initial resize to fit content area if known
    if let (Some(master), Some(area)) = (app.pty_master.as_mut(), app.last_content_area) {
        let rows = area.height.saturating_sub(2 + PAD_Y * 2);
        let cols = area.width.saturating_sub(2 + PAD_X * 2);
        let size = PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        };
        let _ = master.resize(size);
    }
    Ok(())
}

fn start_command_enhanced(app: &mut App, title: &str, cmd_str: &str) -> Result<()> {
    // Enhanced terminal mode: use optimal PTY size with all improvements
    let (rows, cols) = if let Some(area) = app.last_content_area {
        let rows = area.height.saturating_sub(2 + PAD_Y * 2).max(1);
        let cols = area.width.saturating_sub(2 + PAD_X * 2).max(1);
        (rows, cols)
    } else {
        (24, 120)
    };

    let spawned = crate::exec::spawn_pty_with_size(cmd_str, rows, cols)?;

    let mut view = OutputView::new(format!("{}: {} [ENHANCED]", title, cmd_str));
    view.running = true;
    view.exit_status = None;
    view.started_at = Some(Instant::now());
    view.ended_at = None;
    view.md_content = None;

    app.screen = Screen::Output(view);
    // Route output events
    app.rx = Some(spawned.rx);
    app.needs_clear = true;
    // Clear legacy child fields and set PTY handles
    app.child = None;
    app.child_stdin = None;
    // Set PTY handles
    app.pty_child = Some(spawned.child);
    app.pty_master = Some(spawned.master);
    app.pty_writer = Some(spawned.writer);
    // Apply initial resize to fit content area
    if let (Some(master), Some(area)) = (app.pty_master.as_mut(), app.last_content_area) {
        let rows = area.height.saturating_sub(2 + PAD_Y * 2);
        let cols = area.width.saturating_sub(2 + PAD_X * 2);
        let size = PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        };
        let _ = master.resize(size);
    }
    Ok(())
}

fn passthrough_command(app: &mut App, title: &str, cmd_str: &str) -> Result<()> {
    // Leave alternate screen and raw mode, run child attached to current TTY, then restore dx
    // Clear any running child
    if let Some(mut c) = app.child.take() {
        let _ = c.kill();
        let _ = c.wait();
    }

    // Restore terminal to cooked mode and leave alt screen
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    drop(stdout);

    // Run command attached to TTY with a pause prompt to avoid flicker on short commands
    // Optionally wrap with asciinema
    let status = if let Some(ac) = &app.asciinema {
        if ac.enabled && ac.external {
            // live stream takes precedence when enabled via CLI or config
            if app.asciinema_live || ac.stream {
                let inner = format!(
                    "{}; printf '\n\n\033[97;1mPress Enter to return to dx\033[0m\n'; read -r _",
                    cmd_str
                );
                let cmdline = build_asciinema_stream_cmd(ac, &inner);
                let mut cmd = Command::new("sh");
                cmd.arg("-lc").arg(cmdline);
                cmd.current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
                cmd.env("DX_ASCIINEMA", "stream");
                if ac.stream_mode.eq_ignore_ascii_case("remote") {
                    cmd.env("DX_ASC_MODE", "remote");
                    if let Some(r) = &ac.remote {
                        cmd.env("DX_ASC_REMOTE", r);
                    }
                } else {
                    cmd.env("DX_ASC_MODE", "local");
                    if let Some(addr) = &ac.local_addr {
                        cmd.env("DX_ASC_LOCAL_ADDR", addr);
                    }
                }
                // Pipe outputs and sniff URL from stdout/stderr
                cmd.stdout(Stdio::piped());
                cmd.stderr(Stdio::piped());
                match cmd.spawn() {
                    Ok(mut child) => {
                        if let Some(stdout) = child.stdout.take() {
                            let opened = Arc::new(AtomicBool::new(false));
                            let opened2 = opened.clone();
                            let auto_open = app.auto_open;
                            std::thread::spawn(move || {
                                let reader = BufReader::new(stdout);
                                for line in reader.lines().flatten() {
                                    if opened2.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    if let Some(url) =
                                        line.trim().strip_prefix("Live streaming at ")
                                    {
                                        if auto_open {
                                            let _ = open_default_browser(url);
                                        }
                                        opened2.store(true, Ordering::Relaxed);
                                        break;
                                    } else if let Some(url) = first_url_in(&line) {
                                        if auto_open {
                                            let _ = open_default_browser(&url);
                                        }
                                        opened2.store(true, Ordering::Relaxed);
                                        break;
                                    }
                                }
                            });
                        }
                        if let Some(stderr) = child.stderr.take() {
                            let opened = Arc::new(AtomicBool::new(false));
                            let opened2 = opened.clone();
                            let auto_open = app.auto_open;
                            std::thread::spawn(move || {
                                let reader = BufReader::new(stderr);
                                for line in reader.lines().flatten() {
                                    if opened2.load(Ordering::Relaxed) {
                                        break;
                                    }
                                    if let Some(url) =
                                        line.trim().strip_prefix("Live streaming at ")
                                    {
                                        if auto_open {
                                            let _ = open_default_browser(url);
                                        }
                                        opened2.store(true, Ordering::Relaxed);
                                        break;
                                    } else if let Some(url) = first_url_in(&line) {
                                        if auto_open {
                                            let _ = open_default_browser(&url);
                                        }
                                        opened2.store(true, Ordering::Relaxed);
                                        break;
                                    }
                                }
                            });
                        }
                        child.wait()
                    }
                    Err(e) => Err(e),
                }
            } else {
                let file = generate_asciinema_filename(ac);
                // emulate overwrite and ensure dir exists
                if let Some(dir) = ac.dir.as_ref() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if ac.overwrite {
                    let _ = std::fs::remove_file(&file);
                }
                let inner = format!(
                    "{}; printf '\n\n\033[97;1mPress Enter to return to dx\033[0m\n'; read -r _",
                    cmd_str
                );
                let cmdline = build_asciinema_cmd(ac, &file, &inner);
                let mut cmd = Command::new("sh");
                cmd.arg("-lc").arg(cmdline);
                cmd.current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
                cmd.env("DX_ASCIINEMA", "record");
                cmd.env("DX_ASC_FILE", &file);
                match cmd.status() {
                    Ok(st) => Ok(st),
                    Err(_e) => {
                        // Fallback: run without asciinema wrapper
                        let wrapped = format!(
                            "{}; printf '\n\n\033[97;1mPress Enter to return to dx\033[0m\n'; read -r _",
                            cmd_str
                        );
                        Command::new("sh")
                            .arg("-lc")
                            .arg(wrapped)
                            .current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
                            .status()
                    }
                }
            }
        } else {
            let wrapped = format!(
                "{}; printf '\n\n\033[97;1mPress Enter to return to dx\033[0m\n'; read -r _",
                cmd_str
            );
            Command::new("sh")
                .arg("-lc")
                .arg(wrapped)
                .current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
                .status()
        }
    } else {
        let wrapped = format!(
            "{}; printf '\n\n\033[97;1mPress Enter to return to dx\033[0m\n'; read -r _",
            cmd_str
        );
        Command::new("sh")
            .arg("-lc")
            .arg(wrapped)
            .current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
            .status()
    };

    // After process, re-enter dx TUI
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    drop(stdout);

    // Show summary in output view so user sees result and hint
    let mut view = OutputView::new(format!("{}: {}", title, cmd_str));
    match status {
        Ok(st) => {
            if let Some(code) = st.code() {
                if code == 0 {
                    view.lines
                        .push("\x1b[32m[✔] Completed successfully\x1b[0m".to_string());
                    view.lines.push(String::new());
                    view.lines.push(
                        "\x1b[97;1mPress Esc or q to return to the main menu\x1b[0m".to_string(),
                    );
                    view.lines.push(String::new());
                } else {
                    view.lines
                        .push(format!("\x1b[31m[✖] Failed (exit {})\x1b[0m", code));
                }
                // Telemetry for passthrough: send when non-zero
                if code != 0 {
                    if let Some(cfg) = &app.telemetry {
                        if cfg.enabled {
                            if let Some(endpoint) = &cfg.endpoint {
                                let payload = TelemetryPayload {
                                    title: format!("{}: {}", title, cmd_str),
                                    exit_code: code,
                                    lines: view.lines.clone(),
                                };
                                let endpoint = endpoint.clone();
                                let _ = std::thread::spawn(move || {
                                    let client = reqwest::blocking::Client::new();
                                    let _ = client.post(endpoint).json(&payload).send();
                                });
                            }
                        }
                    }
                }
            } else {
                view.lines
                    .push("\x1b[35m[⛔] Terminated\x1b[0m".to_string());
            }
        }
        Err(e) => {
            view.lines.push(format!("\x1b[31m[✖] Error: {}\x1b[0m", e));
        }
    }
    view.lines
        .push("\x1b[97;1mPress Esc or q to return to the main menu\x1b[0m".to_string());
    view.lines.push(String::new());
    app.screen = Screen::Output(view);
    app.needs_clear = true;
    Ok(())
}

fn spawn_status_command(cmd_str: &str) -> Result<(Child, tokio::sync::mpsc::Receiver<String>)> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd_str)
        .current_dir(std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take();
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);
    if let Some(mut out) = stdout {
        thread::spawn(move || {
            let mut buf = [0u8; 1024];
            let mut cur = String::new();
            loop {
                match out.read(&mut buf) {
                    Ok(0) => {
                        if !cur.is_empty() {
                            let _ = tx.blocking_send(cur.clone());
                        }
                        break;
                    }
                    Ok(n) => {
                        let s = String::from_utf8_lossy(&buf[..n]);
                        for ch in s.chars() {
                            match ch {
                                '\r' => {
                                    cur.clear();
                                }
                                '\n' => {
                                    let line = std::mem::take(&mut cur);
                                    let _ = tx.blocking_send(line);
                                }
                                c => cur.push(c),
                            }
                        }
                        if !cur.is_empty() {
                            let _ = tx.blocking_send(cur.clone());
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
    Ok((child, rx))
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{:01}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

// moved to markdown.rs

// moved helpers into crate::asciinema
