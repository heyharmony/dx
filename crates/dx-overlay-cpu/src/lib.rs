use std::collections::VecDeque;

use dx_sdk::prelude::*;
use std::time::Instant;
use sysinfo::{CpuRefreshKind, RefreshKind, System};

const HISTORY_SECS: usize = 60; // ~60s window

#[derive(Default)]
struct CpuOverlay {
    visible: bool,
    total_hist: VecDeque<u8>,
    per_core_hist: Vec<VecDeque<u8>>, // [core][time]
    cores: usize,
    last_ts: Option<Instant>,
    sys: Option<System>,
}

impl CpuOverlay {
    fn color_for(v: u8) -> Color {
        match v {
            0..=39 => Color::Green,
            40..=79 => Color::Yellow,
            _ => Color::Red,
        }
    }
}

impl Overlay for CpuOverlay {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn meta(&self) -> OverlayMeta {
        OverlayMeta {
            id: "overlay.cpu",
            name: "CPU Overlay",
            version: "0.1.0",
            capabilities: &["overlay"],
        }
    }

    fn init(&mut self, ctx: &mut dyn HostContext, _params: UiParams) -> Result<()> {
        self.visible = true;
        // prepare sysinfo once
        let mut sys =
            System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
        sys.refresh_cpu_specifics(CpuRefreshKind::everything());
        self.sys = Some(sys);
        ctx.schedule_tick(500);
        Ok(())
    }

    fn handle_event(
        &mut self,
        _ctx: &mut dyn HostContext,
        event: OverlayEvent,
    ) -> Result<OverlayEffect> {
        match event {
            OverlayEvent::Init { .. } => Ok(OverlayEffect::None),
            OverlayEvent::Tick => {
                // Sample CPU usage and update history
                if let Some(sys) = self.sys.as_mut() {
                    sys.refresh_cpu_specifics(CpuRefreshKind::everything());
                    let mut per: Vec<u8> = Vec::new();
                    for cpu in sys.cpus() {
                        per.push((cpu.cpu_usage().round() as u8).min(100));
                    }
                    let total = if !per.is_empty() {
                        (per.iter().map(|&v| v as u32).sum::<u32>() / per.len() as u32).min(100)
                            as u8
                    } else {
                        0
                    };
                    // resize histories on core count change
                    if self.cores != per.len() {
                        self.cores = per.len();
                        self.per_core_hist = (0..self.cores)
                            .map(|_| VecDeque::with_capacity(HISTORY_SECS * 2))
                            .collect();
                    }
                    // push bounded
                    if self.total_hist.len() == HISTORY_SECS * 2 {
                        let _ = self.total_hist.pop_front();
                    }
                    self.total_hist.push_back(total);
                    for (i, v) in per.into_iter().enumerate() {
                        if let Some(h) = self.per_core_hist.get_mut(i) {
                            if h.len() == HISTORY_SECS * 2 {
                                let _ = h.pop_front();
                            }
                            h.push_back(v);
                        }
                    }
                    self.last_ts = Some(Instant::now());
                }
                Ok(OverlayEffect::Redraw)
            }
            OverlayEvent::VisibilityChanged { visible } => {
                self.visible = visible;
                Ok(OverlayEffect::None)
            }
            OverlayEvent::Resize { .. } => Ok(OverlayEffect::None),
            OverlayEvent::Data { value: _ } => Ok(OverlayEffect::Redraw),
        }
    }

    fn render(&self, req: RenderRequest) -> RenderTree {
        let rows = req.height as usize;
        let cols = req.width as usize;
        let total_now = self.total_hist.back().copied().unwrap_or(0);

        let mut lines: Vec<Line> = Vec::new();
        if rows >= 1 && cols > 0 {
            // Build one line with right-aligned per-core segments (simplified)
            let mut text = String::new();
            let cores = self.cores;
            for core_idx in 0..cores {
                let usage = self
                    .per_core_hist
                    .get(core_idx)
                    .and_then(|h| h.back().copied())
                    .unwrap_or(0);
                let seg = format!("C{core_idx:02} {usage:>3}% ");
                text.push_str(&seg);
            }
            if text.is_empty() {
                lines.push(Line(vec![Span {
                    text: "CPU --%".to_string(),
                    color: Some(Color::Gray),
                }]));
            } else {
                lines.push(Line(vec![Span {
                    text,
                    color: Some(Color::Gray),
                }]));
            }
        }
        if rows >= 2 && cols > 0 {
            let label = format!("{total_now:>3}%");
            let color = Self::color_for(total_now);
            lines.push(Line(vec![Span {
                text: label,
                color: Some(color),
            }]));
        }

        if lines.is_empty() {
            RenderTree::Lines(vec![Line(vec![Span {
                text: "CPU".to_string(),
                color: Some(Color::Gray),
            }])])
        } else {
            RenderTree::Group(vec![RenderTree::Lines(lines)])
        }
    }
}

dx_overlay!(CpuOverlay);
