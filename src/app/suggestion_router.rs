use crate::app::commands::DxAppCommand;
use crate::checks::types::{ActionKind, Suggestion};

pub struct SuggestionRouter;

impl SuggestionRouter {
    #[must_use]
    pub fn map_suggestion_to_command(s: &Suggestion) -> Option<DxAppCommand> {
        if let Some(a) = &s.action {
            match &a.kind {
                ActionKind::OpenOverlay { id, params } => Some(DxAppCommand::ShowOverlay {
                    id: *id,
                    params: params.clone(),
                }),
                ActionKind::OpenModal { id, params } => Some(DxAppCommand::ShowModal {
                    id: *id,
                    params: params.clone(),
                }),
                ActionKind::NavigateToScreen { id, params } => Some(DxAppCommand::NavigateTo {
                    id: *id,
                    params: params.clone(),
                }),
                _ => None,
            }
        } else {
            None
        }
    }
}
