use std::fs;
use std::path::PathBuf;

use dx::app::commands::DxAppCommand;
use dx::app::suggestion_router::SuggestionRouter;
use dx::checks::StartupCheck;
use dx::checks::context::CheckContext;
use dx::checks::env_example_check::EnvExampleCheck;
use dx::checks::node_deps_check::NodeDepsCheck;
use dx::checks::runner::CheckRunner;
use dx::checks::types::ModalId;

#[test]
fn env_example_check_emits_modal_when_env_missing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join(".env.example"), "KEY=VAL\n").unwrap();
    // .env intentionally missing

    let ctx = CheckContext {
        project_root: PathBuf::from(root),
    };
    let check = EnvExampleCheck;
    let results = check.run(&ctx);
    assert_eq!(results.len(), 1);
    let sug = &results[0];
    assert!(sug.id.contains("env.missing_env"));
    let cmd = SuggestionRouter::map_suggestion_to_command(sug).expect("ui command");
    match cmd {
        DxAppCommand::ShowModal { id, .. } => assert!(matches!(id, ModalId::ConfirmEnvCreate)),
        _ => panic!("expected ShowModal"),
    }
}

#[test]
fn node_deps_check_suggests_install_when_missing_node_modules() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join("package.json"), "{\"name\":\"x\"}").unwrap();
    // no node_modules

    let ctx = CheckContext {
        project_root: PathBuf::from(root),
    };
    let check = NodeDepsCheck;
    let results = check.run(&ctx);
    assert_eq!(results.len(), 1);
    assert!(results[0].id.contains("node.missing_node_modules"));
}

#[test]
fn runner_collects_results_from_multiple_checks() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(root.join(".env.example"), "KEY=VAL\n").unwrap();
    fs::write(root.join("package.json"), "{\"name\":\"x\"}").unwrap();

    let ctx = CheckContext {
        project_root: PathBuf::from(root),
    };
    let runner = CheckRunner::new()
        .register(EnvExampleCheck)
        .register(NodeDepsCheck);
    let all = runner.run_all(&ctx);
    // Both should fire (env missing + node_modules missing)
    assert!(all.iter().any(|s| s.id.contains("env.missing_env")));
    assert!(
        all.iter()
            .any(|s| s.id.contains("node.missing_node_modules"))
    );
}
