use std::fs;

use super::StartupCheck;
use super::context::CheckContext;
use super::types::{
    ActionKind, Category, ModalId, Presentation, Severity, SuggestedAction, Suggestion, UiParams,
};

pub struct EnvExampleCheck;

impl StartupCheck for EnvExampleCheck {
    fn name(&self) -> &'static str {
        "EnvExampleCheck"
    }

    fn run(&self, ctx: &CheckContext) -> Vec<Suggestion> {
        let example = ctx.project_root.join(".env.example");
        let env = ctx.project_root.join(".env");

        let has_example = fs::metadata(&example).is_ok();
        let has_env = fs::metadata(&env).is_ok();

        if has_example && !has_env {
            let mut params: UiParams = UiParams::new();
            params.insert("source".into(), example.display().to_string());
            params.insert("target".into(), env.display().to_string());
            return vec![Suggestion {
                id: "env.missing_env".into(),
                title: "Brakuje pliku .env".into(),
                message:
                    "Znaleziono .env.example, ale nie ma .env. Utworzyć na podstawie .env.example?"
                        .into(),
                category: Category::Environment,
                severity: Severity::Warning,
                path: Some(env.clone()),
                tags: vec!["env".into(), "setup".into()],
                action: Some(SuggestedAction {
                    kind: ActionKind::OpenModal {
                        id: ModalId::ConfirmEnvCreate,
                        params,
                    },
                    label: "Otwórz modal tworzenia .env".into(),
                    command: None,
                    source: Some(example),
                    target: Some(env),
                    url: None,
                    presentation: Some(Presentation {
                        auto_show: true,
                        blocking: true,
                        group_key: Some("env".into()),
                    }),
                }),
                source_check: self.name(),
            }];
        }

        Vec::new()
    }
}
