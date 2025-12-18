use std::fs;

use super::StartupCheck;
use super::context::CheckContext;
use super::types::{ActionKind, Category, Severity, SuggestedAction, Suggestion};

pub struct NodeDepsCheck;

impl StartupCheck for NodeDepsCheck {
    fn name(&self) -> &'static str {
        "NodeDepsCheck"
    }

    fn run(&self, ctx: &CheckContext) -> Vec<Suggestion> {
        let pkg = ctx.project_root.join("package.json");
        if !pkg.exists() {
            return Vec::new();
        }

        let node_modules = ctx.project_root.join("node_modules");
        let has_node_modules = fs::metadata(&node_modules).is_ok();

        if !has_node_modules {
            return vec![Suggestion {
                id: "node.missing_node_modules".into(),
                title: "Nie zainstalowano zależności Node".into(),
                message: "Znaleziono package.json, ale brak folderu node_modules. Zainstalować zależności?".into(),
                category: Category::Dependencies,
                severity: Severity::Warning,
                path: Some(pkg),
                tags: vec!["node".into(), "deps".into(), "install".into()],
                action: Some(SuggestedAction {
                    kind: ActionKind::RunCommand,
                    label: "Uruchom npm install".into(),
                    command: Some("npm install".into()),
                    source: None,
                    target: None,
                    url: None,
                    presentation: None,
                }),
                source_check: self.name(),
            }];
        }

        Vec::new()
    }
}
