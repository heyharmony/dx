use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

// This module would host shared frame rendering helpers if needed.
// For now we keep it as a placeholder to centralize future frame/UI composition.

#[must_use]
pub fn split_main_area(area: Rect, motd_height: u16) -> Vec<Rect> {
    if motd_height > 0 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(motd_height),
                Constraint::Length(1),
                Constraint::Min(3),
            ])
            .split(area)
            .to_vec()
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3)])
            .split(area)
            .to_vec()
    }
}

pub fn render_border_block<'a>(title: impl Into<Line<'a>>, area: Rect, f: &mut Frame) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .padding(Padding::new(2, 2, 1, 1));
    f.render_widget(block, area);
    Rect {
        x: area.x.saturating_add(1 + 2),
        y: area.y.saturating_add(1 + 1),
        width: area.width.saturating_sub(2 + 2 * 2),
        height: area.height.saturating_sub(2 + 2),
    }
}

pub fn render_modal<'a>(title: impl Into<Line<'a>>, msg: &str, area: Rect, f: &mut Frame) {
    let inverted = Style::default().add_modifier(Modifier::REVERSED);
    let modal = Paragraph::new(Text::from(msg.to_string()))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .style(inverted),
        )
        .style(inverted);
    f.render_widget(Clear, area);
    f.render_widget(modal, area);
}

pub fn render_status_bar(spans: Line, area: Rect, f: &mut Frame) {
    let status_para = Paragraph::new(spans);
    f.render_widget(Clear, area);
    f.render_widget(status_para, area);
}
