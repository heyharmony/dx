use crate::markdown::markdown_to_text_with_links;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Text;
use ratatui::widgets::{Paragraph, Wrap};
use std::fs;
use std::path::{Path, PathBuf};

#[must_use]
pub fn read_motd_file(path: &Path) -> Option<(Vec<String>, bool)> {
    match fs::read_to_string(path) {
        Ok(s) => {
            let mut force_raw = false;
            let mut filtered: Vec<String> = Vec::new();
            for line in s.lines() {
                let ll = line.to_lowercase();
                if ll.contains("dx:ascii")
                    || ll.contains("ascii_art_mode")
                    || ll.contains("no_markdown")
                {
                    force_raw = true;
                    continue;
                }
                filtered.push(line.to_string());
            }
            Some((filtered, force_raw))
        }
        Err(_) => None,
    }
}

#[must_use]
pub fn find_motd_in_ancestors() -> Option<PathBuf> {
    let mut cur = std::env::current_dir().ok()?;
    loop {
        let candidate = cur.join("MOTD.md");
        if candidate.exists() {
            return Some(candidate);
        }
        if let Some(parent) = cur.parent() {
            cur = parent.to_path_buf();
        } else {
            break;
        }
    }
    None
}

// Build a colored system banner and prepend it to existing MOTD lines.
// Heading is highlighted; each item is listed on a separate line with a dash prefix.
#[must_use]
pub fn prepend_system_banner(
    motd_lines: Vec<String>,
    heading: &str,
    items: &[String],
) -> Vec<String> {
    let mut banner: Vec<String> = Vec::new();
    banner.push("dx:ascii".to_string()); // force raw rendering to preserve colors
    banner.push(format!("\x1b[33;1m{heading}\x1b[0m"));
    for s in items {
        banner.push(format!("- {s}"));
    }
    banner.push(String::new());
    let mut combined: Vec<String> = Vec::new();
    combined.extend(banner);
    combined.extend(motd_lines);
    combined
}

// Render MOTD area, honoring dx:ascii detection (force_raw) and wrap/markdown toggles.
#[allow(clippy::too_many_arguments)]
pub fn render_motd(
    f: &mut Frame,
    area: Rect,
    motd_lines: &[String],
    markdown_enabled: bool,
    wrap_enabled: bool,
    force_raw: bool,
    color: Option<Color>,
    theme: &crate::theme::ThemeTokens,
) {
    let motd_content = motd_lines.join("\n");
    if markdown_enabled && wrap_enabled && !force_raw {
        let (motd_text, _links) = markdown_to_text_with_links(&motd_content, theme);
        let mut motd_para = Paragraph::new(motd_text).wrap(Wrap { trim: false });
        if let Some(c) = color {
            motd_para = motd_para.style(Style::default().fg(c));
        }
        f.render_widget(motd_para, area);
    } else {
        let mut motd_para = Paragraph::new(Text::from(motd_content));
        if let Some(c) = color {
            motd_para = motd_para.style(Style::default().fg(c));
        }
        f.render_widget(motd_para, area);
    }
}
