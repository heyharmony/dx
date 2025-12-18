pub mod prelude {
    pub use crate::events::{OverlayEffect, OverlayEvent, RenderRequest};
    pub use crate::host::{
        AppCommand, HostContext, KeyValueStore, ProcessHandleId, ProcessSpec, Telemetry,
    };
    pub use crate::overlay::{Overlay, OverlayFactory, OverlayMeta};
    pub use crate::render::{Line, RenderTree, Span};
    pub use crate::types::{Color, LogLevel, Result, SdkVersion, UiParams};
    // #[macro_export] places macro at crate root; re-export it from prelude
    pub use crate::commands::{CommandMeta, CommandProvider};
    pub use crate::dx_overlay;
}

pub mod types {
    use std::collections::BTreeMap;

    pub type UiParams = BTreeMap<String, String>;

    #[derive(Debug, Clone, Copy)]
    pub struct SdkVersion {
        pub major: u16,
        pub minor: u16,
        pub patch: u16,
    }

    pub const SDK_VERSION: SdkVersion = SdkVersion {
        major: 0,
        minor: 1,
        patch: 0,
    };

    #[derive(Debug, Clone, Copy)]
    pub enum LogLevel {
        Trace,
        Debug,
        Info,
        Warn,
        Error,
    }

    #[derive(Debug, Clone, Copy)]
    pub enum Color {
        Black,
        DarkGray,
        Gray,
        White,
        Red,
        Green,
        Yellow,
        Blue,
        Magenta,
        Cyan,
        Rgb(u8, u8, u8),
    }

    pub type Result<T> = std::result::Result<T, anyhow::Error>;
}

pub mod host {
    use super::types::{LogLevel, Result};

    pub trait KeyValueStore {
        fn get(&self, key: &str) -> Option<String>;
        fn set(&self, key: &str, value: &str);
        fn delete(&self, key: &str);
    }

    #[derive(Debug, Clone)]
    pub enum AppCommand {
        Toast { title: String, body: String },
        Log { level: String, message: String },
        // Status bar badge (e.g., 📡 streaming)
        SetStatusBadge { text: String },
        ClearStatusBadge,
        // Output view controls
        NavigateToOutput { title: String },
        AppendOutputLine { line: String },
        AppendOutputChunk { bytes: Vec<u8> },
        // Open a URL in user browser (host decides policy)
        OpenUrl { url: String },
    }

    pub trait HostContext {
        fn log(&self, level: LogLevel, msg: &str);
        fn storage(&self) -> &dyn KeyValueStore;
        fn schedule_tick(&self, every_millis: u64);
        fn emit_app_command(&self, cmd: AppCommand);
        // Process management (default: unimplemented)
        /// Spawns a new process according to the specification.
        /// 
        /// # Errors
        /// Returns error if process spawning is not implemented or fails.
        fn spawn_process(&mut self, _spec: ProcessSpec) -> Result<ProcessHandleId> {
            Err(anyhow::anyhow!("spawn_process not implemented"))
        }
        // Open URL (default no-op)
        fn open_url(&self, _url: &str) {}
        // Config (JSON) namespaced by plugin (default: none)
        fn read_config(&self, _ns: &str) -> Option<serde_json::Value> {
            None
        }
        /// Writes configuration value for the specified namespace.
        /// 
        /// # Errors
        /// Returns error if configuration writing fails.
        fn write_config(&mut self, _ns: &str, _value: &serde_json::Value) -> Result<()> {
            Ok(())
        }
        // Secrets (default: none)
        fn secret(&self, _name: &str) -> Option<String> {
            None
        }
        // Telemetry (opt-in)
        fn telemetry(&self) -> Telemetry {
            Telemetry { enabled: false }
        }
    }

    #[derive(Debug, Clone)]
    pub struct Telemetry {
        pub enabled: bool,
    }
    impl Telemetry {
        pub fn record(&self, _event: &str, _props: serde_json::Value) { /* default no-op */
        }
    }

    #[derive(Debug, Clone)]
    pub struct ProcessSpec {
        pub cmd: String,
        pub args: Vec<String>,
        pub cwd: Option<String>,
        pub env: Vec<(String, String)>,
        pub pty: bool,
        pub shell: bool,
        pub merge_stderr: bool,
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ProcessHandleId(pub u64);
}

pub mod render {
    use super::types::Color;

    #[derive(Debug, Clone)]
    pub struct Span {
        pub text: String,
        pub color: Option<Color>,
    }
    #[derive(Debug, Clone)]
    pub struct Line(pub Vec<Span>);

    #[derive(Debug, Clone)]
    pub enum RenderTree {
        Lines(Vec<Line>),
        Bar {
            percent: u8,
            color: Color,
            label_right: Option<String>,
        },
        Group(Vec<RenderTree>),
    }
}

pub mod events {
    use super::types::UiParams;

    #[derive(Debug, Clone)]
    pub enum OverlayEvent {
        Init { params: UiParams },
        Tick,
        Resize { width: u16, height: u16 },
        VisibilityChanged { visible: bool },
        Data { value: serde_json::Value },
    }

    #[derive(Debug, Clone, Copy)]
    pub struct RenderRequest {
        pub width: u16,
        pub height: u16,
    }

    #[derive(Debug, Clone)]
    pub enum OverlayEffect {
        None,
        Redraw,
        ScheduleTick { every_millis: u64 },
        AppCommand(super::host::AppCommand),
    }
}

pub mod overlay {
    use crate::events::{OverlayEffect, OverlayEvent, RenderRequest};
    use crate::host::HostContext;
    use crate::render::RenderTree;
    use crate::types::{Result, UiParams};
    use std::any::Any;

    #[derive(Debug, Clone)]
    pub struct OverlayMeta {
        pub id: &'static str,
        pub name: &'static str,
        pub version: &'static str,
        pub capabilities: &'static [&'static str],
    }

    pub trait Overlay {
        fn as_any(&self) -> &dyn Any;
        fn as_any_mut(&mut self) -> &mut dyn Any;
        fn meta(&self) -> OverlayMeta;
        /// Initializes the overlay with given context and parameters.
        /// 
        /// # Errors
        /// Returns error if initialization fails.
        fn init(&mut self, ctx: &mut dyn HostContext, params: UiParams) -> Result<()>;
        /// Handles an overlay event and returns the resulting effect.
        /// 
        /// # Errors
        /// Returns error if event handling fails.
        fn handle_event(
            &mut self,
            ctx: &mut dyn HostContext,
            event: OverlayEvent,
        ) -> Result<OverlayEffect>;
        fn render(&self, req: RenderRequest) -> RenderTree;
    }

    pub trait OverlayFactory {
        fn create(&self) -> Box<dyn Overlay>;
    }
}

pub mod commands {
    use crate::host::HostContext;
    use crate::types::Result;

    #[derive(Debug, Clone)]
    pub struct CommandMeta {
        pub id: &'static str,
        pub title: &'static str,
        pub description: &'static str,
    }

    pub trait CommandProvider {
        fn commands(&self) -> Vec<CommandMeta>;
        /// Invokes a command with the given ID and arguments.
        /// 
        /// # Errors
        /// Returns error if command invocation fails or command is not found.
        fn invoke(
            &mut self,
            id: &str,
            args: serde_json::Value,
            host: &mut dyn HostContext,
        ) -> Result<()>;
    }
}

pub mod macros {
    #[macro_export]
    macro_rules! dx_overlay {
        ($ty:ty) => {
            #[no_mangle]
            pub extern "C" fn dx_overlay() -> Box<dyn $crate::overlay::Overlay> {
                Box::new(<$ty>::default())
            }

            #[no_mangle]
            pub extern "C" fn dx_sdk_version() -> $crate::types::SdkVersion {
                $crate::types::SDK_VERSION
            }
        };
    }
}
