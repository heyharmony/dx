use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Environment,
    NodeJs,
    Filesystem,
    Dependencies,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlayId {
    Cpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModalId {
    ConfirmEnvCreate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenId {
    Menu,
    Config,
    Output,
}

pub type UiParams = BTreeMap<String, String>;

#[derive(Debug, Clone, Default)]
pub struct Presentation {
    pub auto_show: bool,
    pub blocking: bool,
    pub group_key: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ActionKind {
    CreateFile,
    CopyFile,
    RunCommand,
    OpenUrl,
    OpenOverlay { id: OverlayId, params: UiParams },
    OpenModal { id: ModalId, params: UiParams },
    NavigateToScreen { id: ScreenId, params: UiParams },
}

#[derive(Debug, Clone)]
pub struct SuggestedAction {
    pub kind: ActionKind,
    pub label: String,
    pub command: Option<String>,
    pub source: Option<PathBuf>,
    pub target: Option<PathBuf>,
    pub url: Option<String>,
    pub presentation: Option<Presentation>,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub id: String,
    pub title: String,
    pub message: String,
    pub category: Category,
    pub severity: Severity,
    pub path: Option<PathBuf>,
    pub tags: Vec<String>,
    pub action: Option<SuggestedAction>,
    pub source_check: &'static str,
}
