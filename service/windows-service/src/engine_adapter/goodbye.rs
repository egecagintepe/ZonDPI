//! GoodbyeDPI engine adapter, typed argument builder, and driver availability verification.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;
use tracing::info;

use crate::supervisor::{ProcessSupervisor, SupervisorConfig, SupervisorError, SupervisorState};
use zondpi_packet_engine::{EngineBinaryResolver, EngineId, HealthCheckResult, ProfileDefinition};

#[derive(Error, Debug)]
pub enum GoodbyeAdapterError {
    #[error("Profile target engine mismatch: expected 'goodbye', got '{0}'")]
    EngineMismatch(String),
    #[error("Failed to resolve GoodbyeDPI worker binary: {0}")]
    BinaryNotFound(String),
    #[error("WinDivert driver or DLL dependency missing: {0}")]
    DriverDependencyMissing(String),
    #[error("Process supervisor error: {0}")]
    Supervisor(#[from] SupervisorError),
    #[error("Invalid argument in profile: {0}")]
    InvalidArgument(String),
}

/// Strongly-typed configuration model for GoodbyeDPI parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoodbyeConfig {
    pub block_passive_dpi: bool,
    pub block_quic: bool,
    pub replace_host: bool,
    pub remove_space: bool,
    pub mix_host_case: bool,
    pub http_frag_size: Option<u16>,
    pub https_frag_size: Option<u16>,
    pub auto_ttl: Option<String>,
    pub set_ttl: Option<u8>,
    pub min_ttl: Option<u8>,
    pub wrong_seq: bool,
    pub wrong_chksum: bool,
    pub native_frag: bool,
    pub reverse_frag: bool,
    pub max_payload: Option<u32>,
    pub dns_addr: Option<String>,
    pub dns_port: Option<u16>,
    pub dnsv6_addr: Option<String>,
    pub dnsv6_port: Option<u16>,
    pub custom_args: Vec<String>,
}

impl GoodbyeConfig {
    /// Builds typed GoodbyeConfig from a validated JSON profile definition.
    pub fn from_profile(profile: &ProfileDefinition) -> Result<Self, GoodbyeAdapterError> {
        let target = profile.target_engine.to_lowercase();
        if target != "goodbye" && target != "goodbyedpi" {
            return Err(GoodbyeAdapterError::EngineMismatch(
                profile.target_engine.clone(),
            ));
        }

        let mut config = Self::default();
        let mut iter = profile.arguments.iter().peekable();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-p" => config.block_passive_dpi = true,
                "-q" => config.block_quic = true,
                "-r" => config.replace_host = true,
                "-s" => config.remove_space = true,
                "-m" => config.mix_host_case = true,
                "--wrong-seq" => config.wrong_seq = true,
                "--wrong-chksum" => config.wrong_chksum = true,
                "--native-frag" => config.native_frag = true,
                "--reverse-frag" => config.reverse_frag = true,
                "-f" => {
                    if let Some(val) = iter.next() {
                        let parsed: u16 = val.parse().map_err(|_| {
                            GoodbyeAdapterError::InvalidArgument(format!(
                                "Invalid HTTP fragmentation size: {}",
                                val
                            ))
                        })?;
                        config.http_frag_size = Some(parsed);
                    }
                }
                "-e" => {
                    if let Some(val) = iter.next() {
                        let parsed: u16 = val.parse().map_err(|_| {
                            GoodbyeAdapterError::InvalidArgument(format!(
                                "Invalid HTTPS fragmentation size: {}",
                                val
                            ))
                        })?;
                        config.https_frag_size = Some(parsed);
                    }
                }
                "--auto-ttl" => {
                    if let Some(val) = iter.peek() {
                        if !val.starts_with('-') {
                            config.auto_ttl = Some((*val).clone());
                            iter.next();
                        } else {
                            config.auto_ttl = Some("1-4-10".to_string());
                        }
                    } else {
                        config.auto_ttl = Some("1-4-10".to_string());
                    }
                    // Auto-TTL and Set-TTL are strictly mutually exclusive
                    config.set_ttl = None;
                }
                "--set-ttl" => {
                    if let Some(val) = iter.next() {
                        let parsed: u8 = val.parse().map_err(|_| {
                            GoodbyeAdapterError::InvalidArgument(format!(
                                "Invalid set-ttl value: {}",
                                val
                            ))
                        })?;
                        config.set_ttl = Some(parsed);
                        // Setting explicit TTL invalidates auto-ttl
                        config.auto_ttl = None;
                    }
                }
                "--min-ttl" => {
                    if let Some(val) = iter.next() {
                        let parsed: u8 = val.parse().map_err(|_| {
                            GoodbyeAdapterError::InvalidArgument(format!(
                                "Invalid min-ttl value: {}",
                                val
                            ))
                        })?;
                        config.min_ttl = Some(parsed);
                    }
                }
                "--max-payload" => {
                    if let Some(val) = iter.next() {
                        let parsed: u32 = val.parse().map_err(|_| {
                            GoodbyeAdapterError::InvalidArgument(format!(
                                "Invalid max-payload value: {}",
                                val
                            ))
                        })?;
                        config.max_payload = Some(parsed);
                    }
                }
                "--dns-addr" => {
                    if let Some(val) = iter.next() {
                        config.dns_addr = Some(val.clone());
                    }
                }
                "--dns-port" => {
                    if let Some(val) = iter.next() {
                        let parsed: u16 = val.parse().map_err(|_| {
                            GoodbyeAdapterError::InvalidArgument(format!(
                                "Invalid dns-port: {}",
                                val
                            ))
                        })?;
                        config.dns_port = Some(parsed);
                    }
                }
                "--dnsv6-addr" => {
                    if let Some(val) = iter.next() {
                        config.dnsv6_addr = Some(val.clone());
                    }
                }
                "--dnsv6-port" => {
                    if let Some(val) = iter.next() {
                        let parsed: u16 = val.parse().map_err(|_| {
                            GoodbyeAdapterError::InvalidArgument(format!(
                                "Invalid dnsv6-port: {}",
                                val
                            ))
                        })?;
                        config.dnsv6_port = Some(parsed);
                    }
                }
                // Modesets matching upstream GoodbyeDPI implementation
                "-1" => {
                    config.block_passive_dpi = true;
                    config.replace_host = true;
                    config.remove_space = true;
                    config.http_frag_size = Some(2);
                    config.https_frag_size = Some(2);
                }
                "-5" => {
                    config.http_frag_size = Some(2);
                    config.https_frag_size = Some(2);
                    config.auto_ttl = Some("1-4-10".to_string());
                    config.set_ttl = None;
                    config.native_frag = true;
                    config.reverse_frag = true;
                    config.max_payload = Some(1200);
                    config.wrong_seq = false;
                    config.wrong_chksum = false;
                    config.block_quic = false;
                }
                "-6" => {
                    config.http_frag_size = Some(2);
                    config.https_frag_size = Some(2);
                    config.wrong_seq = true;
                    config.native_frag = true;
                    config.reverse_frag = true;
                    config.max_payload = Some(1200);
                }
                "-7" => {
                    config.http_frag_size = Some(2);
                    config.https_frag_size = Some(2);
                    config.wrong_chksum = true;
                    config.native_frag = true;
                    config.reverse_frag = true;
                    config.max_payload = Some(1200);
                }
                "-8" => {
                    config.http_frag_size = Some(2);
                    config.https_frag_size = Some(2);
                    config.wrong_seq = true;
                    config.wrong_chksum = true;
                    config.native_frag = true;
                    config.reverse_frag = true;
                    config.max_payload = Some(1200);
                }
                "-9" => {
                    config.http_frag_size = Some(2);
                    config.https_frag_size = Some(2);
                    config.wrong_seq = true;
                    config.wrong_chksum = true;
                    config.native_frag = true;
                    config.reverse_frag = true;
                    config.max_payload = Some(1200);
                    config.block_quic = true;
                }
                other => {
                    // Prevent rogue unparsed TTL and DNS flags in custom args
                    if other != "--set-ttl"
                        && other != "--auto-ttl"
                        && other != "--dns-addr"
                        && other != "--dns-port"
                        && other != "--dnsv6-addr"
                        && other != "--dnsv6-port"
                    {
                        config.custom_args.push(other.to_string());
                    }
                }
            }
        }

        Ok(config)
    }

    /// Converts typed configuration to safe command line arguments.
    pub fn to_command_args(&self) -> Vec<OsString> {
        self.to_command_args_filtered(true)
    }

    /// Converts typed configuration to safe command line arguments with optional DNS redirect inclusion.
    pub fn to_command_args_filtered(&self, include_dns_redirect: bool) -> Vec<OsString> {
        let mut args = Vec::new();

        if self.block_passive_dpi {
            args.push(OsString::from("-p"));
        }
        if self.block_quic {
            args.push(OsString::from("-q"));
        }
        if self.replace_host {
            args.push(OsString::from("-r"));
        }
        if self.remove_space {
            args.push(OsString::from("-s"));
        }
        if self.mix_host_case {
            args.push(OsString::from("-m"));
        }
        if let Some(size) = self.http_frag_size {
            args.push(OsString::from("-f"));
            args.push(OsString::from(size.to_string()));
        }
        if let Some(size) = self.https_frag_size {
            args.push(OsString::from("-e"));
            args.push(OsString::from(size.to_string()));
        }
        if self.native_frag {
            args.push(OsString::from("--native-frag"));
        }
        if self.reverse_frag {
            args.push(OsString::from("--reverse-frag"));
        }
        if self.wrong_seq {
            args.push(OsString::from("--wrong-seq"));
        }
        if self.wrong_chksum {
            args.push(OsString::from("--wrong-chksum"));
        }
        // Mutually exclusive TTL handling: NEVER output both
        if let Some(ttl) = self.set_ttl {
            args.push(OsString::from("--set-ttl"));
            args.push(OsString::from(ttl.to_string()));
        } else if let Some(ref auto) = self.auto_ttl {
            args.push(OsString::from("--auto-ttl"));
            args.push(OsString::from(auto));
        }
        if let Some(min) = self.min_ttl {
            args.push(OsString::from("--min-ttl"));
            args.push(OsString::from(min.to_string()));
        }
        if let Some(payload) = self.max_payload {
            args.push(OsString::from("--max-payload"));
            args.push(OsString::from(payload.to_string()));
        }
        if include_dns_redirect {
            if let Some(ref addr) = self.dns_addr {
                args.push(OsString::from("--dns-addr"));
                args.push(OsString::from(addr));
            }
            if let Some(port) = self.dns_port {
                args.push(OsString::from("--dns-port"));
                args.push(OsString::from(port.to_string()));
            }
            if let Some(ref addr) = self.dnsv6_addr {
                args.push(OsString::from("--dnsv6-addr"));
                args.push(OsString::from(addr));
            }
            if let Some(port) = self.dnsv6_port {
                args.push(OsString::from("--dnsv6-port"));
                args.push(OsString::from(port.to_string()));
            }
        }
        for custom in &self.custom_args {
            args.push(OsString::from(custom));
        }

        args
    }
}

/// Process adapter controlling GoodbyeDPI worker lifecycle and driver availability.
pub struct GoodbyeDpiAdapter {
    supervisor: ProcessSupervisor,
}

impl GoodbyeDpiAdapter {
    pub fn new(supervisor: ProcessSupervisor) -> Self {
        Self { supervisor }
    }

    /// Spawns the GoodbyeDPI worker with the specified profile definition and optional DNS redirect suppression.
    pub async fn start(
        &mut self,
        profile: &ProfileDefinition,
        custom_base: Option<&Path>,
        suppress_dns_redirect: bool,
    ) -> Result<(), GoodbyeAdapterError> {
        let exe_path = EngineBinaryResolver::resolve(EngineId::GoodbyeDpi, custom_base)
            .map_err(|e| GoodbyeAdapterError::BinaryNotFound(e.to_string()))?;

        // Check if WinDivert.dll exists in working dir or beside exe
        if let Some(parent) = exe_path.parent() {
            let dll_path = parent.join("WinDivert.dll");
            if !dll_path.is_file() {
                let third_party_dist = parent.parent().and_then(|p| p.parent()).map(|root| {
                    root.join("third_party")
                        .join("windivert")
                        .join("WinDivert.dll")
                });
                let is_third_party_ok = third_party_dist.map(|p| p.is_file()).unwrap_or(false);
                let third_party_dll = PathBuf::from("third_party/windivert/x64/WinDivert.dll");
                if !is_third_party_ok && !third_party_dll.is_file() {
                    return Err(GoodbyeAdapterError::DriverDependencyMissing(
                        "WinDivert.dll is missing from engine directory".to_string(),
                    ));
                }
            }
        }

        let typed_config = GoodbyeConfig::from_profile(profile)?;
        let args = typed_config.to_command_args_filtered(!suppress_dns_redirect);

        let working_dir = exe_path.parent().map(|p| p.to_path_buf());
        let mut sup_config = SupervisorConfig::new("GoodbyeDPI-Worker", exe_path, args);
        sup_config.working_directory = working_dir;

        self.supervisor = ProcessSupervisor::new(sup_config);
        self.supervisor.spawn().await?;
        info!(
            profile = %profile.id,
            suppress_dns_redirect,
            "GoodbyeDPI worker process started"
        );
        Ok(())
    }

    /// Stops the GoodbyeDPI worker gracefully and allows WinDivert driver to unload naturally upon handle release.
    pub async fn stop(&mut self) -> Result<(), GoodbyeAdapterError> {
        self.supervisor.graceful_stop().await?;
        Ok(())
    }

    /// Executes bounded availability and lifecycle health check.
    pub async fn health_check(&self) -> HealthCheckResult {
        let status = self.supervisor.status().await;
        if status.state != SupervisorState::Running {
            return HealthCheckResult::unhealthy(format!(
                "ZonDPI motoru çalışmıyor (durum: {}, son hata: {:?})",
                status.state, status.last_error
            ));
        }

        let recent_logs = self.supervisor.get_recent_logs(Some(50)).await;
        let mut filter_activated = false;
        for log in &recent_logs {
            let lower = log.line.to_lowercase();
            if lower.contains("failed to open windivert")
                || lower.contains("access is denied")
                || lower.contains("error opening windivert device")
            {
                return HealthCheckResult::unhealthy(format!(
                    "Paket filtre sürücüsü hatası: {}",
                    log.line
                ));
            }
            if lower.contains("filter activated") || lower.contains("goodbyedpi is now running") {
                filter_activated = true;
            }
        }

        // Only enforce filter_activated check if process has had time to initialize (>1s)
        if !filter_activated && status.uptime_seconds.unwrap_or(0) >= 2 {
            return HealthCheckResult::unhealthy(
                "ZonDPI motoru başlatıldı; paket filtresinin etkinleşmesi bekleniyor.".to_string(),
            );
        }

        HealthCheckResult::healthy(
            Duration::from_millis(5),
            "ZonDPI paket filtresi etkin ve çalışıyor.",
        )
    }

    pub fn supervisor(&self) -> &ProcessSupervisor {
        &self.supervisor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goodbye_config_from_profile_modeset_9() {
        let profile = ProfileDefinition {
            id: "turktelekom".to_string(),
            name: "Türk Telekom".to_string(),
            description: "Default".to_string(),
            target_engine: "goodbye".to_string(),
            arguments: vec!["-9".to_string()],
        };

        let config = GoodbyeConfig::from_profile(&profile).expect("parse profile");
        assert!(config.block_quic);
        assert!(config.wrong_seq);
        assert!(config.wrong_chksum);
        assert!(config.native_frag, "-9 must enable native_frag");
        assert!(config.reverse_frag, "-9 must enable reverse_frag");
        assert_eq!(config.http_frag_size, Some(2));
        assert_eq!(config.https_frag_size, Some(2));
        assert_eq!(config.max_payload, Some(1200));

        let args = config.to_command_args();
        assert!(args.contains(&OsString::from("-q")));
        assert!(args.contains(&OsString::from("--wrong-seq")));
        assert!(args.contains(&OsString::from("--wrong-chksum")));
        assert!(args.contains(&OsString::from("--native-frag")));
        assert!(args.contains(&OsString::from("--reverse-frag")));
        assert!(!args.contains(&OsString::from("--auto-ttl")));
        assert!(!args.contains(&OsString::from("--set-ttl")));
    }

    #[test]
    fn test_goodbye_config_from_profile_modeset_5() {
        let profile = ProfileDefinition {
            id: "test-5".to_string(),
            name: "Test 5".to_string(),
            description: "Preset 5".to_string(),
            target_engine: "goodbye".to_string(),
            arguments: vec!["-5".to_string()],
        };

        let config = GoodbyeConfig::from_profile(&profile).expect("parse profile");
        assert!(config.native_frag, "-5 must enable native_frag");
        assert!(config.reverse_frag, "-5 must enable reverse_frag");
        assert_eq!(config.auto_ttl, Some("1-4-10".to_string()));
        assert_eq!(config.set_ttl, None);

        let args = config.to_command_args();
        assert!(args.contains(&OsString::from("--auto-ttl")));
        assert!(args.contains(&OsString::from("1-4-10")));
        assert!(args.contains(&OsString::from("--native-frag")));
        assert!(args.contains(&OsString::from("--reverse-frag")));
        assert!(!args.contains(&OsString::from("--set-ttl")));
    }

    #[test]
    fn test_mutually_exclusive_ttl_no_contradictory_flags() {
        // Specifying both -5 and --set-ttl must NOT output both --auto-ttl and --set-ttl
        let profile = ProfileDefinition {
            id: "contradictory-ttl".to_string(),
            name: "Contradictory TTL".to_string(),
            description: "Testing mutual exclusion".to_string(),
            target_engine: "goodbye".to_string(),
            arguments: vec!["-5".to_string(), "--set-ttl".to_string(), "5".to_string()],
        };

        let config = GoodbyeConfig::from_profile(&profile).expect("parse profile");
        assert_eq!(config.set_ttl, Some(5));
        assert_eq!(config.auto_ttl, None, "--set-ttl must nullify auto_ttl");

        let args = config.to_command_args();
        assert!(args.contains(&OsString::from("--set-ttl")));
        assert!(args.contains(&OsString::from("5")));
        assert!(
            !args.contains(&OsString::from("--auto-ttl")),
            "Command args must NEVER contain both --set-ttl and --auto-ttl"
        );
    }

    #[test]
    fn test_dns_redirect_optional() {
        // Default profile without DNS args must NOT have DNS redirection
        let profile_clean = ProfileDefinition {
            id: "clean".to_string(),
            name: "Clean".to_string(),
            description: "No DNS redirect".to_string(),
            target_engine: "goodbye".to_string(),
            arguments: vec!["-9".to_string()],
        };
        let config_clean = GoodbyeConfig::from_profile(&profile_clean).expect("parse");
        let args_clean = config_clean.to_command_args();
        assert!(!args_clean.contains(&OsString::from("--dns-addr")));
        assert!(!args_clean.contains(&OsString::from("--dns-port")));

        // Profile with explicit DNS args has DNS redirection
        let profile_dns = ProfileDefinition {
            id: "with-dns".to_string(),
            name: "With DNS".to_string(),
            description: "Has DNS redirect".to_string(),
            target_engine: "goodbye".to_string(),
            arguments: vec![
                "-5".to_string(),
                "--dns-addr".to_string(),
                "1.1.1.1".to_string(),
                "--dns-port".to_string(),
                "53".to_string(),
                "--dnsv6-addr".to_string(),
                "2606:4700:4700::1111".to_string(),
                "--dnsv6-port".to_string(),
                "53".to_string(),
            ],
        };
        let config_dns = GoodbyeConfig::from_profile(&profile_dns).expect("parse");
        let args_dns = config_dns.to_command_args();
        assert!(args_dns.contains(&OsString::from("--dns-addr")));
        assert!(args_dns.contains(&OsString::from("1.1.1.1")));
        assert!(args_dns.contains(&OsString::from("--dns-port")));
        assert!(args_dns.contains(&OsString::from("53")));
        assert!(args_dns.contains(&OsString::from("--dnsv6-addr")));
        assert!(args_dns.contains(&OsString::from("2606:4700:4700::1111")));
        assert!(args_dns.contains(&OsString::from("--dnsv6-port")));
        assert!(args_dns.contains(&OsString::from("53")));
    }

    #[test]
    fn test_deterministic_argument_ordering() {
        let profile = ProfileDefinition {
            id: "order-test".to_string(),
            name: "Order Test".to_string(),
            description: "Order verification".to_string(),
            target_engine: "goodbye".to_string(),
            arguments: vec!["-5".to_string()],
        };
        let config = GoodbyeConfig::from_profile(&profile).expect("parse");
        let args1 = config.to_command_args();
        let args2 = config.to_command_args();
        assert_eq!(
            args1, args2,
            "to_command_args must be strictly deterministic"
        );
    }

    #[test]
    fn test_turkey_default_validated_modeset_5_semantics_and_clean_dns() {
        // Load the actual JSON profile from disk to test the real production configuration
        let profile_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("profiles")
            .join("turkey")
            .join("turktelekom.json");

        let content = std::fs::read_to_string(&profile_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {}", profile_path.display(), e));
        let profile: ProfileDefinition = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to deserialize profile: {}", e));

        assert_eq!(profile.id, "turkey-default");

        let config = GoodbyeConfig::from_profile(&profile).expect("parse turkey-default profile");

        // 1. turkey-default maps to -5 semantics
        assert_eq!(config.http_frag_size, Some(2), "Fragment HTTP must be 2");
        assert_eq!(config.https_frag_size, Some(2), "Fragment HTTPS must be 2");
        assert_eq!(config.max_payload, Some(1200), "Max payload must be 1200");

        // 2. native fragmentation remains enabled
        assert!(
            config.native_frag,
            "Native fragmentation must remain enabled"
        );

        // 3. reverse fragmentation remains enabled
        assert!(
            config.reverse_frag,
            "Reverse fragmentation must remain enabled"
        );

        // 4. auto TTL remains enabled (1-4-10)
        assert_eq!(
            config.auto_ttl.as_deref(),
            Some("1-4-10"),
            "Auto TTL must be 1-4-10"
        );

        // 5. static set-ttl is NOT present
        assert_eq!(config.set_ttl, None, "Static set-ttl must NOT be present");

        // 6. QUIC block is NOT added
        assert!(!config.block_quic, "QUIC block must NOT be added");

        // 7. -9 is NOT the turkey-default mode (no wrong-seq or wrong-chksum)
        assert!(
            !config.wrong_seq,
            "-9 wrong-seq must NOT be present in turkey-default"
        );
        assert!(
            !config.wrong_chksum,
            "-9 wrong-chksum must NOT be present in turkey-default"
        );

        // 8. DNS IPv4 redirect is 1.1.1.1:53
        assert_eq!(config.dns_addr.as_deref(), Some("1.1.1.1"));
        assert_eq!(config.dns_port, Some(53));

        // 9. DNS IPv6 redirect is 2606:4700:4700::1111:53
        assert_eq!(config.dnsv6_addr.as_deref(), Some("2606:4700:4700::1111"));
        assert_eq!(config.dnsv6_port, Some(53));

        // 10. Yandex 77.88.8.8 does NOT appear
        assert_ne!(config.dns_addr.as_deref(), Some("77.88.8.8"));
        assert_ne!(config.dnsv6_addr.as_deref(), Some("2a02:6b8::feed:0ff"));

        // Test command-line arguments generated for the worker process
        let args = config.to_command_args();
        assert!(args.contains(&OsString::from("-f")));
        assert!(args.contains(&OsString::from("2")));
        assert!(args.contains(&OsString::from("-e")));
        assert!(args.contains(&OsString::from("--native-frag")));
        assert!(args.contains(&OsString::from("--reverse-frag")));
        assert!(args.contains(&OsString::from("--auto-ttl")));
        assert!(args.contains(&OsString::from("1-4-10")));
        assert!(args.contains(&OsString::from("--max-payload")));
        assert!(args.contains(&OsString::from("1200")));

        assert!(args.contains(&OsString::from("--dns-addr")));
        assert!(args.contains(&OsString::from("1.1.1.1")));
        assert!(args.contains(&OsString::from("--dns-port")));
        assert!(args.contains(&OsString::from("53")));
        assert!(args.contains(&OsString::from("--dnsv6-addr")));
        assert!(args.contains(&OsString::from("2606:4700:4700::1111")));
        assert!(args.contains(&OsString::from("--dnsv6-port")));

        // Strict negative checks
        assert!(!args.contains(&OsString::from("-q")), "Must NOT contain -q");
        assert!(!args.contains(&OsString::from("-9")), "Must NOT contain -9");
        assert!(
            !args.contains(&OsString::from("--wrong-seq")),
            "Must NOT contain --wrong-seq"
        );
        assert!(
            !args.contains(&OsString::from("--wrong-chksum")),
            "Must NOT contain --wrong-chksum"
        );
        assert!(
            !args.contains(&OsString::from("--set-ttl")),
            "Must NOT contain --set-ttl"
        );
        assert!(
            !args.contains(&OsString::from("77.88.8.8")),
            "Must NOT contain Yandex 77.88.8.8"
        );
        assert!(
            !args.contains(&OsString::from("1253")),
            "Must NOT contain Yandex port 1253"
        );
    }

    #[test]
    fn test_kaspersky_detected_suppresses_dns_redirect_args() {
        let profile = ProfileDefinition {
            id: "turkey-default".to_string(),
            name: "Türk Telekom".to_string(),
            description: "Default".to_string(),
            target_engine: "goodbye".to_string(),
            arguments: vec![
                "-5".to_string(),
                "--dns-addr".to_string(),
                "1.1.1.1".to_string(),
                "--dns-port".to_string(),
                "53".to_string(),
                "--dnsv6-addr".to_string(),
                "2606:4700:4700::1111".to_string(),
                "--dnsv6-port".to_string(),
                "53".to_string(),
            ],
        };

        let config = GoodbyeConfig::from_profile(&profile).expect("parse");
        // When Kaspersky is detected: include_dns_redirect = false
        let filtered_args = config.to_command_args_filtered(false);

        // Verify all DNS redirect arguments are strictly suppressed
        assert!(!filtered_args.contains(&OsString::from("--dns-addr")));
        assert!(!filtered_args.contains(&OsString::from("1.1.1.1")));
        assert!(!filtered_args.contains(&OsString::from("--dns-port")));
        assert!(!filtered_args.contains(&OsString::from("--dnsv6-addr")));
        assert!(!filtered_args.contains(&OsString::from("2606:4700:4700::1111")));
        assert!(!filtered_args.contains(&OsString::from("--dnsv6-port")));
    }

    #[test]
    fn test_kaspersky_detected_preserves_modeset_5_packet_args() {
        let profile = ProfileDefinition {
            id: "turkey-default".to_string(),
            name: "Türk Telekom".to_string(),
            description: "Default".to_string(),
            target_engine: "goodbye".to_string(),
            arguments: vec![
                "-5".to_string(),
                "--dns-addr".to_string(),
                "1.1.1.1".to_string(),
                "--dns-port".to_string(),
                "53".to_string(),
            ],
        };

        let config = GoodbyeConfig::from_profile(&profile).expect("parse");
        let filtered_args = config.to_command_args_filtered(false);

        // Validated -5 packet evasion semantics must be completely preserved
        assert!(filtered_args.contains(&OsString::from("-f")));
        assert!(filtered_args.contains(&OsString::from("2")));
        assert!(filtered_args.contains(&OsString::from("-e")));
        assert!(filtered_args.contains(&OsString::from("--native-frag")));
        assert!(filtered_args.contains(&OsString::from("--reverse-frag")));
        assert!(filtered_args.contains(&OsString::from("--auto-ttl")));
        assert!(filtered_args.contains(&OsString::from("1-4-10")));
        assert!(filtered_args.contains(&OsString::from("--max-payload")));
        assert!(filtered_args.contains(&OsString::from("1200")));

        // Negative check: no -9 flags
        assert!(!filtered_args.contains(&OsString::from("--wrong-seq")));
        assert!(!filtered_args.contains(&OsString::from("--wrong-chksum")));
        assert!(!filtered_args.contains(&OsString::from("-q")));
    }
}
