// TODO: Fix clippy warnings for better code quality
#![allow(clippy::uninlined_format_args)] // TODO: Use {var} format syntax

use crate::App;
use crate::Screen;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if let Screen::Config(cfg) = &mut app.screen {
        let title = "Configuration (Save: s, Toggle: keys, Esc/q back)";
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(format!(
            "Path: {}{}",
            cfg.path.display(),
            if cfg.is_global {
                " (global)"
            } else {
                " (local)"
            }
        )));
        lines.push(Line::from(""));
        fn bool_str(b: Option<bool>) -> String {
            match b {
                Some(true) => "true".into(),
                Some(false) => "false".into(),
                None => "<inherit>".into(),
            }
        }
        let s1 = format!("[1] motd_wrap: {}", bool_str(cfg.cfg.motd_wrap));
        let s2 = format!(
            "[2] markdown_enabled: {}",
            bool_str(cfg.cfg.markdown_enabled)
        );
        let s3 = format!("[3] output_dim: {}", bool_str(cfg.cfg.output_dim));
        let s4 = format!(
            "[4] theme: {}",
            cfg.cfg.theme.clone().unwrap_or_else(|| "<inherit>".into())
        );
        let s5 = format!(
            "[5] telemetry.enabled: {}",
            cfg.cfg
                .telemetry
                .as_ref()
                .map(|t| t.enabled)
                .unwrap_or(false)
        );
        let s6 = format!(
            "[6] asciinema.enabled: {}",
            cfg.cfg
                .asciinema
                .as_ref()
                .map(|a| a.enabled)
                .unwrap_or(false)
        );
        let s7 = format!(
            "     motd_color: {}",
            cfg.cfg
                .motd_color
                .clone()
                .unwrap_or_else(|| "<inherit>".into())
        );
        lines.push(Line::from(s1));
        lines.push(Line::from(s2));
        lines.push(Line::from(s3));
        lines.push(Line::from(s4));
        lines.push(Line::from(s5));
        lines.push(Line::from(s6));
        lines.push(Line::from(s7));
        lines.push(Line::from(""));
        lines.push(Line::from("Keys: 1-3 toggle booleans, t cycle theme, e toggle telemetry, a toggle asciinema, s save"));
        if let Some(msg) = &cfg.message {
            lines.push(Line::from(msg.clone()));
        }
        let text = Text::from(lines);
        let paragraph = Paragraph::new(text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .padding(Padding::new(2, 2, 1, 1)),
        );
        f.render_widget(paragraph, area);
    }
}

use crate::menu::load_menu;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
// use crate::config::open_config_state;
// use std::path::PathBuf;

pub fn handle_event(app: &mut App, key: KeyEvent) -> Result<bool> {
    if let Screen::Config(cfg) = &mut app.screen {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('q'), _) => {
                if let Some(menu_path) = &app.menu_path {
                    if let Ok(menu) = load_menu(menu_path) {
                        app.screen = Screen::Menu(menu);
                        app.needs_clear = true;
                    }
                } else {
                    return Ok(true);
                }
            }
            (KeyCode::Char('s'), _) => {
                if let Err(e) = save_app_config(&cfg.path, &cfg.cfg) {
                    cfg.message = Some(format!("Save failed: {e}"));
                } else {
                    cfg.message = Some("Saved.".to_string());
                }
            }
            (KeyCode::Char('1'), _) => {
                cfg.cfg.motd_wrap = Some(!cfg.cfg.motd_wrap.unwrap_or(true));
            }
            (KeyCode::Char('2'), _) => {
                cfg.cfg.markdown_enabled = Some(!cfg.cfg.markdown_enabled.unwrap_or(true));
            }
            (KeyCode::Char('3'), _) => {
                cfg.cfg.output_dim = Some(!cfg.cfg.output_dim.unwrap_or(true));
            }
            (KeyCode::Char('t'), _) => {
                cfg.cfg.theme = Some(match cfg.cfg.theme.as_deref() {
                    Some("dark") => "light".into(),
                    _ => "dark".into(),
                });
            }
            (KeyCode::Char('e'), _) => {
                let mut t = cfg.cfg.telemetry.clone().unwrap_or(crate::TelemetryConfig {
                    enabled: false,
                    endpoint: None,
                });
                t.enabled = !t.enabled;
                cfg.cfg.telemetry = Some(t);
            }
            (KeyCode::Char('a'), _) => {
                let mut a =
                    cfg.cfg
                        .asciinema
                        .clone()
                        .unwrap_or(crate::asciinema::AsciinemaConfig {
                            enabled: false,
                            external: false,
                            on_relaunch: false,
                            dir: None,
                            file_prefix: None,
                            title: None,
                            quiet: false,
                            overwrite: false,
                            stream: false,
                            stream_mode: crate::asciinema::default_stream_mode(),
                            local_addr: None,
                            remote: None,
                        });
                a.enabled = !a.enabled;
                cfg.cfg.asciinema = Some(a);
            }
            _ => {}
        }
    }
    Ok(false)
}

use crate::config::save_app_config;

// Optional View trait adapter for testing/extensibility
use crate::view::View;

pub struct ConfigViewAdapter;

impl View for ConfigViewAdapter {
    fn render(
        &mut self,
        f: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        app: &mut crate::App,
    ) {
        render(f, area, app);
    }

    fn handle_event(
        &mut self,
        app: &mut crate::App,
        key: crossterm::event::KeyEvent,
    ) -> anyhow::Result<bool> {
        handle_event(app, key)
    }
}
