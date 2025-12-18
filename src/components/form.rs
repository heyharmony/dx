use crate::components::{Input as InputWidget, Select as SelectWidget};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

#[derive(Debug, Clone)]
pub enum FormFieldWidget {
    Input(InputWidget),
    Select(SelectWidget),
}

#[derive(Debug, Clone)]
pub struct Form {
    pub title: Option<String>,
    pub fields: Vec<(String, FormFieldWidget)>, // (name, widget)
    pub focus: usize,
}

impl Form {
    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let mut constraints: Vec<Constraint> = Vec::new();
        for _ in &self.fields {
            constraints.push(Constraint::Length(3));
        }
        if constraints.is_empty() {
            constraints.push(Constraint::Min(1));
        }
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);
        for (i, (_name, w)) in self.fields.iter_mut().enumerate() {
            match w {
                FormFieldWidget::Input(inp) => {
                    inp.focused = self.focus == i;
                    inp.render(f, chunks[i]);
                }
                FormFieldWidget::Select(sel) => {
                    sel.focused = self.focus == i;
                    sel.render(f, chunks[i]);
                }
            }
        }
    }
}
