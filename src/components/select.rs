use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

#[derive(Debug, Clone)]
pub struct Select {
    pub label: Option<String>,
    pub options: Vec<String>,
    pub selected: usize,
    pub focused: bool,
    pub help: Option<String>,
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl Select {
    #[must_use]
    pub fn new() -> Self {
        Self {
            label: None,
            options: Vec::new(),
            selected: 0,
            focused: false,
            help: None,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        if self.selected >= self.options.len() {
            self.selected = self.options.len().saturating_sub(1);
        }
        let items: Vec<ListItem> = self
            .options
            .iter()
            .map(|o| ListItem::new(o.clone()))
            .collect();
        if self.help.is_some() && area.height >= 4 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vec![Constraint::Min(3), Constraint::Length(1)])
                .split(area);
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.label.clone().unwrap_or_default()),
            );
            f.render_stateful_widget(
                list,
                chunks[0],
                &mut ratatui::widgets::ListState::default().with_selected(Some(self.selected)),
            );
            let help_text = self.help.clone().unwrap_or_default();
            let help_para = Paragraph::new(Line::from(help_text));
            f.render_widget(help_para, chunks[1]);
        } else {
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(self.label.clone().unwrap_or_default()),
            );
            f.render_stateful_widget(
                list,
                area,
                &mut ratatui::widgets::ListState::default().with_selected(Some(self.selected)),
            );
        }
    }
}
