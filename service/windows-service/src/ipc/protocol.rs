//! Versioned IPC protocol definitions for ZonDPI Windows Service.

use serde::{Deserialize, Serialize};
use zondpi_packet_engine::{
    EngineCapabilities, EngineId, EngineStatus, HealthCheckResult, ProfileDefinition,
};

pub const CURRENT_IPC_PROTOCOL_VERSION: u32 = 1;

/// Top-level versioned IPC Request sent from client (e.g. GUI or CLI) to ZonDPI Windows Service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub protocol_version: u32,
    pub request_id: String,
    pub command: ServiceCommand,
}

impl IpcRequest {
    pub fn new(request_id: impl Into<String>, command: ServiceCommand) -> Self {
        Self {
            protocol_version: CURRENT_IPC_PROTOCOL_VERSION,
            request_id: request_id.into(),
            command,
        }
    }
}

/// Commands accepted by the ZonDPI Service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceCommand {
    GetStatus,
    GetCapabilities {
        engine_id: EngineId,
    },
    RunHealthCheck {
        engine_id: EngineId,
    },
    GetRecommendation,
    StartEngine {
        engine_id: EngineId,
        profile: ProfileDefinition,
    },
    StopEngine,
    RestartEngine {
        profile: ProfileDefinition,
    },
    ApplyDns {
        resolvers: Vec<String>,
    },
    RestoreDns,
}

/// Top-level versioned IPC Response sent from ZonDPI Windows Service to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub protocol_version: u32,
    pub request_id: String,
    pub result: ServiceResult,
}

impl IpcResponse {
    pub fn success(request_id: impl Into<String>, payload: ResponsePayload) -> Self {
        Self {
            protocol_version: CURRENT_IPC_PROTOCOL_VERSION,
            request_id: request_id.into(),
            result: ServiceResult::Ok(payload),
        }
    }

    pub fn error(request_id: impl Into<String>, error_code: String, error_message: String) -> Self {
        Self {
            protocol_version: CURRENT_IPC_PROTOCOL_VERSION,
            request_id: request_id.into(),
            result: ServiceResult::Err {
                code: error_code,
                message: error_message,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceResult {
    Ok(ResponsePayload),
    Err { code: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponsePayload {
    Empty,
    Status(EngineStatus),
    Capabilities(EngineCapabilities),
    HealthCheck(HealthCheckResult),
    Recommendation {
        preferred_engine: Option<EngineId>,
        fallback_engine: Option<EngineId>,
        reason: String,
        confidence: f32,
    },
    DnsRestored,
}
