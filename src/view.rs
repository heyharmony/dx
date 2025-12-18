use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

/// Unified view interface for screens to simplify testing and future extensibility.
pub trait View {
    #[allow(dead_code)]
    fn render(&mut self, f: &mut Frame, area: Rect, app: &mut crate::App);

    #[allow(dead_code)]
    fn handle_event(&mut self, app: &mut crate::App, key: KeyEvent) -> Result<bool>;
}
