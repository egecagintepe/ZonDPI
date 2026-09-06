//! OS-agnostic wire protocol, request/response models, and length-prefixed framing for ZonDPI Named Pipe IPC.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Default Named Pipe endpoint path on Windows.
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\zondpi-service-ipc";

/// Current supported wire protocol version.
pub const PROTOCOL_VERSION: u32 = 1;

/// Maximum allowable IPC payload size (1 MiB) to prevent memory exhaustion / DoS.
pub const MAX_FRAME_SIZE: usize = 1024 * 1024;

/// Structured error codes for IPC operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpcErrorCode {
    EngineBinaryNotFound,
    ProfileNotFound,
    ProfileInvalid,
    AccessDenied,
    EngineStartFailed,
    EngineHealthFailed,
    PortInUse,
    InvalidState,
    UnsupportedProtocolVersion,
    InternalError,
    Timeout,
    BadRequest,
}

/// Structured IPC error carrying code, readable message, and optional technical details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl IpcError {
    pub fn new(code: IpcErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: IpcErrorCode,
        message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            details: Some(details.into()),
        }
    }
}

/// Supported IPC commands requested by clients (CLI, GUI).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum IpcCommand {
    Ping,
    GetVersion,
    GetStatus,
    GetRecommendation,
    ListProfiles,
    StartEngine { engine: String, profile: String },
    StartAuto { profile: String },
    StopEngine,
    SwitchEngine { engine: String, profile: String },
    GetRecentLogs { lines: Option<usize> },
}

impl IpcCommand {
    /// Returns true if this command modifies engine state (requires serialization and write auth).
    pub fn is_mutating(&self) -> bool {
        matches!(
            self,
            IpcCommand::StartEngine { .. }
                | IpcCommand::StartAuto { .. }
                | IpcCommand::StopEngine
                | IpcCommand::SwitchEngine { .. }
        )
    }
}

/// Envelope for incoming client requests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcRequest {
    pub request_id: String,
    pub protocol_version: u32,
    pub command: IpcCommand,
}

impl IpcRequest {
    pub fn new(command: IpcCommand) -> Self {
        Self {
            request_id: format!(
                "req-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
            ),
            protocol_version: PROTOCOL_VERSION,
            command,
        }
    }

    pub fn with_id(request_id: impl Into<String>, command: IpcCommand) -> Self {
        Self {
            request_id: request_id.into(),
            protocol_version: PROTOCOL_VERSION,
            command,
        }
    }
}

/// Envelope for outgoing server responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcResponse {
    pub request_id: String,
    pub protocol_version: u32,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<IpcError>,
}

impl IpcResponse {
    pub fn success<T: Serialize>(request_id: impl Into<String>, result: &T) -> Self {
        Self {
            request_id: request_id.into(),
            protocol_version: PROTOCOL_VERSION,
            success: true,
            result: serde_json::to_value(result).ok(),
            error: None,
        }
    }

    pub fn ok_empty(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            protocol_version: PROTOCOL_VERSION,
            success: true,
            result: Some(serde_json::json!({ "status": "ok" })),
            error: None,
        }
    }

    pub fn failure(request_id: impl Into<String>, error: IpcError) -> Self {
        Self {
            request_id: request_id.into(),
            protocol_version: PROTOCOL_VERSION,
            success: false,
            result: None,
            error: Some(error),
        }
    }
}

// ==========================================
// Wire DTOs for IPC Results
// ==========================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthStatusDto {
    pub is_healthy: bool,
    pub latency_ms: u64,
    pub message: String,
    #[serde(default = "default_health_string")]
    pub process_health: String,
    #[serde(default = "default_health_string")]
    pub engine_health: String,
    #[serde(default = "default_health_string")]
    pub network_effectiveness: String,
}

fn default_health_string() -> String {
    "Unknown".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceStatusDto {
    pub version: String,
    pub uptime_seconds: u64,
    pub service_state: String,
    pub mode: String,
    pub active_engine: Option<String>,
    pub requested_engine: Option<String>,
    pub active_profile: Option<String>,
    pub engine_pid: Option<u32>,
    pub engine_uptime_seconds: Option<u64>,
    pub health: HealthStatusDto,
    pub compatibility_recommendation: Option<String>,
    pub fallback_reason: Option<String>,
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_compatibility: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_compatibility_method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_provider: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileSummaryDto {
    pub id: String,
    pub name: String,
    pub description: String,
    pub target_engine: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecommendationDto {
    pub detected_security_products: Vec<String>,
    pub recommended_engine: String,
    pub rationale: String,
    pub compatibility_rating: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogEntryDto {
    pub timestamp: String,
    pub stream: String,
    pub line: String,
}

// ==========================================
// Framing & Codec (Async Length-Prefixed)
// ==========================================

#[derive(Error, Debug)]
pub enum ProtocolFrameError {
    #[error("I/O error during frame transmission: {0}")]
    Io(#[from] std::io::Error),
    #[error("Frame length {0} exceeds maximum allowable limit of {MAX_FRAME_SIZE} bytes")]
    OversizedFrame(usize),
    #[error("JSON serialization/deserialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Connection closed prematurely by peer")]
    ConnectionClosed,
    #[error("Invalid framing header")]
    InvalidHeader,
}

#[cfg(feature = "tokio")]
pub mod framing {
    use super::*;
    use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

    /// Reads a length-prefixed raw frame (`[4 bytes u32 LE][payload bytes]`).
    pub async fn read_raw_frame<R: AsyncRead + Unpin>(
        reader: &mut R,
    ) -> Result<Vec<u8>, ProtocolFrameError> {
        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(ProtocolFrameError::ConnectionClosed);
            }
            Err(e) => return Err(ProtocolFrameError::Io(e)),
        }

        let length = u32::from_le_bytes(len_buf) as usize;
        if length > MAX_FRAME_SIZE {
            return Err(ProtocolFrameError::OversizedFrame(length));
        }

        let mut payload = vec![0u8; length];
        reader.read_exact(&mut payload).await?;
        Ok(payload)
    }

    /// Writes a length-prefixed raw frame (`[4 bytes u32 LE][payload bytes]`).
    pub async fn write_raw_frame<W: AsyncWrite + Unpin>(
        writer: &mut W,
        payload: &[u8],
    ) -> Result<(), ProtocolFrameError> {
        if payload.len() > MAX_FRAME_SIZE {
            return Err(ProtocolFrameError::OversizedFrame(payload.len()));
        }

        let len_bytes = (payload.len() as u32).to_le_bytes();
        writer.write_all(&len_bytes).await?;
        writer.write_all(payload).await?;
        writer.flush().await?;
        Ok(())
    }

    /// Reads and deserializes an [`IpcRequest`] from the stream.
    pub async fn read_request<R: AsyncRead + Unpin>(
        reader: &mut R,
    ) -> Result<IpcRequest, ProtocolFrameError> {
        let raw = read_raw_frame(reader).await?;
        let req = serde_json::from_slice(&raw)?;
        Ok(req)
    }

    /// Serializes and writes an [`IpcRequest`] to the stream.
    pub async fn write_request<W: AsyncWrite + Unpin>(
        writer: &mut W,
        request: &IpcRequest,
    ) -> Result<(), ProtocolFrameError> {
        let raw = serde_json::to_vec(request)?;
        write_raw_frame(writer, &raw).await
    }

    /// Reads and deserializes an [`IpcResponse`] from the stream.
    pub async fn read_response<R: AsyncRead + Unpin>(
        reader: &mut R,
    ) -> Result<IpcResponse, ProtocolFrameError> {
        let raw = read_raw_frame(reader).await?;
        let res = serde_json::from_slice(&raw)?;
        Ok(res)
    }

    /// Serializes and writes an [`IpcResponse`] to the stream.
    pub async fn write_response<W: AsyncWrite + Unpin>(
        writer: &mut W,
        response: &IpcResponse,
    ) -> Result<(), ProtocolFrameError> {
        let raw = serde_json::to_vec(response)?;
        write_raw_frame(writer, &raw).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_request_serialization() {
        let req = IpcRequest::new(IpcCommand::StartEngine {
            engine: "goodbye".to_string(),
            profile: "turktelekom".to_string(),
        });
        assert!(req.command.is_mutating());

        let json = serde_json::to_string(&req).expect("serialize");
        let deserialized: IpcRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(req.request_id, deserialized.request_id);
        assert_eq!(req.command, deserialized.command);
    }

    #[test]
    fn test_ipc_response_success_and_error() {
        let resp_ok = IpcResponse::success("req-1", &serde_json::json!({ "active": true }));
        assert!(resp_ok.success);
        assert!(resp_ok.error.is_none());

        let resp_err = IpcResponse::failure(
            "req-2",
            IpcError::with_details(
                IpcErrorCode::EngineBinaryNotFound,
                "Binary missing",
                "goodbyedpi.exe not found in path",
            ),
        );
        assert!(!resp_err.success);
        let err = resp_err.error.unwrap();
        assert_eq!(err.code, IpcErrorCode::EngineBinaryNotFound);
        assert_eq!(err.message, "Binary missing");
        assert_eq!(
            err.details.as_deref(),
            Some("goodbyedpi.exe not found in path")
        );
    }

    #[cfg(feature = "tokio")]
    #[tokio::test]
    async fn test_async_framing_roundtrip() {
        use framing::{read_request, read_response, write_request, write_response};
        use tokio::io::duplex;

        let (mut client, mut server) = duplex(1024);

        let req = IpcRequest::new(IpcCommand::Ping);
        let write_fut = write_request(&mut client, &req);
        let read_fut = read_request(&mut server);

        let (_, received_req) = tokio::join!(write_fut, read_fut);
        let received_req = received_req.expect("read request");
        assert_eq!(received_req.command, IpcCommand::Ping);

        let resp = IpcResponse::ok_empty(&received_req.request_id);
        let write_resp_fut = write_response(&mut server, &resp);
        let read_resp_fut = read_response(&mut client);

        let (_, received_resp) = tokio::join!(write_resp_fut, read_resp_fut);
        let received_resp = received_resp.expect("read response");
        assert!(received_resp.success);
        assert_eq!(received_resp.request_id, req.request_id);
    }
}
