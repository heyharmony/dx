// TODO: Fix clippy warnings for better code quality
#![allow(clippy::uninlined_format_args)] // TODO: Use {var} format syntax
#![allow(clippy::collapsible_else_if)] // TODO: Simplify nested if-else chains

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};
use std::io::Write;
use std::time::Instant;

use crate::frame;
use crate::markdown::markdown_to_text_with_links;
use crate::menu::load_menu;
use crate::{App, PAD_X, PAD_Y, SPINNER_FRAMES, Screen};
use crate::{centered_rect_fixed, format_duration};
use ansi_to_tui::IntoText;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if let Screen::Output(view) = &mut app.screen {
        // Update viewport height and scroller
        let border_h: u16 = 2;
        let inner_h = area
            .height
            .saturating_sub(border_h + PAD_Y.saturating_mul(2));
        view.viewport_height = inner_h;
        view.scroller.set_viewport(inner_h);

        if let Some(md) = &view.md_content {
            let _vh = view.viewport_height.max(1);
            if app.markdown_enabled && view.render_markdown {
                let (text, links) = markdown_to_text_with_links(md, &app.theme);
                view.md_links = links;
                let total_lines = text.lines.len() as u16;
                view.md_footnote_start = if !view.md_links.is_empty() {
                    Some(total_lines.saturating_sub((view.md_links.len() as u16).saturating_add(2)))
                } else {
                    None
                };
                view.scroller.set_total(total_lines);
                if view.auto_scroll {
                    view.scroller.end();
                }
                view.scroll_y = view.scroller.scroll_y;
                let paragraph = if view.wrap_enabled {
                    Paragraph::new(text)
                        .wrap(ratatui::widgets::Wrap { trim: false })
                        .scroll((view.scroll_y, 0))
                } else {
                    Paragraph::new(text).scroll((view.scroll_y, 0))
                };
                let paragraph = paragraph.block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(view.title.clone())
                        .padding(Padding::new(PAD_X, PAD_X, PAD_Y, PAD_Y)),
                );
                f.render_widget(paragraph, area);
            } else {
                let total_lines = md.lines().count() as u16;
                view.scroller.set_total(total_lines);
                if view.auto_scroll {
                    view.scroller.end();
                }
                view.scroll_y = view.scroller.scroll_y;
                let paragraph = if view.wrap_enabled {
                    Paragraph::new(Text::from(md.clone()))
                        .wrap(ratatui::widgets::Wrap { trim: false })
                        .scroll((view.scroll_y, 0))
                } else {
                    Paragraph::new(Text::from(md.clone())).scroll((view.scroll_y, 0))
                };
                let paragraph = paragraph.block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(view.title.clone())
                        .padding(Padding::new(PAD_X, PAD_X, PAD_Y, PAD_Y)),
                );
                f.render_widget(paragraph, area);
            }
        } else {
            if let Some(term) = &view.term {
                let lines = term.render_lines();
                let total_lines = lines.len() as u16;
                view.scroller.set_total(total_lines);
                if view.auto_scroll {
                    view.scroller.end();
                }
                view.scroll_y = view.scroller.scroll_y;
                let colored_text: Text = match lines.join("\n").into_text() {
                    Ok(t) => t,
                    Err(_) => Text::from(lines.join("\n")),
                };
                let mut title_spans = vec![Span::raw(view.title.clone())];
                if view.running {
                    title_spans.push(Span::raw(" "));
                    title_spans.push(Span::styled(
                        "● running",
                        Style::default().fg(Color::Yellow),
                    ));
                } else if let Some(code) = view.exit_status {
                    title_spans.push(Span::raw(" "));
                    if code == 0 {
                        title_spans.push(Span::styled(
                            format!("✔ {}", code),
                            Style::default().fg(Color::Green),
                        ));
                    } else {
                        title_spans.push(Span::styled(
                            format!("✖ {}", code),
                            Style::default().fg(Color::Red),
                        ));
                    }
                } else {
                    title_spans.push(Span::raw(" "));
                    title_spans.push(Span::styled("⛔", Style::default().fg(Color::Magenta)));
                }
                if view.auto_scroll {
                    title_spans.push(Span::raw(" [AUTO]"));
                }
                let title_line = Line::from(title_spans);
                let mut paragraph = Paragraph::new(colored_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(title_line)
                            .padding(Padding::new(PAD_X, PAD_X, PAD_Y, PAD_Y)),
                    )
                    .scroll((view.scroll_y, 0));
                if app.output_dim {
                    paragraph = paragraph.style(Style::default().add_modifier(Modifier::DIM));
                }
                f.render_widget(paragraph, area);
            } else {
                let mut combined: Vec<String> = view.lines.clone();
                if let Some(p) = &view.pending_line {
                    combined.push(p.clone());
                }
                if !view.input_buffer.is_empty() {
                    combined.push(format!("> {}", view.input_buffer));
                }
                let total_lines = combined.len() as u16;
                view.scroller.set_total(total_lines);
                if view.auto_scroll {
                    view.scroller.end();
                }
                view.scroll_y = view.scroller.scroll_y;
                let colored_text: Text = match combined.join("\n").into_text() {
                    Ok(t) => t,
                    Err(_) => Text::from(combined.join("\n")),
                };
                let mut title_spans = vec![Span::raw(view.title.clone())];
                if view.running {
                    title_spans.push(Span::raw(" "));
                    title_spans.push(Span::styled(
                        "● running",
                        Style::default().fg(Color::Yellow),
                    ));
                } else if let Some(code) = view.exit_status {
                    title_spans.push(Span::raw(" "));
                    if code == 0 {
                        title_spans.push(Span::styled(
                            format!("✔ {}", code),
                            Style::default().fg(Color::Green),
                        ));
                    } else {
                        title_spans.push(Span::styled(
                            format!("✖ {}", code),
                            Style::default().fg(Color::Red),
                        ));
                    }
                } else {
                    title_spans.push(Span::raw(" "));
                    title_spans.push(Span::styled("⛔", Style::default().fg(Color::Magenta)));
                }
                if view.auto_scroll {
                    title_spans.push(Span::raw(" [AUTO]"));
                }
                let title_line = Line::from(title_spans);
                let mut paragraph = Paragraph::new(colored_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(title_line)
                            .padding(Padding::new(PAD_X, PAD_X, PAD_Y, PAD_Y)),
                    )
                    .scroll((view.scroll_y, 0));
                if app.output_dim {
                    paragraph = paragraph.style(Style::default().add_modifier(Modifier::DIM));
                }
                f.render_widget(paragraph, area);
            }
        }

        if let Some(start) = view.started_at {
            let elapsed = if let Some(end) = view.ended_at {
                end.saturating_duration_since(start)
            } else {
                Instant::now().saturating_duration_since(start)
            };
            let timer_str = format_duration(elapsed);
            let status_span = if view.running {
                Span::styled("●", Style::default().fg(Color::Yellow))
            } else if let Some(code) = view.exit_status {
                if code == 0 {
                    Span::styled("✔", Style::default().fg(Color::Green))
                } else {
                    Span::styled("✖", Style::default().fg(Color::Red))
                }
            } else {
                Span::styled("⛔", Style::default().fg(Color::Magenta))
            };
            let spinner = if view.running {
                SPINNER_FRAMES.get(view.spinner_idx).copied().unwrap_or("⠋")
            } else {
                ""
            };
            let mut overlay_spans = vec![status_span, Span::raw(" "), Span::raw(timer_str)];
            if !spinner.is_empty() {
                overlay_spans.push(Span::raw(" "));
                overlay_spans.push(Span::raw(spinner));
            }
            let line = Line::from(overlay_spans);
            let overlay_text = Text::from(line.clone());
            let inner = Rect {
                x: area.x.saturating_add(1 + PAD_X),
                y: area.y.saturating_add(1 + PAD_Y),
                width: area.width.saturating_sub(2 + PAD_X * 2),
                height: area.height.saturating_sub(2 + PAD_Y * 2),
            };
            let w = line.width() as u16;
            let h: u16 = 1;
            if inner.width >= w && inner.height >= h {
                let x = inner.x + inner.width.saturating_sub(w);
                let y = inner.y + inner.height.saturating_sub(h);
                let r = Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                };
                let overlay = Paragraph::new(overlay_text);
                f.render_widget(Clear, r);
                f.render_widget(overlay, r);
            }
        }

        if let Some(crate::Confirm::KillProcess { yes_selected }) = app.confirm {
            let title = "Confirm";
            let line1 = "Stop the running process?";
            
            // Create selectable buttons with visual indication
            let yes_text = if yes_selected { "[YES]" } else { " Yes " };
            let no_text = if !yes_selected { "[NO]" } else { " No " };
            let line2 = format!("{}  {}", yes_text, no_text);
            let line3 = "← → to select, Enter to confirm, Esc to cancel";
            
            let content_w = [line1.len(), line2.len(), line3.len()].iter().max().unwrap_or(&0) + 2;
            let w = (content_w as u16)
                .saturating_add(4)
                .min(area.width.saturating_sub(2));
            let h: u16 = 4;
            let marea = centered_rect_fixed(w, h, area);
            let msg = format!("{}\n{}\n{}", line1, line2, line3);
            frame::render_modal(title, &msg, marea, f);
        }
    }
}

pub fn handle_event(app: &mut App, key: KeyEvent) -> Result<bool> {
    if let Screen::Output(view) = &mut app.screen {
        match (key.code, key.modifiers) {
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                app.selection_mode = !app.selection_mode;
                if app.selection_mode && app.mouse_captured {
                    let mut out = std::io::stdout();
                    let _ = crossterm::execute!(out, crossterm::event::DisableMouseCapture);
                    app.mouse_captured = false;
                } else if !app.selection_mode && !app.mouse_captured {
                    let mut out = std::io::stdout();
                    let _ = crossterm::execute!(out, crossterm::event::EnableMouseCapture);
                    app.mouse_captured = true;
                }
            }
            (KeyCode::Esc, _) => {
                if (app.child.is_some() || app.pty_child.is_some()) && view.running {
                    app.confirm = Some(crate::Confirm::KillProcess { yes_selected: false });
                } else if let Some(prev) = app.screen_stack.pop() {
                    app.screen = prev;
                    app.needs_clear = true;
                } else if let Some(menu_path) = &app.menu_path {
                    if let Ok(menu) = load_menu(menu_path) {
                        app.screen = Screen::Menu(menu);
                        app.needs_clear = true;
                    }
                } else {
                    return Ok(true);
                }
            }
            (KeyCode::Char('q'), _) => {
                if (app.child.is_some() || app.pty_child.is_some()) && view.running {
                    app.confirm = Some(crate::Confirm::KillProcess { yes_selected: false });
                } else if let Some(prev) = app.screen_stack.pop() {
                    app.screen = prev;
                    app.needs_clear = true;
                } else if let Some(menu_path) = &app.menu_path {
                    if let Ok(menu) = load_menu(menu_path) {
                        app.screen = Screen::Menu(menu);
                        app.needs_clear = true;
                    }
                } else {
                    return Ok(true);
                }
            }
            (KeyCode::Char('d'), _) => {
                app.output_dim = !app.output_dim;
            }
            (KeyCode::Char('b'), _) | (KeyCode::Backspace, _) => {
                if !view.input_buffer.is_empty() {
                    view.input_buffer.pop();
                } else if let Some(menu_path) = &app.menu_path {
                    if let Ok(menu) = load_menu(menu_path) {
                        app.screen = Screen::Menu(menu);
                        app.needs_clear = true;
                    }
                } else {
                    return Ok(true);
                }
            }
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if let Some(child) = &mut app.child {
                    let _ = child.kill();
                }
                crate::exec::pty_write(&mut app.pty_writer, &[0x03]);
            }
            (KeyCode::Enter, _) => {
                if app.child.is_some() {
                    if let Some(stdin) = &mut app.child_stdin {
                        let to_send = if view.input_buffer.is_empty() {
                            "\n".to_string()
                        } else {
                            format!("{}\n", view.input_buffer)
                        };
                        let _ = stdin.write_all(to_send.as_bytes());
                        let _ = stdin.flush();
                    }
                    view.input_buffer.clear();
                }
                crate::exec::pty_write(&mut app.pty_writer, b"\n");
            }
            (KeyCode::Char(c), m) if m.is_empty() => {
                if app.child.is_some() {
                    view.input_buffer.push(c);
                }
                crate::exec::pty_write(&mut app.pty_writer, &[c as u8]);
            }
            // handled above with (KeyCode::Char('b'), _) | (KeyCode::Backspace, _)
            (KeyCode::Tab, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\t");
            }
            (KeyCode::Delete, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[3~");
            }
            (KeyCode::Up, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[A");
            }
            (KeyCode::Down, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[B");
            }
            (KeyCode::Left, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[D");
            }
            (KeyCode::Right, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[C");
            }
            (KeyCode::Home, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[H");
            }
            (KeyCode::End, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[F");
            }
            (KeyCode::PageUp, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[5~");
            }
            (KeyCode::PageDown, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[6~");
            }
            (KeyCode::Insert, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[2~");
            }
            (KeyCode::BackTab, _) => {
                crate::exec::pty_write(&mut app.pty_writer, b"\x1b[Z");
            }
            (KeyCode::F(n), _) => {
                if app.pty_writer.is_some() {
                    let seq: Option<&'static [u8]> = match n {
                        1 => Some(b"\x1bOP"),
                        2 => Some(b"\x1bOQ"),
                        3 => Some(b"\x1bOR"),
                        4 => Some(b"\x1bOS"),
                        5 => Some(b"\x1b[15~"),
                        6 => Some(b"\x1b[17~"),
                        7 => Some(b"\x1b[18~"),
                        8 => Some(b"\x1b[19~"),
                        9 => Some(b"\x1b[20~"),
                        10 => Some(b"\x1b[21~"),
                        11 => Some(b"\x1b[23~"),
                        12 => Some(b"\x1b[24~"),
                        _ => None,
                    };
                    if let Some(s) = seq {
                        crate::exec::pty_write(&mut app.pty_writer, s);
                    }
                }
            }
            (KeyCode::Char(c), m)
                if m.contains(KeyModifiers::ALT) && !m.contains(KeyModifiers::CONTROL) =>
            {
                let buf = vec![0x1b, c as u8];
                crate::exec::pty_write(&mut app.pty_writer, &buf);
            }
            (KeyCode::Char(c), m)
                if m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
            {
                let mut code = c as u8;
                if c.is_ascii_lowercase() || c.is_ascii_uppercase() {
                    code = (c.to_ascii_lowercase() as u8) & 0x1f;
                }
                crate::exec::pty_write(&mut app.pty_writer, &[code]);
            }
            (KeyCode::Char('k'), _) => {
                if view.term.is_none() {
                    view.scroller.line_up();
                    view.scroll_y = view.scroller.scroll_y;
                    view.auto_scroll = false;
                }
            }
            (KeyCode::Char('j'), _) => {
                if view.term.is_none() {
                    view.scroller.line_down();
                    view.scroll_y = view.scroller.scroll_y;
                    view.auto_scroll = false;
                }
            }
            (KeyCode::Char('g'), _) => {
                if view.term.is_none() {
                    view.scroller.home();
                    view.scroll_y = view.scroller.scroll_y;
                    view.auto_scroll = false;
                }
            }
            (KeyCode::Char('G'), _) => {
                if view.term.is_none() {
                    view.scroller.end();
                    view.scroll_y = view.scroller.scroll_y;
                    view.auto_scroll = true;
                }
            }
            (KeyCode::Char('a'), _) => {
                view.auto_scroll = !view.auto_scroll;
                view.scroller.set_auto(view.auto_scroll);
                view.scroll_y = view.scroller.scroll_y;
            }
            (KeyCode::Char('m'), _) => {
                if view.md_content.is_some() {
                    view.render_markdown = !view.render_markdown;
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

// Optional View trait adapter for testing/extensibility
use crate::view::View;

pub struct OutputViewAdapter;

impl View for OutputViewAdapter {
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
