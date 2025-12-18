// TODO: Fix clippy warnings for better code quality
#![allow(clippy::collapsible_if)] // TODO: Simplify nested if statements
#![allow(clippy::uninlined_format_args)] // TODO: Use {var} format syntax

use ratatui::style::Color;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct ThemeTokens {
    pub text_primary: Color,
    pub text_muted: Color,
    pub border: Color,
    pub surface: Color,
    pub surface_alt: Color,

    pub menu_title: Color,
    pub menu_action: Color,
    pub menu_desc: Color,

    pub link: Color,
    pub code: Color,
    pub md_heading1: Color,
    pub md_heading2: Color,
    pub md_heading3: Color,
    pub rule: Color,

    pub status_fg: Color,
    pub status_bg: Color,

    pub accent_success: Color,
    pub accent_warning: Color,
    pub accent_danger: Color,

    pub selection_fg: Color,
    pub selection_bg: Color,
}

impl ThemeTokens {
    #[allow(dead_code)]
    #[must_use]
    pub fn default_theme() -> Self {
        ThemeTokens::builtin_dark()
    }
}

impl Default for ThemeTokens {
    fn default() -> Self {
        Self::builtin_dark()
    }
}

impl ThemeTokens {
    #[must_use]
    pub fn builtin_dark() -> Self {
        Self {
            text_primary: Color::White,
            text_muted: Color::Gray,
            border: Color::DarkGray,
            surface: Color::Rgb(16, 16, 16),
            surface_alt: Color::Rgb(24, 24, 24),

            menu_title: Color::White,
            menu_action: Color::Gray,
            menu_desc: Color::DarkGray,

            link: Color::Blue,
            code: Color::Cyan,
            md_heading1: Color::Yellow,
            md_heading2: Color::LightYellow,
            md_heading3: Color::Green,
            rule: Color::DarkGray,

            status_fg: Color::White,
            status_bg: Color::Rgb(24, 24, 24),

            accent_success: Color::Green,
            accent_warning: Color::Yellow,
            accent_danger: Color::Red,

            selection_fg: Color::White,
            selection_bg: Color::Rgb(24, 24, 24),
        }
    }

    #[must_use]
    pub fn builtin_light() -> Self {
        Self {
            text_primary: Color::Black,
            text_muted: Color::DarkGray,
            border: Color::Gray,
            surface: Color::Rgb(245, 245, 245),
            surface_alt: Color::Rgb(230, 230, 230),

            menu_title: Color::Black,
            menu_action: Color::DarkGray,
            menu_desc: Color::Gray,

            link: Color::Blue,
            code: Color::Cyan,
            md_heading1: Color::Yellow,
            md_heading2: Color::LightYellow,
            md_heading3: Color::Green,
            rule: Color::DarkGray,

            status_fg: Color::Black,
            status_bg: Color::Rgb(230, 230, 230),

            accent_success: Color::Green,
            accent_warning: Color::Yellow,
            accent_danger: Color::Red,

            selection_fg: Color::Black,
            selection_bg: Color::Rgb(230, 230, 230),
        }
    }

    fn apply_token(&mut self, key: &str, value: &str) {
        if let Some(color) = parse_color(value) {
            match key {
                "text_primary" => self.text_primary = color,
                "text_muted" => self.text_muted = color,
                "border" => self.border = color,
                "surface" => self.surface = color,
                "surface_alt" => self.surface_alt = color,
                "menu_title" => self.menu_title = color,
                "menu_action" => self.menu_action = color,
                "menu_desc" => self.menu_desc = color,
                "link" => self.link = color,
                "code" => self.code = color,
                "md_heading1" => self.md_heading1 = color,
                "md_heading2" => self.md_heading2 = color,
                "md_heading3" => self.md_heading3 = color,
                "rule" => self.rule = color,
                "status_fg" => self.status_fg = color,
                "status_bg" => self.status_bg = color,
                "accent_success" => self.accent_success = color,
                "accent_warning" => self.accent_warning = color,
                "accent_danger" => self.accent_danger = color,
                "selection_fg" => self.selection_fg = color,
                "selection_bg" => self.selection_bg = color,
                _ => {}
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct FileTheme {
    #[serde(default)]
    tokens: HashMap<String, String>,
}

#[allow(dead_code)]
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

#[allow(dead_code)]
#[must_use]
pub fn load_theme_from<S: ::std::hash::BuildHasher>(
    name: Option<&str>,
    file: Option<&str>,
    overrides: Option<&HashMap<String, String, S>>,
) -> ThemeTokens {
    // Base: builtin by name (default dark)
    let mut tokens = match name.map(str::to_ascii_lowercase) {
        Some(n) if n == "light" => ThemeTokens::builtin_light(),
        _ => ThemeTokens::builtin_dark(),
    };

    // Optional: external .dx-theme YAML file
    if let Some(p) = file {
        if p.ends_with(".dx-theme") {
            let path = PathBuf::from(expand_tilde(p));
            if let Ok(s) = fs::read_to_string(&path) {
                if let Ok(ft) = serde_yaml::from_str::<FileTheme>(&s) {
                    for (k, v) in &ft.tokens {
                        tokens.apply_token(k, v);
                    }
                }
            }
        }
    }

    // Inline overrides from config
    if let Some(map) = overrides {
        for (k, v) in map {
            tokens.apply_token(k, v);
        }
    }

    tokens
}

#[must_use]
pub fn parse_color(spec: &str) -> Option<Color> {
    let s = spec.trim();
    let hex = if let Some(h) = s.strip_prefix('#') {
        h
    } else {
        s
    };
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return Some(Color::Rgb(r, g, b));
        }
    }
    match s.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightyellow" => Some(Color::LightYellow),
        "lightblue" => Some(Color::LightBlue),
        "lightmagenta" => Some(Color::LightMagenta),
        "lightcyan" => Some(Color::LightCyan),
        "white" => Some(Color::White),
        _ => None,
    }
}
