use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use sysinfo::{CpuRefreshKind, RefreshKind, System};

const HISTORY_SECS: usize = 60; // ~60s window
const SAMPLE_MS: u64 = 500; // 0.5s sampling

#[derive(Debug, Clone)]
pub struct CpuSample {
    pub timestamp: Instant,
    pub total: u8,         // 0..=100
    pub per_core: Vec<u8>, // 0..=100
}

#[derive(Debug)]
pub struct CpuOverlayState {
    pub visible: bool,
    pub total_hist: VecDeque<u8>,
    pub per_core_hist: Vec<VecDeque<u8>>, // [core][time]
    pub cores: usize,
    last_redraw_at: Instant,
}

impl Default for CpuOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuOverlayState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            visible: true,
            total_hist: VecDeque::with_capacity(HISTORY_SECS * 2),
            per_core_hist: Vec::new(),
            cores: 0,
            last_redraw_at: Instant::now(),
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn apply_sample(&mut self, s: &CpuSample) {
        if self.cores != s.per_core.len() {
            self.cores = s.per_core.len();
            self.per_core_hist = (0..self.cores)
                .map(|_| VecDeque::with_capacity(HISTORY_SECS * 2))
                .collect();
        }
        push_bounded(&mut self.total_hist, s.total, HISTORY_SECS * 2);
        for (i, &v) in s.per_core.iter().enumerate() {
            if let Some(h) = self.per_core_hist.get_mut(i) {
                push_bounded(h, v, HISTORY_SECS * 2);
            }
        }
        self.last_redraw_at = s.timestamp;
    }
}

fn push_bounded(buf: &mut VecDeque<u8>, v: u8, cap: usize) {
    if buf.len() == cap {
        let _ = buf.pop_front();
    }
    buf.push_back(v);
}

#[must_use]
pub fn start_sampler() -> tokio::sync::mpsc::Receiver<CpuSample> {
    let (tx, rx) = tokio::sync::mpsc::channel::<CpuSample>(128);
    tokio::spawn(async move {
        let mut sys =
            System::new_with_specifics(RefreshKind::new().with_cpu(CpuRefreshKind::everything()));
        let mut last = Instant::now();
        let mut ticker = tokio::time::interval(Duration::from_millis(SAMPLE_MS));
        loop {
            ticker.tick().await;
            sys.refresh_cpu_specifics(CpuRefreshKind::everything());
            let mut per: Vec<u8> = Vec::new();
            for cpu in sys.cpus() {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                per.push((cpu.cpu_usage().round() as u8).min(100));
            }
            let total = if per.is_empty() {
                0
            } else {
                #[allow(clippy::cast_possible_truncation)]
                let len = per.len() as u32;
                (per.iter().map(|&v| u32::from(v)).sum::<u32>() / len).min(100) as u8
            };
            let _ = tx.try_send(CpuSample {
                timestamp: last,
                total,
                per_core: per,
            });
            last = Instant::now();
        }
    });
    rx
}

// removed unused helper to keep build warning-free

pub fn render(f: &mut Frame, area: Rect, state: &CpuOverlayState) {
    if !state.visible {
        return;
    }
    f.render_widget(Clear, area);

    let cols = area.width as usize;
    let rows = area.height as usize;

    let total_now = state.total_hist.back().copied().unwrap_or(0);

    // Prepare buffers for up to 2 rows
    let mut row_chars: Vec<Vec<char>> = vec![vec![' '; cols]; rows.max(2)];
    let mut row_colors: Vec<Vec<Option<Color>>> = vec![vec![None; cols]; rows.max(2)];

    // Row 0: right-aligned per-core list as text segments: "Cxx xxx%  Cyy yyy%  ..."
    if rows >= 1 && cols > 0 {
        #[allow(clippy::cast_possible_wrap)]
        let mut x_cursor: isize = cols as isize; // right edge (exclusive)
        let cores = state.cores;
        for core_idx in 0..cores {
            let usage = state
                .per_core_hist
                .get(core_idx)
                .and_then(|h| h.back().copied())
                .unwrap_or(0);
            let seg = format!("C{core_idx:02} {usage:>3}%");
            #[allow(clippy::cast_possible_wrap)]
            let seg_len = seg.len() as isize;
            let next_x = x_cursor - seg_len;
            if next_x < 0 {
                break;
            }
            // Write segment chars with coloring: label gray, percentage colored
            for (i, ch) in seg.chars().enumerate() {
                #[allow(clippy::cast_sign_loss)]
                let x = (next_x as usize) + i;
                row_chars[0][x] = ch;
                if i < 3 {
                    row_colors[0][x] = Some(Color::Gray);
                } else {
                    row_colors[0][x] = Some(color_for(usage));
                }
            }
            // One space between segments
            x_cursor = next_x - 1;
            if x_cursor <= 0 {
                break;
            }
        }
    }

    // Row 1: total percent label on the right and a right-to-left bar to its left
    if rows >= 2 && cols > 0 {
        let pct_label = format!("{total_now:>3}%");
        let label_len = pct_label.len();
        let label_start = cols.saturating_sub(label_len);
        for (i, ch) in pct_label.chars().enumerate() {
            let x = label_start + i;
            if x < cols {
                row_chars[1][x] = ch;
                row_colors[1][x] = Some(color_for(total_now));
            }
        }
        // Bar area is everything left of a single space before the label
        let bar_end = label_start.saturating_sub(1); // leave one space gap
        let bar_width = bar_end;
        if bar_width > 0 {
            #[allow(clippy::cast_possible_truncation)]
            let filled = ((u32::try_from(bar_width).unwrap_or(0) * u32::from(total_now)) / 100) as usize;
            // Fill from right to left with '█' for filled, '·' for empty
            for i in 0..bar_width {
                let x = bar_end.saturating_sub(1).saturating_sub(i);
                if i < filled {
                    row_chars[1][x] = '█';
                    row_colors[1][x] = Some(color_for(total_now));
                } else {
                    row_chars[1][x] = '·';
                    row_colors[1][x] = Some(Color::DarkGray);
                }
            }
        }
    }

    // Convert rows to Lines grouping contiguous same-color segments
    let mut lines: Vec<Line> = Vec::new();
    for y in 0..rows.min(2) {
        let mut spans: Vec<Span> = Vec::new();
        let mut cur_color: Option<Color> = None;
        let mut cur_text: String = String::new();
        let flush =
            |spans: &mut Vec<Span>, cur_text: &mut String, cur_color: &mut Option<Color>| {
                if !cur_text.is_empty() {
                    let s = std::mem::take(cur_text);
                    match cur_color {
                        Some(c) => spans.push(Span::styled(s, Style::default().fg(*c))),
                        None => spans.push(Span::raw(s)),
                    }
                }
            };
        for x in 0..cols {
            let ch = row_chars[y][x];
            let col = row_colors[y][x];
            if col != cur_color {
                flush(&mut spans, &mut cur_text, &mut cur_color);
                cur_color = col;
            }
            cur_text.push(ch);
        }
        flush(&mut spans, &mut cur_text, &mut cur_color);
        lines.push(Line::from(spans));
    }

    f.render_widget(Paragraph::new(lines), area);
}

fn color_for(v: u8) -> Color {
    match v {
        0..=39 => Color::Green,
        40..=79 => Color::Yellow,
        _ => Color::Red,
    }
}
