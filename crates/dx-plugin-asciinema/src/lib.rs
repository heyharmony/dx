use dx_sdk::prelude::*;
use dx_sdk::{commands, host};
use serde_json;

#[derive(Default)]
struct AsciinemaPlugin;

impl Overlay for AsciinemaPlugin {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn meta(&self) -> OverlayMeta {
        OverlayMeta {
            id: "plugin.asciinema",
            name: "Asciinema",
            version: "0.1.0",
            capabilities: &["commands"],
        }
    }
    fn init(&mut self, _ctx: &mut dyn HostContext, _params: UiParams) -> Result<()> {
        Ok(())
    }
    fn handle_event(
        &mut self,
        _ctx: &mut dyn HostContext,
        _event: OverlayEvent,
    ) -> Result<OverlayEffect> {
        Ok(OverlayEffect::None)
    }
    fn render(&self, _req: RenderRequest) -> RenderTree {
        RenderTree::Lines(vec![Line(vec![Span {
            text: "".to_string(),
            color: None,
        }])])
    }
}

impl commands::CommandProvider for AsciinemaPlugin {
    fn commands(&self) -> Vec<commands::CommandMeta> {
        vec![
            commands::CommandMeta {
                id: "asciinema.record",
                title: "Record session",
                description: "asciinema record",
            },
            commands::CommandMeta {
                id: "asciinema.stream",
                title: "Stream session",
                description: "asciinema stream",
            },
        ]
    }
    fn invoke(
        &mut self,
        id: &str,
        _args: serde_json::Value,
        host: &mut dyn HostContext,
    ) -> Result<()> {
        match id {
            "asciinema.record" => {
                host.emit_app_command(AppCommand::NavigateToOutput {
                    title: "asciinema record".into(),
                });
                let _ = host.spawn_process(host::ProcessSpec {
                    cmd: "asciinema".into(),
                    args: vec!["record".into()],
                    cwd: None,
                    env: Vec::new(),
                    pty: false,
                    shell: false,
                    merge_stderr: false,
                });
            }
            "asciinema.stream" => {
                host.emit_app_command(AppCommand::NavigateToOutput {
                    title: "asciinema stream".into(),
                });
                let _ = host.spawn_process(host::ProcessSpec {
                    cmd: "asciinema".into(),
                    args: vec!["stream".into()],
                    cwd: None,
                    env: Vec::new(),
                    pty: false,
                    shell: false,
                    merge_stderr: false,
                });
            }
            _ => {}
        }
        Ok(())
    }
}

dx_overlay!(AsciinemaPlugin);
