//! Central EngineManager coordinating single active engine lifecycle, transactional switching, and Auto mode resolution.

use std::fs;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::engine_adapter::{
    ByeDpiAdapter, ByeDpiAdapterError, GoodbyeAdapterError, GoodbyeDpiAdapter,
};
use crate::runtime_paths::{PathError, RuntimePaths};
use crate::supervisor::{ProcessSupervisor, SupervisorConfig, SupervisorState};
use zondpi_compatibility::{
    CompatibilityPolicy, EngineRecommendation, SecurityDetectorTrait, SystemSecurityDetector,
};
use zondpi_dns::{
    AdapterDnsOps, DnsCompatibilityController, DnsManagerError, DnsProvider,
};
use zondpi_ipc_protocol::{
    HealthStatusDto, LogEntryDto, ProfileSummaryDto, RecommendationDto, ServiceStatusDto,
};
use zondpi_packet_engine::{EngineCapabilities, EngineId, HealthCheckResult, ProfileDefinition};

#[derive(Error, Debug)]
pub enum EngineManagerError {
    #[error("Profile error: {0}")]
    Path(#[from] PathError),
    #[error("Profile JSON deserialization error: {0}")]
    ProfileJson(#[from] serde_json::Error),
    #[error("GoodbyeDPI engine error: {0}")]
    Goodbye(#[from] GoodbyeAdapterError),
    #[error("ByeDPI engine error: {0}")]
    ByeDpi(#[from] ByeDpiAdapterError),
    #[error("DNS compatibility error: {0}")]
    Dns(#[from] DnsManagerError),
    #[error("Engine start failed health check verification: {0}")]
    HealthCheckFailed(String),
    #[error("Unknown or unsupported engine name: '{0}' (expected 'goodbye' or 'byedpi')")]
    UnknownEngine(String),
    #[error("Auto mode failed to activate any available engine: {0}")]
    AutoModeFailed(String),
    #[error("Port already in use: {0}")]
    PortInUse(u16),
}

/// Operational state mode of the EngineManager.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveMode {
    Idle,
    Manual {
        engine: EngineId,
    },
    Auto {
        preferred: EngineId,
        fallback_reason: Option<String>,
    },
}

impl std::fmt::Display for ActiveMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActiveMode::Idle => write!(f, "Idle"),
            ActiveMode::Manual { engine } => write!(f, "Manual ({})", engine),
            ActiveMode::Auto { preferred, .. } => write!(f, "Auto ({})", preferred),
        }
    }
}

struct ManagerInner {
    goodbye_adapter: GoodbyeDpiAdapter,
    byedpi_adapter: ByeDpiAdapter,
    dns_controller: DnsCompatibilityController,
    dns_provider: DnsProvider,
    kaspersky_dns_override_applied: bool,
    active_mode: ActiveMode,
    active_engine: Option<EngineId>,
    requested_engine: Option<EngineId>,
    active_profile: Option<String>,
    last_error: Option<String>,
}

/// Thread-safe central engine manager with serialized state mutation and transactional switching.
#[derive(Clone)]
pub struct EngineManager {
    inner: Arc<Mutex<ManagerInner>>,
    runtime_paths: RuntimePaths,
    security_detector: Arc<dyn SecurityDetectorTrait>,
    service_start_time: Instant,
    mutation_lock: Arc<Mutex<()>>,
}

impl EngineManager {
    /// Creates a new EngineManager using default runtime paths and system security detector.
    pub fn new(runtime_paths: RuntimePaths) -> Self {
        let dummy_sup_g = ProcessSupervisor::new(SupervisorConfig::new(
            "GoodbyeDPI-Worker",
            std::path::PathBuf::from("goodbyedpi.exe"),
            vec![],
        ));
        let dummy_sup_b = ProcessSupervisor::new(SupervisorConfig::new(
            "ByeDPI-Worker",
            std::path::PathBuf::from("ciadpi.exe"),
            vec![],
        ));

        let dns_controller = DnsCompatibilityController::new(&runtime_paths.data_root);
        // Boot/crash recovery: If previous session terminated abnormally while DNS override was active,
        // restore original DNS safely so adapter is not permanently modified.
        let _ = dns_controller.handle_boot_recovery(false, DnsProvider::Automatic);

        let inner = ManagerInner {
            goodbye_adapter: GoodbyeDpiAdapter::new(dummy_sup_g),
            byedpi_adapter: ByeDpiAdapter::new(dummy_sup_b),
            dns_controller,
            dns_provider: DnsProvider::Automatic,
            kaspersky_dns_override_applied: false,
            active_mode: ActiveMode::Idle,
            active_engine: None,
            requested_engine: None,
            active_profile: None,
            last_error: None,
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
            runtime_paths,
            security_detector: Arc::new(SystemSecurityDetector),
            service_start_time: Instant::now(),
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Creates an EngineManager with custom security detector for testing.
    pub fn with_detector(
        runtime_paths: RuntimePaths,
        detector: Arc<dyn SecurityDetectorTrait>,
    ) -> Self {
        let mut mgr = Self::new(runtime_paths);
        mgr.security_detector = detector;
        mgr
    }

    /// Creates an EngineManager with custom security detector and mock DNS ops for testing.
    pub fn with_detector_and_dns_ops(
        runtime_paths: RuntimePaths,
        detector: Arc<dyn SecurityDetectorTrait>,
        dns_ops: Box<dyn AdapterDnsOps>,
    ) -> Self {
        let dummy_sup_g = ProcessSupervisor::new(SupervisorConfig::new(
            "GoodbyeDPI-Worker",
            std::path::PathBuf::from("goodbyedpi.exe"),
            vec![],
        ));
        let dummy_sup_b = ProcessSupervisor::new(SupervisorConfig::new(
            "ByeDPI-Worker",
            std::path::PathBuf::from("ciadpi.exe"),
            vec![],
        ));

        let dns_controller = DnsCompatibilityController::with_ops(&runtime_paths.data_root, dns_ops);
        let _ = dns_controller.handle_boot_recovery(false, DnsProvider::Automatic);

        let inner = ManagerInner {
            goodbye_adapter: GoodbyeDpiAdapter::new(dummy_sup_g),
            byedpi_adapter: ByeDpiAdapter::new(dummy_sup_b),
            dns_controller,
            dns_provider: DnsProvider::Automatic,
            kaspersky_dns_override_applied: false,
            active_mode: ActiveMode::Idle,
            active_engine: None,
            requested_engine: None,
            active_profile: None,
            last_error: None,
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
            runtime_paths,
            security_detector: detector,
            service_start_time: Instant::now(),
            mutation_lock: Arc::new(Mutex::new(())),
        }
    }


    /// Loads and parses a profile definition from disk safely.
    pub fn load_profile(&self, profile_id: &str) -> Result<ProfileDefinition, EngineManagerError> {
        let path = self.runtime_paths.resolve_profile_path(profile_id)?;
        let content = fs::read_to_string(&path)
            .map_err(|_e| PathError::ProfileNotFound(profile_id.to_string(), vec![path]))?;
        let profile: ProfileDefinition = serde_json::from_str(&content)?;
        Ok(profile)
    }

    /// Starts a specific networking engine in Manual mode.
    pub async fn start_engine(
        &self,
        engine: EngineId,
        profile_id: &str,
    ) -> Result<(), EngineManagerError> {
        let _guard = self.mutation_lock.lock().await;
        self.start_engine_internal(engine, profile_id, ActiveMode::Manual { engine })
            .await
    }

    /// Evaluates compatibility policy to determine best engine recommendation.
    pub fn evaluate_recommendation(&self) -> EngineRecommendation {
        let sec_env = self.security_detector.detect();
        let goodbye_caps = EngineCapabilities::goodbye_dpi();
        let byedpi_caps = EngineCapabilities::bye_dpi();
        let goodbye_health = HealthCheckResult::healthy(Duration::from_millis(5), "Ready");
        let byedpi_health = HealthCheckResult::healthy(Duration::from_millis(5), "Ready");

        CompatibilityPolicy::evaluate(
            &sec_env,
            &goodbye_caps,
            &byedpi_caps,
            &goodbye_health,
            &byedpi_health,
        )
    }

    /// Starts the engine using Auto compatibility selection and transparent fallback.
    pub async fn start_auto(&self, profile_id: &str) -> Result<(), EngineManagerError> {
        let _guard = self.mutation_lock.lock().await;

        let recommendation = self.evaluate_recommendation();
        let preferred = recommendation
            .preferred_engine
            .unwrap_or(EngineId::GoodbyeDpi);

        info!(
            preferred = %preferred,
            reason = %recommendation.reason,
            "Auto mode resolved preferred engine"
        );

        // Attempt preferred engine first
        let preferred_res = self
            .start_engine_internal(
                preferred,
                profile_id,
                ActiveMode::Auto {
                    preferred,
                    fallback_reason: None,
                },
            )
            .await;

        match preferred_res {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!(
                    preferred = %preferred,
                    error = %e,
                    "Preferred engine failed to start in Auto mode; triggering fallback"
                );

                // Determine fallback alternative
                let fallback = recommendation.fallback_engine.unwrap_or(match preferred {
                    EngineId::GoodbyeDpi => EngineId::ByeDpi,
                    EngineId::ByeDpi => EngineId::GoodbyeDpi,
                    EngineId::Native => EngineId::ByeDpi,
                });

                let fallback_reason = format!("{} startup/health check failed: {}", preferred, e);

                // Attempt fallback engine
                match self
                    .start_engine_internal(
                        fallback,
                        profile_id,
                        ActiveMode::Auto {
                            preferred,
                            fallback_reason: Some(fallback_reason),
                        },
                    )
                    .await
                {
                    Ok(()) => {
                        info!(fallback = %fallback, "Fallback engine started successfully in Auto mode");
                        Ok(())
                    }
                    Err(fallback_err) => {
                        error!(
                            fallback = %fallback,
                            error = %fallback_err,
                            "Fallback engine also failed in Auto mode"
                        );
                        let mut inner = self.inner.lock().await;
                        inner.active_mode = ActiveMode::Idle;
                        inner.last_error = Some(format!(
                            "Both {} and {} failed to start. Last error: {}",
                            preferred, fallback, fallback_err
                        ));
                        Err(EngineManagerError::AutoModeFailed(format!(
                            "Preferred ({}) error: {}; Fallback ({}) error: {}",
                            preferred, e, fallback, fallback_err
                        )))
                    }
                }
            }
        }
    }

    /// Resolves the effective profile to apply, enforcing that Kaspersky environments
    /// always use the validated turkey-default -5 semantics regardless of ISP-specific recommendations.
    pub fn resolve_effective_profile<'a>(&self, engine: EngineId, requested_profile: &'a str) -> &'a str {
        let sec_env = self.security_detector.detect();
        let kaspersky_detected = sec_env.contains_product("kaspersky");
        if kaspersky_detected && engine == EngineId::GoodbyeDpi {
            "turkey-default"
        } else {
            requested_profile
        }
    }

    /// Internal transactional start procedure.
    async fn start_engine_internal(
        &self,
        engine: EngineId,
        profile_id: &str,
        mode: ActiveMode,
    ) -> Result<(), EngineManagerError> {
        let sec_env = self.security_detector.detect();
        let kaspersky_detected = sec_env.contains_product("kaspersky");

        // Hard requirement 5: When Kaspersky is detected, force validated turkey-default -5 semantics
        // regardless of unvalidated ISP-specific profile recommendation (such as superonline -9).
        let effective_profile_id = self.resolve_effective_profile(engine, profile_id);

        let profile = self.load_profile(effective_profile_id)?;
        let custom_base = Some(self.runtime_paths.install_root.as_path());

        let mut inner = self.inner.lock().await;
        inner.requested_engine = Some(engine);

        // 1. Stop any currently active engine and clear any active DNS override first
        if inner.active_engine.is_some() {
            let _ = inner.goodbye_adapter.stop().await;
            let _ = inner.byedpi_adapter.stop().await;
            if inner.kaspersky_dns_override_applied {
                let _ = inner.dns_controller.restore_dns();
                inner.kaspersky_dns_override_applied = false;
            }
            inner.active_engine = None;
        }

        // 2. If Kaspersky is active and using GoodbyeDpi:
        // Apply ZonDPI-managed adapter DNS override and suppress packet DNS redirect
        let suppress_dns_redirect = kaspersky_detected;
        if engine == EngineId::GoodbyeDpi && kaspersky_detected {
            if inner.dns_provider.should_override_system() {
                match inner.dns_controller.apply_dns(inner.dns_provider) {
                    Ok(_) => {
                        inner.kaspersky_dns_override_applied = true;
                    }
                    Err(e) => {
                        inner.last_error = Some(e.to_string());
                        return Err(EngineManagerError::Dns(e));
                    }
                }
            }
        }

        // 3. Spawn and verify requested engine
        match engine {
            EngineId::GoodbyeDpi => {
                if let Err(e) = inner
                    .goodbye_adapter
                    .start(&profile, custom_base, suppress_dns_redirect)
                    .await
                {
                    if inner.kaspersky_dns_override_applied {
                        let _ = inner.dns_controller.restore_dns();
                        inner.kaspersky_dns_override_applied = false;
                    }
                    inner.last_error = Some(e.to_string());
                    return Err(EngineManagerError::Goodbye(e));
                }

                // Short delay for worker initialization
                tokio::time::sleep(Duration::from_millis(300)).await;

                let health = inner.goodbye_adapter.health_check().await;
                if !health.is_healthy {
                    let _ = inner.goodbye_adapter.stop().await;
                    if inner.kaspersky_dns_override_applied {
                        let _ = inner.dns_controller.restore_dns();
                        inner.kaspersky_dns_override_applied = false;
                    }
                    inner.last_error = Some(health.diagnostic_message.clone());
                    return Err(EngineManagerError::HealthCheckFailed(
                        health.diagnostic_message,
                    ));
                }
            }
            EngineId::ByeDpi => {
                if let Err(e) = inner.byedpi_adapter.start(&profile, custom_base).await {
                    inner.last_error = Some(e.to_string());
                    if let ByeDpiAdapterError::PortAlreadyInUse(p) = e {
                        return Err(EngineManagerError::PortInUse(p));
                    }
                    return Err(EngineManagerError::ByeDpi(e));
                }

                // Short delay for worker initialization
                tokio::time::sleep(Duration::from_millis(300)).await;

                let health = inner.byedpi_adapter.health_check().await;
                if !health.is_healthy {
                    let _ = inner.byedpi_adapter.stop().await;
                    inner.last_error = Some(health.diagnostic_message.clone());
                    return Err(EngineManagerError::HealthCheckFailed(
                        health.diagnostic_message,
                    ));
                }
            }
            EngineId::Native => {
                return Err(EngineManagerError::UnknownEngine("NativeRust".to_string()));
            }
        }

        // Commit active state
        inner.active_engine = Some(engine);
        inner.active_profile = Some(effective_profile_id.to_string());
        inner.active_mode = mode;
        inner.last_error = None;
        info!(
            engine = %engine,
            profile = %effective_profile_id,
            kaspersky_detected,
            "Active networking engine successfully committed"
        );
        Ok(())
    }

    /// Stops the currently active engine gracefully.
    pub async fn stop_engine(&self) -> Result<(), EngineManagerError> {
        let _guard = self.mutation_lock.lock().await;
        let mut inner = self.inner.lock().await;

        let _ = inner.goodbye_adapter.stop().await;
        let _ = inner.byedpi_adapter.stop().await;
        if inner.kaspersky_dns_override_applied {
            let _ = inner.dns_controller.restore_dns();
            inner.kaspersky_dns_override_applied = false;
        }

        inner.active_engine = None;
        inner.active_profile = None;
        inner.active_mode = ActiveMode::Idle;
        inner.last_error = None;
        info!("Active networking engine stopped");
        Ok(())
    }

    /// Sets the DNS provider preference for compatibility mode.
    pub async fn set_dns_provider(&self, provider: DnsProvider) {
        let mut inner = self.inner.lock().await;
        inner.dns_provider = provider;
    }

    /// Transactionally switches active engine and applies a new profile.
    pub async fn switch_engine(
        &self,
        target_engine: EngineId,
        profile_id: &str,
    ) -> Result<(), EngineManagerError> {
        let _guard = self.mutation_lock.lock().await;
        self.start_engine_internal(
            target_engine,
            profile_id,
            ActiveMode::Manual {
                engine: target_engine,
            },
        )
        .await
    }

    /// Returns a structured live status DTO of the service and active engine.
    pub async fn get_status(&self) -> ServiceStatusDto {
        let inner = self.inner.lock().await;
        let service_uptime = self.service_start_time.elapsed().as_secs();

        let (engine_pid, engine_uptime, service_state, mut health) = match inner.active_engine {
            Some(EngineId::GoodbyeDpi) => {
                let st = inner.goodbye_adapter.supervisor().status().await;
                let h = inner.goodbye_adapter.health_check().await;
                let state_str = st.state.to_string();
                let engine_health = if h.is_healthy {
                    "Paket filtresi etkin".to_string()
                } else if st.state == SupervisorState::Running {
                    "Paket filtresi hazırlanıyor".to_string()
                } else {
                    "Paket filtresi etkinleştirilemedi".to_string()
                };
                let health_dto = HealthStatusDto {
                    is_healthy: h.is_healthy,
                    latency_ms: h.latency.as_millis() as u64,
                    message: h.diagnostic_message,
                    process_health: state_str.clone(),
                    engine_health,
                    network_effectiveness: "Bilinmiyor".to_string(),
                };
                (st.pid, st.uptime_seconds, state_str, health_dto)
            }
            Some(EngineId::ByeDpi) => {
                let st = inner.byedpi_adapter.supervisor().status().await;
                let h = inner.byedpi_adapter.health_check().await;
                let state_str = st.state.to_string();
                let engine_health = if h.is_healthy {
                    "Uyumluluk modu etkin".to_string()
                } else {
                    "Uyumluluk modu devre dışı".to_string()
                };
                let health_dto = HealthStatusDto {
                    is_healthy: h.is_healthy,
                    latency_ms: h.latency.as_millis() as u64,
                    message: h.diagnostic_message,
                    process_health: state_str.clone(),
                    engine_health,
                    network_effectiveness: "Bilinmiyor".to_string(),
                };
                (st.pid, st.uptime_seconds, state_str, health_dto)
            }
            _ => (
                None,
                None,
                SupervisorState::Stopped.to_string(),
                HealthStatusDto {
                    is_healthy: true,
                    latency_ms: 0,
                    message: "ZonDPI koruması kapalı".to_string(),
                    process_health: "Durduruldu".to_string(),
                    engine_health: "Devre Dışı".to_string(),
                    network_effectiveness: "Bilinmiyor".to_string(),
                },
            ),
        };

        let (mode_str, fallback_reason) = match &inner.active_mode {
            ActiveMode::Idle => ("Kapalı".to_string(), None),
            ActiveMode::Manual { .. } => ("Manuel".to_string(), None),
            ActiveMode::Auto {
                fallback_reason, ..
            } => ("Otomatik".to_string(), fallback_reason.clone()),
        };

        let sec_env = self.security_detector.detect();
        let kaspersky_detected = sec_env.contains_product("kaspersky");

        let (security_compatibility, dns_compatibility_method, dns_provider) = if kaspersky_detected {
            (
                Some("Kaspersky".to_string()),
                Some("Sistem DNS yapılandırması".to_string()),
                Some("Cloudflare".to_string()),
            )
        } else {
            (None, None, None)
        };

        // If Kaspersky compatibility DNS was applied and engine is running, verify effectiveness
        if inner.kaspersky_dns_override_applied && health.is_healthy {
            let probe_ok = inner
                .dns_controller
                .ops()
                .verify_dns_resolution("one.one.one.one")
                .unwrap_or(false);
            health.network_effectiveness = if probe_ok {
                "Doğrulandı".to_string()
            } else {
                "Etkin".to_string()
            };
        }

        let _rec = self.evaluate_recommendation();
        let rec_str = "Otomatik (Uyumlu)".to_string();

        ServiceStatusDto {
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: service_uptime,
            service_state,
            mode: mode_str,
            active_engine: inner.active_engine.map(|e| e.to_string()),
            requested_engine: inner.requested_engine.map(|e| e.to_string()),
            active_profile: inner.active_profile.clone(),
            engine_pid,
            engine_uptime_seconds: engine_uptime,
            health,
            compatibility_recommendation: Some(rec_str),
            fallback_reason,
            last_error: inner.last_error.clone(),
            security_compatibility,
            dns_compatibility_method,
            dns_provider,
        }
    }

    /// Queries the live compatibility recommendation.
    pub async fn get_recommendation(&self) -> RecommendationDto {
        let sec = self.security_detector.detect();
        let rec = self.evaluate_recommendation();
        let product_names: Vec<String> = sec
            .products
            .iter()
            .map(|p| p.display_name.clone())
            .collect();
        RecommendationDto {
            detected_security_products: product_names,
            recommended_engine: rec
                .preferred_engine
                .map(|e| e.to_string())
                .unwrap_or_else(|| "None".to_string()),
            rationale: rec.reason,
            compatibility_rating: format!("Confidence: {:.0}%", rec.confidence * 100.0),
        }
    }

    /// Discovers all available profiles in configured profile roots.
    pub async fn list_profiles(&self) -> Vec<ProfileSummaryDto> {
        let mut profiles = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for root in &self.runtime_paths.profile_roots {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(def) = serde_json::from_str::<ProfileDefinition>(&content) {
                                if seen_ids.insert(def.id.clone()) {
                                    profiles.push(ProfileSummaryDto {
                                        id: def.id,
                                        name: def.name,
                                        description: def.description,
                                        target_engine: def.target_engine,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        profiles.sort_by(|a, b| a.id.cmp(&b.id));
        profiles
    }

    /// Retrieves recent log lines from the active engine's ring buffer.
    pub async fn get_recent_logs(&self, count: Option<usize>) -> Vec<LogEntryDto> {
        let inner = self.inner.lock().await;
        match inner.active_engine {
            Some(EngineId::GoodbyeDpi) => {
                inner
                    .goodbye_adapter
                    .supervisor()
                    .get_recent_logs(count)
                    .await
            }
            Some(EngineId::ByeDpi) => {
                inner
                    .byedpi_adapter
                    .supervisor()
                    .get_recent_logs(count)
                    .await
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zondpi_compatibility::{
        DetectionStatus, MockSecurityDetector, SecurityEnvironment, SecurityProduct,
    };

    #[tokio::test]
    async fn test_engine_manager_initial_status_is_idle() {
        let paths = RuntimePaths::discover().expect("discover paths");
        let mgr = EngineManager::new(paths);
        let status = mgr.get_status().await;
        assert_eq!(status.service_state, "Stopped");
        assert_eq!(status.active_engine, None);
        assert_eq!(status.mode, "Kapalı");
    }

    #[tokio::test]
    async fn test_list_profiles_finds_turkey_presets() {
        let paths = RuntimePaths::discover().expect("discover paths");
        let mgr = EngineManager::new(paths);
        let profiles = mgr.list_profiles().await;
        assert!(!profiles.is_empty());
        let ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"turkey-default"));
        assert!(ids.contains(&"superonline-default"));
        assert!(ids.contains(&"byedpi-kaspersky-mode"));
    }

    #[tokio::test]
    async fn test_auto_mode_recommendation_with_mock_detector() {
        let paths = RuntimePaths::discover().expect("discover paths");
        let env = SecurityEnvironment::new(
            vec![SecurityProduct {
                display_name: "Kaspersky Premium".to_string(),
                product_state: 0,
                path_to_signed_product_exe: None,
                path_to_signed_reporting_exe: None,
            }],
            DetectionStatus::Available,
        );
        let mock = Arc::new(MockSecurityDetector::new(env));
        let mgr = EngineManager::with_detector(paths, mock);
        let rec = mgr.get_recommendation().await;
        assert_eq!(rec.recommended_engine, "GoodbyeDPI");
        assert!(rec
            .detected_security_products
            .contains(&"Kaspersky Premium".to_string()));
    }

    #[tokio::test]
    async fn test_status_presentation_contains_no_internal_names_in_user_strings() {
        let paths = RuntimePaths::discover().expect("discover paths");
        let mgr = EngineManager::new(paths);
        let status = mgr.get_status().await;
        let forbidden = [
            "GoodbyeDPI",
            "goodbyedpi",
            "ByeDPI",
            "byedpi",
            "ciadpi",
            "WinDivert",
            "windivert",
        ];
        for name in &forbidden {
            assert!(
                !status.mode.contains(name),
                "mode contains forbidden name: {}",
                name
            );
            assert!(
                !status.health.process_health.contains(name),
                "process_health contains forbidden name: {}",
                name
            );
            assert!(
                !status.health.engine_health.contains(name),
                "engine_health contains forbidden name: {}",
                name
            );
            assert!(
                !status.health.message.contains(name),
                "message contains forbidden name: {}",
                name
            );
            if let Some(rec) = &status.compatibility_recommendation {
                assert!(
                    !rec.contains(name),
                    "recommendation contains forbidden name: {}",
                    name
                );
            }
        }
    }

    #[tokio::test]
    async fn test_kaspersky_detected_plus_superonline_forces_validated_modeset_5_and_adapter_dns() {
        use crate::engine_adapter::goodbye::GoodbyeConfig;

        let paths = RuntimePaths::discover().expect("discover paths");
        let env = SecurityEnvironment::new(
            vec![SecurityProduct {
                display_name: "Kaspersky Premium".to_string(),
                product_state: 0,
                path_to_signed_product_exe: None,
                path_to_signed_reporting_exe: None,
            }],
            DetectionStatus::Available,
        );
        let mock = Arc::new(MockSecurityDetector::new(env));
        let mgr = EngineManager::with_detector(paths.clone(), mock);

        // Even though user or ISP detection requests "superonline-default",
        // Kaspersky presence MUST override to "turkey-default" (-5 semantics)
        let resolved = mgr.resolve_effective_profile(EngineId::GoodbyeDpi, "superonline-default");
        assert_eq!(
            resolved, "turkey-default",
            "Kaspersky must override superonline to turkey-default"
        );

        // Load the profile and verify argument generation
        let profile = mgr.load_profile(resolved).expect("load turkey-default");
        let config = GoodbyeConfig::from_profile(&profile).expect("parse goodbye config");

        // When Kaspersky is detected, packet DNS redirect is suppressed
        let args = config.to_command_args_filtered(false);

        // 1. Must contain validated -5 packet strategy
        assert!(args.contains(&std::ffi::OsString::from("-f")));
        assert!(args.contains(&std::ffi::OsString::from("2")));
        assert!(args.contains(&std::ffi::OsString::from("-e")));
        assert!(args.contains(&std::ffi::OsString::from("--native-frag")));
        assert!(args.contains(&std::ffi::OsString::from("--reverse-frag")));
        assert!(args.contains(&std::ffi::OsString::from("--auto-ttl")));
        assert!(args.contains(&std::ffi::OsString::from("1-4-10")));
        assert!(args.contains(&std::ffi::OsString::from("--max-payload")));
        assert!(args.contains(&std::ffi::OsString::from("1200")));

        // 2. Must NOT contain -9 flags from superonline
        assert!(!args.contains(&std::ffi::OsString::from("-9")));
        assert!(!args.contains(&std::ffi::OsString::from("--wrong-chksum")));
        assert!(!args.contains(&std::ffi::OsString::from("--wrong-seq")));

        // 3. Must NOT contain packet DNS redirect
        assert!(!args.contains(&std::ffi::OsString::from("--dns-addr")));
        assert!(!args.contains(&std::ffi::OsString::from("--dns-port")));
        assert!(!args.contains(&std::ffi::OsString::from("--dnsv6-addr")));
        assert!(!args.contains(&std::ffi::OsString::from("--dnsv6-port")));
    }
}
