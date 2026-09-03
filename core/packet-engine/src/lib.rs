//! Core packet engine abstractions, binary discovery, and capability contracts for ZonDPI.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

/// Unique identifier for each networking engine backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineId {
    GoodbyeDpi,
    ByeDpi,
    Native,
}

impl fmt::Display for EngineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineId::GoodbyeDpi => write!(f, "GoodbyeDPI"),
            EngineId::ByeDpi => write!(f, "ByeDPI"),
            EngineId::Native => write!(f, "NativeRust"),
        }
    }
}

impl EngineId {
    /// Returns the standard executable file name for this engine.
    pub fn executable_name(&self) -> &'static str {
        match self {
            EngineId::GoodbyeDpi => "goodbyedpi.exe",
            EngineId::ByeDpi => "ciadpi.exe",
            EngineId::Native => "zondpi-native.exe",
        }
    }

    /// Returns the standard relative subdirectory for this engine.
    pub fn sub_directory(&self) -> &'static str {
        match self {
            EngineId::GoodbyeDpi => "goodbye",
            EngineId::ByeDpi => "byedpi",
            EngineId::Native => "native",
        }
    }
}

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Engine initialization failed: {0}")]
    InitFailed(String),
    #[error("Driver load error: {0}")]
    DriverError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Process supervisor error: {0}")]
    SupervisorError(String),
    #[error("Engine crashed with exit code: {0:?}")]
    EngineCrash(Option<i32>),
    #[error("Health check failed: {0}")]
    HealthCheckFailed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityCompatibilityRating {
    /// Driver-free, pure user-space socket operation (e.g. ByeDPI)
    High,
    /// Requires driver with known, official exclusions
    Medium,
    /// Known active WFP collisions without manual configuration (e.g. WinDivert)
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineCapabilities {
    pub engine_id: EngineId,
    pub engine_name: String,
    pub requires_kernel_driver: bool,
    pub requires_admin: bool,
    pub supports_tcp: bool,
    pub supports_udp: bool,
    pub supports_ipv4: bool,
    pub supports_ipv6: bool,
    pub supports_quic: bool,
    pub supports_system_wide: bool,
    pub supports_per_app: bool,
    pub supports_dns_redirect: bool,
    pub security_compatibility: SecurityCompatibilityRating,
}

impl EngineCapabilities {
    pub fn goodbye_dpi() -> Self {
        Self {
            engine_id: EngineId::GoodbyeDpi,
            engine_name: "GoodbyeDPI-Worker".to_string(),
            requires_kernel_driver: true,
            requires_admin: true,
            supports_tcp: true,
            supports_udp: true,
            supports_ipv4: true,
            supports_ipv6: true,
            supports_quic: true,
            supports_system_wide: true,
            supports_per_app: false,
            supports_dns_redirect: true,
            security_compatibility: SecurityCompatibilityRating::Low,
        }
    }

    pub fn bye_dpi() -> Self {
        Self {
            engine_id: EngineId::ByeDpi,
            engine_name: "ByeDPI-SOCKS5".to_string(),
            requires_kernel_driver: false,
            requires_admin: false,
            supports_tcp: true,
            supports_udp: true,
            supports_ipv4: true,
            supports_ipv6: true,
            supports_quic: false,
            supports_system_wide: false,
            supports_per_app: true,
            supports_dns_redirect: false,
            security_compatibility: SecurityCompatibilityRating::High,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub is_healthy: bool,
    pub driver_available: bool,
    pub packet_capture_working: bool,
    pub packet_reinject_working: bool,
    pub https_connectivity_ok: bool,
    pub latency: Duration,
    pub diagnostic_message: String,
}

impl HealthCheckResult {
    pub fn healthy(latency: Duration, message: impl Into<String>) -> Self {
        Self {
            is_healthy: true,
            driver_available: true,
            packet_capture_working: true,
            packet_reinject_working: true,
            https_connectivity_ok: true,
            latency,
            diagnostic_message: message.into(),
        }
    }

    pub fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            is_healthy: false,
            driver_available: false,
            packet_capture_working: false,
            packet_reinject_working: false,
            https_connectivity_ok: false,
            latency: Duration::from_millis(0),
            diagnostic_message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub running: bool,
    pub active_profile: Option<String>,
    pub packets_processed: u64,
    pub bytes_processed: u64,
    pub uptime_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectivityResult {
    pub success: bool,
    pub target_url: String,
    pub latency_ms: u64,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub target_engine: String,
    pub arguments: Vec<String>,
}

/// Generic engine configuration wrapping profile metadata and runtime options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineConfig {
    pub profile: ProfileDefinition,
    pub socks_port: Option<u16>,
    pub dns_redirect_addr: Option<String>,
}

/// Deterministic discovery and path resolution for native engine worker binaries.
pub struct EngineBinaryResolver;

impl EngineBinaryResolver {
    /// Resolves the absolute path to the requested engine worker executable.
    pub fn resolve(
        engine_id: EngineId,
        custom_base: Option<&Path>,
    ) -> Result<PathBuf, EngineError> {
        let exe_name = engine_id.executable_name();
        let sub_dir = engine_id.sub_directory();
        let mut searched_paths = Vec::new();

        // 1. If explicit custom base path is provided, check candidates within base path first
        if let Some(base) = custom_base {
            let candidates = [
                base.join("engines").join(sub_dir).join(exe_name),
                base.join("artifacts")
                    .join("engines")
                    .join(sub_dir)
                    .join(exe_name),
                base.join("dist")
                    .join("engines")
                    .join(sub_dir)
                    .join(exe_name),
                base.join("dist").join("bin").join(exe_name),
                base.join(exe_name),
            ];
            for cand in candidates {
                searched_paths.push(cand.clone());
                if cand.is_file() {
                    return Ok(cand);
                }
            }
        }

        // 2. Search upwards from current working directory
        if let Ok(cwd) = std::env::current_dir() {
            let mut curr = Some(cwd.as_path());
            while let Some(dir) = curr {
                let candidates = [
                    dir.join("artifacts")
                        .join("engines")
                        .join(sub_dir)
                        .join(exe_name),
                    dir.join("dist")
                        .join("engines")
                        .join(sub_dir)
                        .join(exe_name),
                    dir.join("dist").join("bin").join(exe_name),
                    dir.join("engines").join(sub_dir).join(exe_name),
                ];
                for cand in candidates {
                    if !searched_paths.contains(&cand) {
                        searched_paths.push(cand.clone());
                    }
                    if cand.is_file() {
                        return Ok(cand);
                    }
                }
                curr = dir.parent();
            }
        }

        // 5. Check relative to current running executable
        if let Ok(current_exe) = std::env::current_exe() {
            if let Some(exe_dir) = current_exe.parent() {
                let sibling_engine = exe_dir.join("engines").join(sub_dir).join(exe_name);
                searched_paths.push(sibling_engine.clone());
                if sibling_engine.is_file() {
                    return Ok(sibling_engine);
                }

                let sibling_flat = exe_dir.join(exe_name);
                searched_paths.push(sibling_flat.clone());
                if sibling_flat.is_file() {
                    return Ok(sibling_flat);
                }
            }
        }

        Err(EngineError::InitFailed(format!(
            "Native binary for engine '{}' ({}) was not found. Searched candidate paths: {:?}",
            engine_id, exe_name, searched_paths
        )))
    }
}

#[async_trait]
pub trait NetworkEngine: Send + Sync {
    /// Returns the engine identifier
    fn id(&self) -> EngineId;

    /// Returns the static capability profile of this engine
    fn capabilities(&self) -> EngineCapabilities;

    /// Runs a non-destructive bounded health-check
    async fn health_check(&self) -> Result<HealthCheckResult, EngineError>;

    /// Starts the engine with the supplied profile definition
    async fn start(&mut self, profile: &ProfileDefinition) -> Result<(), EngineError>;

    /// Gracefully stops the engine
    async fn stop(&mut self) -> Result<(), EngineError>;

    /// Restarts the engine with a new profile
    async fn restart(&mut self, profile: &ProfileDefinition) -> Result<(), EngineError>;

    /// Queries current engine status and packet counters
    async fn status(&self) -> Result<EngineStatus, EngineError>;

    /// Verifies end-to-end connectivity to a target URL
    async fn test_connectivity(&self, target_url: &str) -> Result<ConnectivityResult, EngineError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};

    #[test]
    fn test_engine_id_display() {
        assert_eq!(EngineId::GoodbyeDpi.to_string(), "GoodbyeDPI");
        assert_eq!(EngineId::ByeDpi.to_string(), "ByeDPI");
        assert_eq!(EngineId::Native.to_string(), "NativeRust");
    }

    #[test]
    fn test_goodbye_dpi_capabilities() {
        let caps = EngineCapabilities::goodbye_dpi();
        assert_eq!(caps.engine_id, EngineId::GoodbyeDpi);
        assert!(caps.requires_kernel_driver);
        assert!(caps.requires_admin);
        assert!(caps.supports_system_wide);
        assert_eq!(
            caps.security_compatibility,
            SecurityCompatibilityRating::Low
        );
    }

    #[test]
    fn test_bye_dpi_capabilities() {
        let caps = EngineCapabilities::bye_dpi();
        assert_eq!(caps.engine_id, EngineId::ByeDpi);
        assert!(!caps.requires_kernel_driver);
        assert!(!caps.requires_admin);
        assert!(caps.supports_per_app);
        assert_eq!(
            caps.security_compatibility,
            SecurityCompatibilityRating::High
        );
    }

    #[test]
    fn test_health_check_result_constructors() {
        let healthy = HealthCheckResult::healthy(Duration::from_millis(15), "all good");
        assert!(healthy.is_healthy);
        assert!(healthy.driver_available);
        assert_eq!(healthy.latency, Duration::from_millis(15));

        let unhealthy = HealthCheckResult::unhealthy("driver failed");
        assert!(!unhealthy.is_healthy);
        assert!(!unhealthy.driver_available);
    }

    #[test]
    fn test_binary_resolver_with_temp_directory() {
        let temp_dir = std::env::temp_dir().join("zondpi_resolver_test");
        let goodbye_dir = temp_dir.join("engines").join("goodbye");
        fs::create_dir_all(&goodbye_dir).expect("create test dir");
        let dummy_exe = goodbye_dir.join("goodbyedpi.exe");
        File::create(&dummy_exe).expect("create dummy file");

        let resolved = EngineBinaryResolver::resolve(EngineId::GoodbyeDpi, Some(&temp_dir));
        assert!(resolved.is_ok());
        assert_eq!(resolved.unwrap(), dummy_exe);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
