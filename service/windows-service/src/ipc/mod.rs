//! Named Pipe IPC Server subsystem interface and message schemas.

pub mod protocol;

pub use protocol::*;

/// Configuration parameters for the Windows Named Pipe server.
#[derive(Debug, Clone)]
pub struct IpcServerConfig {
    pub pipe_name: String,
    pub max_instances: u32,
    pub buffer_size: usize,
}

impl Default for IpcServerConfig {
    fn default() -> Self {
        Self {
            pipe_name: r"\\.\pipe\zondpi-service-ipc".to_string(),
            max_instances: 4,
            buffer_size: 65536,
        }
    }
}
