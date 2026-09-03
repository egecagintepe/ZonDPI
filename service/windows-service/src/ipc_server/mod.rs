//! High-performance Named Pipe IPC server for ZonDPI with length-prefixed framing and local access controls.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tracing::{error, info, warn};

use crate::engine_mgr::{EngineManager, EngineManagerError};
use zondpi_ipc_protocol::{
    framing::{read_request, write_response},
    IpcCommand, IpcError, IpcErrorCode, IpcRequest, IpcResponse, DEFAULT_PIPE_NAME,
    PROTOCOL_VERSION,
};
use zondpi_packet_engine::EngineId;

#[derive(Error, Debug)]
pub enum IpcServerError {
    #[error("I/O error in Named Pipe server: {0}")]
    Io(#[from] std::io::Error),
    #[error("Pipe creation failed: {0}")]
    PipeCreation(String),
}

/// Asynchronous Named Pipe server dispatching commands to the EngineManager.
#[derive(Clone)]
pub struct IpcServer {
    pipe_name: String,
    engine_mgr: EngineManager,
    shutdown_requested: Arc<AtomicBool>,
}

impl IpcServer {
    pub fn new(pipe_name: impl Into<String>, engine_mgr: EngineManager) -> Self {
        Self {
            pipe_name: pipe_name.into(),
            engine_mgr,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn default_endpoint(engine_mgr: EngineManager) -> Self {
        Self::new(DEFAULT_PIPE_NAME, engine_mgr)
    }

    fn create_pipe(
        pipe_name: &str,
        is_first_instance: bool,
    ) -> Result<NamedPipeServer, IpcServerError> {
        let mut options = ServerOptions::new();
        options.first_pipe_instance(is_first_instance);
        options.in_buffer_size(65536);
        options.out_buffer_size(65536);

        let sddl = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;AU)\0";
        let mut p_sd: *mut std::ffi::c_void = std::ptr::null_mut();

        let sd_success = unsafe {
            windows_sys::Win32::Security::Authorization::ConvertStringSecurityDescriptorToSecurityDescriptorA(
                sddl.as_ptr(),
                windows_sys::Win32::Security::Authorization::SDDL_REVISION_1,
                &mut p_sd,
                std::ptr::null_mut(),
            )
        };

        let server_result = if sd_success != 0 && !p_sd.is_null() {
            let mut sa = windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>()
                    as u32,
                lpSecurityDescriptor: p_sd,
                bInheritHandle: 0,
            };
            unsafe {
                options.create_with_security_attributes_raw(
                    pipe_name,
                    &mut sa as *mut _ as *mut std::ffi::c_void,
                )
            }
        } else {
            warn!("Failed to set custom Security Descriptor for Named Pipe. Defaulting to process token.");
            options.create(pipe_name)
        };

        if !p_sd.is_null() {
            unsafe { windows_sys::Win32::Foundation::LocalFree(p_sd as _) };
        }

        server_result.map_err(|e| IpcServerError::PipeCreation(e.to_string()))
    }

    /// Runs the Named Pipe listener loop until shutdown is signaled.
    pub async fn run(&self) -> Result<(), IpcServerError> {
        info!(pipe = %self.pipe_name, "Starting ZonDPI Named Pipe IPC server");

        let mut is_first_instance = true;

        while !self.shutdown_requested.load(Ordering::SeqCst) {
            // Extract pipe creation to a separate block to ensure *mut c_void doesn't cross the await point
            let server = match Self::create_pipe(&self.pipe_name, is_first_instance) {
                Ok(s) => {
                    is_first_instance = false;
                    s
                }
                Err(e) => {
                    error!(error = %e, "Failed to create Named Pipe server");
                    return Err(e);
                }
            };

            // Await next client connection
            match server.connect().await {
                Ok(()) => {
                    let mgr = self.engine_mgr.clone();
                    tokio::spawn(async move {
                        Self::handle_client(server, mgr).await;
                    });
                }
                Err(e) => {
                    if !self.shutdown_requested.load(Ordering::SeqCst) {
                        warn!(error = %e, "Error accepting Named Pipe client connection");
                    }
                }
            }
        }

        info!("Named Pipe IPC server shutdown completed");
        Ok(())
    }

    /// Handles an individual connected client session over the Named Pipe.
    async fn handle_client(mut server: NamedPipeServer, mgr: EngineManager) {
        loop {
            let request = match read_request(&mut server).await {
                Ok(req) => req,
                Err(zondpi_ipc_protocol::ProtocolFrameError::ConnectionClosed) => {
                    // Client disconnected normally
                    break;
                }
                Err(e) => {
                    warn!(error = %e, "Malformed or oversized IPC request received");
                    let err_resp = IpcResponse::failure(
                        "invalid-request",
                        IpcError::new(IpcErrorCode::BadRequest, format!("Malformed frame: {}", e)),
                    );
                    let _ = write_response(&mut server, &err_resp).await;
                    break;
                }
            };

            let response = Self::dispatch_command(&request, &mgr).await;
            if let Err(e) = write_response(&mut server, &response).await {
                warn!(error = %e, "Failed to write IPC response to client");
                break;
            }
        }
    }

    /// Dispatches an incoming request to the EngineManager and forms a structured response.
    pub async fn dispatch_command(request: &IpcRequest, mgr: &EngineManager) -> IpcResponse {
        let req_id = &request.request_id;

        if request.protocol_version != PROTOCOL_VERSION {
            return IpcResponse::failure(
                req_id,
                IpcError::new(
                    IpcErrorCode::UnsupportedProtocolVersion,
                    format!(
                        "Client protocol version {} is unsupported. Server supports version {}.",
                        request.protocol_version, PROTOCOL_VERSION
                    ),
                ),
            );
        }

        match &request.command {
            IpcCommand::Ping => IpcResponse::ok_empty(req_id),
            IpcCommand::GetVersion => IpcResponse::success(
                req_id,
                &serde_json::json!({
                    "service_version": env!("CARGO_PKG_VERSION"),
                    "protocol_version": PROTOCOL_VERSION,
                }),
            ),
            IpcCommand::GetStatus => {
                let status = mgr.get_status().await;
                IpcResponse::success(req_id, &status)
            }
            IpcCommand::GetRecommendation => {
                let rec = mgr.get_recommendation().await;
                IpcResponse::success(req_id, &rec)
            }
            IpcCommand::ListProfiles => {
                let profiles = mgr.list_profiles().await;
                IpcResponse::success(req_id, &profiles)
            }
            IpcCommand::StartEngine { engine, profile } => {
                let engine_id = match Self::parse_engine_name(engine) {
                    Some(id) => id,
                    None => {
                        return IpcResponse::failure(
                            req_id,
                            IpcError::new(
                                IpcErrorCode::BadRequest,
                                format!("Unknown engine name '{}'", engine),
                            ),
                        );
                    }
                };

                match mgr.start_engine(engine_id, profile).await {
                    Ok(()) => IpcResponse::ok_empty(req_id),
                    Err(e) => Self::map_manager_error(req_id, e),
                }
            }
            IpcCommand::StartAuto { profile } => match mgr.start_auto(profile).await {
                Ok(()) => IpcResponse::ok_empty(req_id),
                Err(e) => Self::map_manager_error(req_id, e),
            },
            IpcCommand::StopEngine => match mgr.stop_engine().await {
                Ok(()) => IpcResponse::ok_empty(req_id),
                Err(e) => Self::map_manager_error(req_id, e),
            },
            IpcCommand::SwitchEngine { engine, profile } => {
                let engine_id = match Self::parse_engine_name(engine) {
                    Some(id) => id,
                    None => {
                        return IpcResponse::failure(
                            req_id,
                            IpcError::new(
                                IpcErrorCode::BadRequest,
                                format!("Unknown engine name '{}'", engine),
                            ),
                        );
                    }
                };

                match mgr.switch_engine(engine_id, profile).await {
                    Ok(()) => IpcResponse::ok_empty(req_id),
                    Err(e) => Self::map_manager_error(req_id, e),
                }
            }
            IpcCommand::GetRecentLogs { lines } => {
                let logs = mgr.get_recent_logs(*lines).await;
                IpcResponse::success(req_id, &logs)
            }
        }
    }

    /// Maps domain error to structured IPC error codes.
    fn map_manager_error(req_id: &str, err: EngineManagerError) -> IpcResponse {
        let (code, msg, details) = match err {
            EngineManagerError::Path(p) => (IpcErrorCode::ProfileNotFound, p.to_string(), None),
            EngineManagerError::ProfileJson(j) => (
                IpcErrorCode::ProfileInvalid,
                format!("Failed to parse profile JSON: {}", j),
                None,
            ),
            EngineManagerError::PortInUse(port) => (
                IpcErrorCode::PortInUse,
                format!("Local port {} is already in use by another service", port),
                Some(
                    "Configure a different port in profile or stop conflicting process".to_string(),
                ),
            ),
            EngineManagerError::HealthCheckFailed(h) => (
                IpcErrorCode::EngineHealthFailed,
                format!("Engine started but failed post-launch health check: {}", h),
                None,
            ),
            EngineManagerError::Goodbye(g) => (
                IpcErrorCode::EngineStartFailed,
                format!("GoodbyeDPI worker error: {}", g),
                None,
            ),
            EngineManagerError::ByeDpi(b) => (
                IpcErrorCode::EngineStartFailed,
                format!("ByeDPI worker error: {}", b),
                None,
            ),
            EngineManagerError::AutoModeFailed(a) => (
                IpcErrorCode::EngineStartFailed,
                format!("Auto mode selection failed: {}", a),
                None,
            ),
            EngineManagerError::UnknownEngine(u) => (
                IpcErrorCode::BadRequest,
                format!("Unsupported engine: {}", u),
                None,
            ),
        };

        IpcResponse::failure(
            req_id,
            IpcError {
                code,
                message: msg,
                details,
            },
        )
    }

    fn parse_engine_name(name: &str) -> Option<EngineId> {
        match name.to_lowercase().as_str() {
            "goodbye" | "goodbyedpi" => Some(EngineId::GoodbyeDpi),
            "byedpi" | "ciadpi" => Some(EngineId::ByeDpi),
            "native" | "rust" => Some(EngineId::Native),
            _ => None,
        }
    }

    /// Signals the listener loop to stop accepting new requests.
    pub fn stop(&self) {
        self.shutdown_requested.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_paths::RuntimePaths;

    #[tokio::test]
    async fn test_dispatch_ping_command() {
        let paths = RuntimePaths::discover().expect("paths");
        let mgr = EngineManager::new(paths);
        let req = IpcRequest::new(IpcCommand::Ping);
        let resp = IpcServer::dispatch_command(&req, &mgr).await;
        assert!(resp.success);
        assert_eq!(resp.request_id, req.request_id);
    }

    #[tokio::test]
    async fn test_dispatch_get_status_command() {
        let paths = RuntimePaths::discover().expect("paths");
        let mgr = EngineManager::new(paths);
        let req = IpcRequest::new(IpcCommand::GetStatus);
        let resp = IpcServer::dispatch_command(&req, &mgr).await;
        assert!(resp.success);
        assert!(resp.result.is_some());
    }

    #[tokio::test]
    async fn test_dispatch_rejects_unsupported_protocol_version() {
        let paths = RuntimePaths::discover().expect("paths");
        let mgr = EngineManager::new(paths);
        let mut req = IpcRequest::new(IpcCommand::Ping);
        req.protocol_version = 999;
        let resp = IpcServer::dispatch_command(&req, &mgr).await;
        assert!(!resp.success);
        assert_eq!(
            resp.error.unwrap().code,
            IpcErrorCode::UnsupportedProtocolVersion
        );
    }
}
