use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};

#[derive(Debug, Clone)]
pub struct Input {
    pub label: Option<String>,
    pub value: String,
    pub placeholder: Option<String>,
    pub focused: bool,
    pub help: Option<String>,
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    #[must_use]
    pub fn new() -> Self {
        Self {
            label: None,
            value: String::new(),
            placeholder: None,
            focused: false,
            help: None,
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let title = self.label.clone().unwrap_or_default();
        let text = if self.value.is_empty() {
            self.placeholder.clone().unwrap_or_default()
        } else {
            self.value.clone()
        };
        if self.help.is_some() && area.height >= 4 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Length(3), Constraint::Length(1)])
                .split(area);
            let para = Paragraph::new(Line::from(text))
                .block(Block::default().borders(Borders::ALL).title(title))
                .style(Style::default());
            f.render_widget(para, chunks[0]);
            let help_text = self.help.clone().unwrap_or_default();
            let help_para = Paragraph::new(Line::from(help_text));
            f.render_widget(help_para, chunks[1]);
        } else {
            let para = Paragraph::new(Line::from(text))
                .block(Block::default().borders(Borders::ALL).title(title))
                .style(Style::default());
            f.render_widget(para, area);
        }
    }
}
