// TODO: Fix clippy warnings for better code quality
#![allow(clippy::uninlined_format_args)] // TODO: Use {var} format syntax

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, List, ListItem, Padding};
use std::path::Path;

use crate::menu::{MenuItem, submenu_at, build_terminal_alias};
use crate::theme::ThemeTokens;
use crate::{App, PAD_X, PAD_Y, Screen};
// use crate::menu::MenuItem;
use crate::{
    open_config_state, open_file_view, passthrough_command, start_command, start_command_enhanced,
};

fn count_folders_and_cmds(item: &MenuItem) -> (usize, usize) {
    // Count recursively: folders = nodes with non-empty children; cmds = nodes with Some(cmd)
    fn dfs(it: &MenuItem, acc: &mut (usize, usize)) {
        if !it.items.is_empty() {
            acc.0 = acc.0.saturating_add(1);
        }
        if it.cmd.is_some() {
            acc.1 = acc.1.saturating_add(1);
        }
        for c in &it.items {
            dfs(c, acc);
        }
    }
    let mut acc = (0usize, 0usize);
    for c in &item.items {
        dfs(c, &mut acc);
    }
    acc
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if let Screen::Menu(menu) = &mut app.screen {
        let current = submenu_at(&menu.items, &menu.path);
        if menu.selected_index >= current.len() {
            menu.selected_index = current.len().saturating_sub(1);
        }
        let mut list_items: Vec<ListItem> = Vec::new();
        // Build items without borrowing app across loop body
        let theme = app.theme;
        for (idx, it) in current.iter().enumerate() {
            let is_selected_item = idx == menu.selected_index;
            list_items.push(make_menu_list_item(it, theme, is_selected_item, &menu.items, &menu.path, idx));
        }
        let highlight_bg = app.theme.surface_alt;
        // Breadcrumbs in title
        let mut crumbs: Vec<String> = Vec::new();
        let mut items_ref = &menu.items;
        for &idx in &menu.path {
            if let Some(mi) = items_ref.get(idx) {
                crumbs.push(mi.name.clone());
                items_ref = &mi.items;
            } else {
                break;
            }
        }
        let title = if crumbs.is_empty() {
            "Menu".to_string()
        } else {
            format!("Menu — {}", crumbs.join(" > "))
        };
        let list = List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .padding(Padding::new(PAD_X, PAD_X, PAD_Y, PAD_Y)),
            )
            .highlight_style(Style::default().bg(highlight_bg));
        f.render_stateful_widget(
            list,
            area,
            &mut ratatui::widgets::ListState::default().with_selected(Some(menu.selected_index)),
        );
    }
}

fn make_menu_list_item<'a>(
    item: &'a MenuItem,
    theme: ThemeTokens,
    is_selected: bool,
    menu_root: &[MenuItem],
    menu_path: &[usize],
    item_index: usize,
) -> ListItem<'a> {
    let title_color = theme.menu_title;
    let action_color = theme.menu_action;
    let desc_color = theme.menu_desc;

    // Left vertical bar styling: dim when not selected, vivid when selected
    let bar_color = if is_selected {
        theme.selection_fg
    } else {
        theme.border
    };
    let bar_style = if is_selected {
        Style::default().fg(bar_color)
    } else {
        Style::default().fg(bar_color).add_modifier(Modifier::DIM)
    };
    let bar_span = Span::styled("│ ", bar_style);

    // Title line: name and optional non-dimmed [dir] tag at the end
    let name_span = Span::styled(item.name.clone(), Style::default().fg(title_color));
    let mut title_spans: Vec<Span> = vec![bar_span.clone(), name_span];
    if !item.items.is_empty() {
        title_spans.push(Span::styled(" \u{2630}", Style::default().fg(title_color)));
    }

    let mut action_text = if let Some(cmd) = &item.cmd {
        cmd.clone()
    } else if let Some(file) = &item.file {
        format!("file: {file}")
    } else if item.form.is_some() {
        "form".to_string()
    } else if !item.items.is_empty() {
        let (folders, cmds) = count_folders_and_cmds(item);
        format!("{} folders, {} commands", folders, cmds)
    } else {
        String::new()
    };
    
    // Add terminal alias if available (for commands and files)
    if item.cmd.is_some() || item.file.is_some() {
        if let Some(alias) = build_terminal_alias(menu_root, menu_path, item_index) {
            if !action_text.is_empty() {
                action_text.push_str("  •  ");
            }
            action_text.push_str(&alias);
        }
    }
    let action_span = if action_text.is_empty() {
        Span::raw("")
    } else {
        Span::styled(
            action_text,
            Style::default()
                .fg(action_color)
                .add_modifier(Modifier::DIM),
        )
    };

    let desc_text = item.desc.clone().unwrap_or_default();
    let desc_span = if desc_text.is_empty() {
        Span::raw("")
    } else {
        Span::styled(
            desc_text,
            Style::default().fg(desc_color).add_modifier(Modifier::DIM),
        )
    };

    let lines: Vec<Line> = vec![
        Line::from(title_spans),
        Line::from(vec![bar_span.clone(), action_span]),
        Line::from(vec![bar_span, desc_span]),
        // Spacer line between items
        Line::raw(""),
    ];

    ListItem::new(Text::from(lines))
}

pub fn handle_event(app: &mut App, key: KeyEvent) -> Result<bool> {
    if let Screen::Menu(menu) = &mut app.screen {
        match (key.code, key.modifiers) {
            // Esc/q always go back; at root they exit. Ctrl+C/Ctrl+Q exit immediately.
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Ok(true),
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => return Ok(true),
            (KeyCode::Esc, _) | (KeyCode::Char('q'), _) => {
                if menu.path.is_empty() {
                    return Ok(true);
                } else {
                    if let Some(prev_idx) = menu.path.pop() {
                        menu.selected_index = prev_idx;
                    } else {
                        menu.selected_index = 0;
                    }
                    return Ok(false);
                }
            }

            // Vim-style :q Enter
            (KeyCode::Char(':'), _) => {
                app.menu_cmd = Some(String::new());
            }
            (KeyCode::Backspace, _) => {
                if let Some(buf) = &mut app.menu_cmd {
                    let _ = buf.pop();
                }
            }
            (KeyCode::Char(ch), m) if m.is_empty() && app.menu_cmd.is_some() => {
                if let Some(buf) = &mut app.menu_cmd {
                    buf.push(ch);
                }
            }
            (KeyCode::Enter, _) => {
                if let Some(buf) = app.menu_cmd.take() {
                    if buf.trim() == "q" {
                        return Ok(true);
                    }
                } else {
                    // Enter submenu, run item, or open form
                    let current = submenu_at(&menu.items, &menu.path);
                    if let Some(item) = current.get(menu.selected_index).cloned() {
                        if !item.items.is_empty() {
                            menu.path.push(menu.selected_index);
                            menu.selected_index = 0;
                        } else if let Some(form) = item.form.clone() {
                            let state = crate::screens::form::from_spec(&form);
                            app.screen_stack
                                .push(std::mem::replace(&mut app.screen, Screen::Form(state)));
                            app.needs_clear = true;
                        } else if let Some(file) = item.file {
                            let view = open_file_view(Path::new(&file));
                            app.screen_stack
                                .push(std::mem::replace(&mut app.screen, Screen::Output(view)));
                            app.needs_clear = true;
                        } else if let Some(cmd) = item.cmd {
                            let external = item.external.unwrap_or(false);
                            let enhanced_terminal = item.enhanced_terminal.unwrap_or(false);
                            if external {
                                passthrough_command(app, &item.name, &cmd)?;
                            } else if enhanced_terminal {
                                start_command_enhanced(app, &item.name, &cmd)?;
                                app.needs_clear = true;
                            } else {
                                start_command(app, &item.name, &cmd)?;
                                app.needs_clear = true;
                            }
                        } else if item.plugin_list {
                            // Build dynamic submenu of running plugins
                            let mut children: Vec<crate::menu::MenuItem> = Vec::new();
                            for rt in &app.plugin_overlays {
                                let m = rt.meta();
                                children.push(crate::menu::MenuItem {
                                    name: m.name.to_string(),
                                    desc: Some(format!("{} ({})", m.id, m.version)),
                                    alias: None,
                                    aliases: None,
                                    cmd: None,
                                    file: None,
                                    items: Vec::new(),
                                    capture: None,
                                    external: None,
                                    enhanced_terminal: None,
                                    form: None,
                                    plugin_list: false,
                                });
                            }
                            if !children.is_empty() {
                                let mut folder = item.clone();
                                folder.items = children;
                                menu.path.push(menu.selected_index);
                                menu.selected_index = 0;
                            }
                        } else if item.name == "Configuration"
                            || item.alias.as_deref() == Some("config")
                        {
                            app.screen_stack.push(std::mem::replace(
                                &mut app.screen,
                                Screen::Config(open_config_state()),
                            ));
                            app.needs_clear = true;
                        }
                    }
                }
            }

            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                if menu.selected_index > 0 {
                    menu.selected_index -= 1;
                }
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                let current = submenu_at(&menu.items, &menu.path);
                if menu.selected_index + 1 < current.len() {
                    menu.selected_index += 1;
                }
            }
            (KeyCode::Left, _) => {
                if let Some(prev_idx) = menu.path.pop() {
                    menu.selected_index = prev_idx;
                } else if let Some(prev) = app.screen_stack.pop() {
                    app.screen = prev;
                    app.needs_clear = true;
                } else {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

// Optional View trait adapter for testing/extensibility
use crate::view::View;

pub struct MenuViewAdapter;

impl View for MenuViewAdapter {
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
