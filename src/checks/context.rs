use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CheckContext {
    pub project_root: PathBuf,
}
