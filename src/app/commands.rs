use crate::checks::types::{ModalId, OverlayId, ScreenId, UiParams};

#[derive(Debug, Clone)]
pub enum DxAppCommand {
    ShowOverlay { id: OverlayId, params: UiParams },
    ShowModal { id: ModalId, params: UiParams },
    NavigateTo { id: ScreenId, params: UiParams },
    Toast { title: String, body: String },
    Log { level: String, message: String },
}
