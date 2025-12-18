// TODO: Fix clippy warnings for better code quality
#![allow(clippy::uninlined_format_args)] // TODO: Use {var} format syntax
#![allow(clippy::collapsible_match)] // TODO: Simplify nested match patterns

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

use crate::menu::FormSpec;
use crate::{App, Screen};
use dx::components::Form as FormWidget;
use dx::components::form::FormFieldWidget;

#[derive(Debug)]
pub struct FormState {
    pub title: String,
    pub form: FormWidget,
    pub submit_tpl: Option<String>,
}

pub fn from_spec(spec: &FormSpec) -> FormState {
    use dx::components::{Input as InputWidget, Select as SelectWidget};
    let mut form = FormWidget {
        title: spec.title.clone(),
        fields: Vec::new(),
        focus: 0,
    };
    for f in &spec.fields {
        let label = f.label.clone().unwrap_or_else(|| f.name.clone());
        let ty = f.r#type.clone().unwrap_or_else(|| "input".to_string());
        if ty.eq_ignore_ascii_case("select") {
            let mut sel = SelectWidget::new();
            sel.label = Some(label);
            sel.options = f.options.clone().unwrap_or_default();
            sel.help = f.help.clone();
            form.fields
                .push((f.name.clone(), FormFieldWidget::Select(sel)));
        } else {
            let mut inp = InputWidget::new();
            inp.label = Some(label);
            inp.placeholder = f.placeholder.clone();
            if let Some(d) = &f.default {
                inp.value = d.clone();
            }
            inp.help = f.help.clone();
            form.fields
                .push((f.name.clone(), FormFieldWidget::Input(inp)));
        }
    }
    FormState {
        title: spec.title.clone().unwrap_or_else(|| "Form".to_string()),
        form,
        submit_tpl: spec.submit.clone(),
    }
}

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    if let Screen::Form(state) = &mut app.screen {
        let inner = crate::frame::render_border_block(Line::from(state.title.clone()), area, f);
        state.form.render(f, inner);
    }
}

pub fn handle_event(app: &mut App, key: KeyEvent) -> Result<bool> {
    if let Screen::Form(state) = &mut app.screen {
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('q'), _) => {
                if let Some(prev) = app.screen_stack.pop() {
                    app.screen = prev;
                    app.needs_clear = true;
                } else {
                    return Ok(true);
                }
            }
            (KeyCode::Tab, _) => {
                state.form.focus = (state.form.focus + 1) % state.form.fields.len().max(1);
            }
            (KeyCode::BackTab, _) => {
                if state.form.fields.is_empty() {
                } else {
                    state.form.focus =
                        state.form.focus.saturating_sub(1) % state.form.fields.len().max(1);
                }
            }
            (KeyCode::Enter, _) => {
                if let Some(tpl) = state.submit_tpl.clone() {
                    // build map
                    let mut cmd = tpl.clone();
                    let snapshot = state.form.fields.clone();
                    for (name, w) in &snapshot {
                        let v = match w {
                            FormFieldWidget::Input(inp) => inp.value.clone(),
                            FormFieldWidget::Select(sel) => {
                                sel.options.get(sel.selected).cloned().unwrap_or_default()
                            }
                        };
                        let placeholder = format!("{{{}}}", name);
                        cmd = cmd.replace(&placeholder, &v);
                    }
                    // run command
                    let title = state.title.clone();
                    let _ = &state;
                    let _ = crate::start_command(app, &title, &cmd);
                    app.needs_clear = true;
                }
            }
            // Accept normal chars and Shift-modified chars; ignore Control combinations
            (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
                if let Some((_n, w)) = state.form.fields.get_mut(state.form.focus) {
                    if let FormFieldWidget::Input(inp) = w {
                        inp.value.push(c);
                    }
                }
            }
            (KeyCode::Backspace, _) => {
                if let Some((_n, w)) = state.form.fields.get_mut(state.form.focus) {
                    if let FormFieldWidget::Input(inp) = w {
                        let _ = inp.value.pop();
                    }
                }
            }
            (KeyCode::Up, _) => {
                if let Some((_n, w)) = state.form.fields.get_mut(state.form.focus) {
                    if let FormFieldWidget::Select(sel) = w {
                        if sel.selected > 0 {
                            sel.selected -= 1;
                        }
                    }
                }
            }
            (KeyCode::Down, _) => {
                if let Some((_n, w)) = state.form.fields.get_mut(state.form.focus) {
                    if let FormFieldWidget::Select(sel) = w {
                        if sel.selected + 1 < sel.options.len() {
                            sel.selected += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(false)
}
