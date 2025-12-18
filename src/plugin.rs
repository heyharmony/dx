use anyhow::Result;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color as TuiColor, Style};
use ratatui::text::Line as TuiLine;
use ratatui::text::Span as TuiSpan;
use ratatui::widgets::Paragraph;

use dx_sdk::overlay::OverlayMeta;
use dx_sdk::prelude::*;
use dx_sdk::render::{Line, RenderTree};
use dx_sdk::types::Color;
use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub struct OverlayRuntime {
    overlay: Box<dyn Overlay>,
    _lib: Option<Library>,
    host: HostImpl,
    rx: UnboundedReceiver<OverlayEvent>,
    app_rx: UnboundedReceiver<dx_sdk::host::AppCommand>,
}

impl OverlayRuntime {
    #[allow(dead_code)] // TODO: Use this function or remove it
    pub fn new(overlay: Box<dyn Overlay>) -> Self {
        let (tx, rx) = unbounded_channel();
        let (app_tx, app_rx) = unbounded_channel();
        let host = HostImpl::new(tx, app_tx);
        Self {
            overlay,
            _lib: None,
            host,
            rx,
            app_rx,
        }
    }
    pub fn new_with_lib(overlay: Box<dyn Overlay>, lib: Library) -> Self {
        let (tx, rx) = unbounded_channel();
        let (app_tx, app_rx) = unbounded_channel();
        let host = HostImpl::new(tx, app_tx);
        Self {
            overlay,
            _lib: Some(lib),
            host,
            rx,
            app_rx,
        }
    }
    pub fn init(&mut self) -> Result<()> {
        self.overlay
            .init(&mut self.host, Default::default())
            .map(|_| ())
    }
    pub fn drain(&mut self) -> Result<()> {
        while let Ok(ev) = self.rx.try_recv() {
            let _ = self.overlay.handle_event(&mut self.host, ev)?;
        }
        Ok(())
    }
    pub fn render(&self, f: &mut Frame, area: Rect) {
        let tree = self.overlay.render(RenderRequest {
            width: area.width,
            height: area.height,
        });
        render_tree(f, area, &tree);
    }

    pub fn drain_app_commands(&mut self) -> Vec<dx_sdk::host::AppCommand> {
        let mut cmds = Vec::new();
        while let Ok(cmd) = self.app_rx.try_recv() {
            cmds.push(cmd);
        }
        cmds
    }

    pub fn meta(&self) -> OverlayMeta {
        self.overlay.meta()
    }
}

struct MemStore(Mutex<HashMap<String, String>>);
impl MemStore {
    fn new() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}
impl KeyValueStore for MemStore {
    fn get(&self, key: &str) -> Option<String> {
        self.0.lock().ok().and_then(|m| m.get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) {
        if let Ok(mut m) = self.0.lock() {
            m.insert(key.to_string(), value.to_string());
        }
    }
    fn delete(&self, key: &str) {
        if let Ok(mut m) = self.0.lock() {
            m.remove(key);
        }
    }
}

struct HostImpl {
    tx: UnboundedSender<OverlayEvent>,
    app_tx: UnboundedSender<dx_sdk::host::AppCommand>,
    store: Arc<MemStore>,
}
impl HostImpl {
    fn new(
        tx: UnboundedSender<OverlayEvent>,
        app_tx: UnboundedSender<dx_sdk::host::AppCommand>,
    ) -> Self {
        Self {
            tx,
            app_tx,
            store: Arc::new(MemStore::new()),
        }
    }
}
impl HostContext for HostImpl {
    fn log(&self, _level: dx_sdk::types::LogLevel, _msg: &str) {}
    fn storage(&self) -> &dyn KeyValueStore {
        &*self.store
    }
    fn schedule_tick(&self, every_millis: u64) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut intv = tokio::time::interval(std::time::Duration::from_millis(every_millis));
            loop {
                intv.tick().await;
                let _ = tx.send(OverlayEvent::Tick);
            }
        });
    }
    fn emit_app_command(&self, cmd: dx_sdk::host::AppCommand) {
        let _ = self.app_tx.send(cmd);
    }

    fn spawn_process(
        &mut self,
        spec: dx_sdk::host::ProcessSpec,
    ) -> Result<dx_sdk::host::ProcessHandleId> {
        use tokio::process::Command;
        let app_tx = self.app_tx.clone();
        tokio::spawn(async move {
            let mut cmd = if spec.shell {
                let mut c = Command::new("sh");
                c.arg("-lc")
                    .arg(format!("{} {}", spec.cmd, spec.args.join(" ")));
                c
            } else {
                let mut c = Command::new(&spec.cmd);
                c.args(&spec.args);
                c
            };
            if let Some(cwd) = &spec.cwd {
                cmd.current_dir(cwd);
            }
            if !spec.env.is_empty() {
                cmd.envs(spec.env.iter().map(|(k, v)| (k, v)));
            }
            // PTY not handled here; fallback to pipe stdio
            cmd.stdout(std::process::Stdio::piped());
            if spec.merge_stderr {
                cmd.stderr(std::process::Stdio::null());
            } else {
                cmd.stderr(std::process::Stdio::piped());
            }
            match cmd.spawn() {
                Ok(mut child) => {
                    if let Some(mut out) = child.stdout.take() {
                        let app = app_tx.clone();
                        tokio::spawn(async move {
                            use tokio::io::AsyncReadExt;
                            let mut buf = [0u8; 4096];
                            loop {
                                match out.read(&mut buf).await {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        let _ =
                                            app.send(dx_sdk::host::AppCommand::AppendOutputChunk {
                                                bytes: buf[..n].to_vec(),
                                            });
                                    }
                                    Err(_) => break,
                                }
                            }
                        });
                    }
                    if !spec.merge_stderr {
                        if let Some(mut err) = child.stderr.take() {
                            let app = app_tx.clone();
                            tokio::spawn(async move {
                                use tokio::io::AsyncReadExt;
                                let mut buf = [0u8; 4096];
                                loop {
                                    match err.read(&mut buf).await {
                                        Ok(0) => break,
                                        Ok(n) => {
                                            let _ = app.send(
                                                dx_sdk::host::AppCommand::AppendOutputChunk {
                                                    bytes: buf[..n].to_vec(),
                                                },
                                            );
                                        }
                                        Err(_) => break,
                                    }
                                }
                            });
                        }
                    }
                    let _ = child.wait().await;
                }
                Err(_e) => {
                    let _ = app_tx.send(dx_sdk::host::AppCommand::Toast {
                        title: "spawn failed".into(),
                        body: spec.cmd,
                    });
                }
            }
        });
        // Simple increasing handle id could be added; return dummy for now
        Ok(dx_sdk::host::ProcessHandleId(0))
    }
    fn open_url(&self, url: &str) {
        let _ = self.app_tx.send(dx_sdk::host::AppCommand::OpenUrl {
            url: url.to_string(),
        });
    }
}

pub struct DynOverlay {
    _lib: Library,
    overlay: Box<dyn Overlay>,
}

impl DynOverlay {
    pub unsafe fn load(path: &str) -> Result<Self> {
        let lib = unsafe { Library::new(path)? };
        let ver: Symbol<unsafe extern "C" fn() -> dx_sdk::types::SdkVersion> =
            unsafe { lib.get(b"dx_sdk_version")? };
        let _v = unsafe { ver() }; // could check compatibility here
        let ctor: Symbol<unsafe extern "C" fn() -> Box<dyn Overlay>> =
            unsafe { lib.get(b"dx_overlay")? };
        let overlay = unsafe { ctor() };
        Ok(Self { _lib: lib, overlay })
    }
}

pub fn try_load_overlay_runtime(path: &str) -> Result<OverlayRuntime> {
    unsafe {
        let dyno = DynOverlay::load(path)?;
        // Keep the library alive inside the runtime
        let mut rt = OverlayRuntime::new_with_lib(dyno.overlay, dyno._lib);
        rt.init()?;
        Ok(rt)
    }
}

pub fn render_tree(f: &mut Frame, area: Rect, tree: &RenderTree) {
    match tree {
        RenderTree::Lines(lines) => {
            let tui_lines: Vec<TuiLine> = lines.iter().map(to_tui_line).collect();
            f.render_widget(Paragraph::new(tui_lines), area);
        }
        RenderTree::Bar {
            percent,
            color,
            label_right,
        } => {
            // Minimal bar: draw text-only representation until a full widget is needed
            let pct = *percent;
            let label = label_right.clone().unwrap_or_else(|| format!("{pct:>3}%"));
            let line = TuiLine::from(vec![TuiSpan::styled(
                label,
                Style::default().fg(to_tui_color(*color)),
            )]);
            f.render_widget(Paragraph::new(vec![line]), area);
        }
        RenderTree::Group(children) => {
            // Stack children vertically with simple height heuristics
            let mut y = area.y;
            let bottom = area.y.saturating_add(area.height);
            for child in children {
                if y >= bottom {
                    break;
                }
                let preferred_h: u16 = match child {
                    RenderTree::Lines(ls) => (ls.len() as u16).clamp(1, bottom - y),
                    RenderTree::Bar { .. } => 1,
                    RenderTree::Group(_) => 1,
                };
                let h = preferred_h.min(bottom - y);
                let child_area = Rect {
                    x: area.x,
                    y,
                    width: area.width,
                    height: h.max(1),
                };
                render_tree(f, child_area, child);
                y = y.saturating_add(h.max(1));
            }
        }
    }
}

fn to_tui_line(line: &Line) -> TuiLine<'static> {
    let spans: Vec<TuiSpan<'static>> = line
        .0
        .iter()
        .map(|s| {
            let style = match s.color {
                Some(c) => Style::default().fg(to_tui_color(c)),
                None => Style::default(),
            };
            TuiSpan::styled(s.text.clone(), style)
        })
        .collect();
    TuiLine::from(spans)
}

fn to_tui_color(c: Color) -> TuiColor {
    match c {
        Color::Black => TuiColor::Black,
        Color::DarkGray => TuiColor::DarkGray,
        Color::Gray => TuiColor::Gray,
        Color::White => TuiColor::White,
        Color::Red => TuiColor::Red,
        Color::Green => TuiColor::Green,
        Color::Yellow => TuiColor::Yellow,
        Color::Blue => TuiColor::Blue,
        Color::Magenta => TuiColor::Magenta,
        Color::Cyan => TuiColor::Cyan,
        Color::Rgb(r, g, b) => TuiColor::Rgb(r, g, b),
    }
}
