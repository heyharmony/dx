use crate::frame;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Line;

pub struct Statusbar;

impl Statusbar {
    pub fn render(spans: Line, area: Rect, f: &mut Frame) {
        frame::render_status_bar(spans, area, f);
    }
}
