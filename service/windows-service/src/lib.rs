//! ZonDPI Windows Service library module definitions.

pub mod engine_adapter;
pub mod engine_mgr;
pub mod hosts;
pub mod ipc_server;
pub mod runtime_paths;
pub mod service_scm;
pub mod supervisor;

pub use engine_mgr::{ActiveMode, EngineManager, EngineManagerError};
pub use ipc_server::IpcServer;
pub use runtime_paths::RuntimePaths;
pub use supervisor::{ProcessSupervisor, SupervisorConfig, SupervisorError, SupervisorState};
