//! Deterministic Compatibility Manager for ZonDPI.
//!
//! # Compatibility Decision Engine
//! Evaluates the host security environment, engine capabilities, and live health check results.
//!
//! Ground Truth Invariants:
//! 1. AV detection is a risk signal, NOT a hard ban ("Kaspersky detected != automatic ByeDPI").
//! 2. Live health-check result takes precedence over passive environment signals.
//! 3. Detector failures or unknown environments must never crash the service.

use crate::security_detector::{
    DetectionStatus, SecurityDetectorTrait, SecurityEnvironment, SystemSecurityDetector,
};
use serde::{Deserialize, Serialize};
use zondpi_packet_engine::{EngineCapabilities, EngineId, HealthCheckResult};

/// Represents a deterministic engine recommendation output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineRecommendation {
    pub preferred_engine: Option<EngineId>,
    pub fallback_engine: Option<EngineId>,
    pub reason: String,
    pub confidence: f32,
}

pub struct CompatibilityPolicy;

impl CompatibilityPolicy {
    /// Evaluates environment risk and health check telemetry to produce a deterministic recommendation.
    pub fn evaluate(
        env: &SecurityEnvironment,
        goodbye_caps: &EngineCapabilities,
        byedpi_caps: &EngineCapabilities,
        goodbye_health: &HealthCheckResult,
        byedpi_health: &HealthCheckResult,
    ) -> EngineRecommendation {
        let kaspersky_detected = env.contains_product("kaspersky");

        // Scenario 1: GoodbyeDPI is fully healthy
        if goodbye_health.is_healthy {
            let (confidence, reason) = if kaspersky_detected {
                (
                    0.85,
                    "GoodbyeDPI passed system health verification. Kaspersky is active on host: WinDivert is operating normally, with driver-free ByeDPI as fallback if WFP interference occurs.".to_string(),
                )
            } else if env.detection_status != DetectionStatus::Available {
                (
                    0.90,
                    "GoodbyeDPI passed health verification (Security detection status: unavailable/failed; engine health prioritized).".to_string(),
                )
            } else {
                (
                    0.95,
                    "GoodbyeDPI verified healthy for system-wide packet evasion.".to_string(),
                )
            };

            let fallback = if byedpi_health.is_healthy {
                Some(byedpi_caps.engine_id)
            } else {
                None
            };

            return EngineRecommendation {
                preferred_engine: Some(goodbye_caps.engine_id),
                fallback_engine: fallback,
                reason,
                confidence,
            };
        }

        // Scenario 2: GoodbyeDPI failed health check, ByeDPI is healthy
        if byedpi_health.is_healthy {
            let reason = if kaspersky_detected {
                "GoodbyeDPI health check failed in Kaspersky environment (driver load / packet capture error). Safely switched to driver-free ByeDPI SOCKS5 engine to prevent kernel filter collision.".to_string()
            } else {
                format!(
                    "GoodbyeDPI is unhealthy ({}); using driver-free ByeDPI engine.",
                    goodbye_health.diagnostic_message
                )
            };

            return EngineRecommendation {
                preferred_engine: Some(byedpi_caps.engine_id),
                fallback_engine: None,
                reason,
                confidence: 0.95,
            };
        }

        // Scenario 3: Both engines are unhealthy
        EngineRecommendation {
            preferred_engine: None,
            fallback_engine: None,
            reason: format!(
                "Both engines failed health verification. GoodbyeDPI: '{}', ByeDPI: '{}'. Action required: verify file integrity and network adapter status.",
                goodbye_health.diagnostic_message, byedpi_health.diagnostic_message
            ),
            confidence: 0.0,
        }
    }
}

pub struct CompatibilityManager;

impl CompatibilityManager {
    /// Recommends engine using the default system detector.
    pub fn recommend_with_system_detector(
        goodbye_caps: &EngineCapabilities,
        byedpi_caps: &EngineCapabilities,
        goodbye_health: &HealthCheckResult,
        byedpi_health: &HealthCheckResult,
    ) -> (EngineRecommendation, SecurityEnvironment) {
        let detector = SystemSecurityDetector;
        Self::recommend_with_detector(
            &detector,
            goodbye_caps,
            byedpi_caps,
            goodbye_health,
            byedpi_health,
        )
    }

    /// Recommends engine using any detector implementing SecurityDetectorTrait (enables mock injection).
    pub fn recommend_with_detector<D: SecurityDetectorTrait>(
        detector: &D,
        goodbye_caps: &EngineCapabilities,
        byedpi_caps: &EngineCapabilities,
        goodbye_health: &HealthCheckResult,
        byedpi_health: &HealthCheckResult,
    ) -> (EngineRecommendation, SecurityEnvironment) {
        let env = detector.detect();
        let recommendation = CompatibilityPolicy::evaluate(
            &env,
            goodbye_caps,
            byedpi_caps,
            goodbye_health,
            byedpi_health,
        );
        (recommendation, env)
    }
}
