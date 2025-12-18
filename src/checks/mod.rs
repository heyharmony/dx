pub mod context;
pub mod env_example_check;
pub mod node_deps_check;
pub mod runner;
pub mod types;

use context::CheckContext;
use types::Suggestion;

/// Trait implemented by individual startup checks.
pub trait StartupCheck: Send + Sync {
    fn name(&self) -> &'static str;
    fn run(&self, ctx: &CheckContext) -> Vec<Suggestion>;
}
