//! ByeDPI (ciadpi) engine adapter, typed argument builder, port preflight check, and SOCKS5 health verification.

use std::ffi::OsString;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::info;

use crate::supervisor::{ProcessSupervisor, SupervisorConfig, SupervisorError, SupervisorState};
use zondpi_packet_engine::{EngineBinaryResolver, EngineId, HealthCheckResult, ProfileDefinition};

#[derive(Error, Debug)]
pub enum ByeDpiAdapterError {
    #[error("Profile target engine mismatch: expected 'byedpi', got '{0}'")]
    EngineMismatch(String),
    #[error("Failed to resolve ByeDPI (ciadpi) worker binary: {0}")]
    BinaryNotFound(String),
    #[error("SOCKS5 port {0} is already in use by another application")]
    PortAlreadyInUse(u16),
    #[error("Process supervisor error: {0}")]
    Supervisor(#[from] SupervisorError),
    #[error("Invalid argument in profile: {0}")]
    InvalidArgument(String),
    #[error("Insecure configuration: binding to 0.0.0.0 is prohibited (must use 127.0.0.1)")]
    InsecureBinding,
}

/// Strongly-typed configuration model for ByeDPI parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByeDpiConfig {
    pub listen_ip: String,
    pub port: u16,
    pub max_conn: Option<u32>,
    pub split_pos: Option<String>,
    pub disorder_pos: Option<String>,
    pub fake_pos: Option<String>,
    pub fake_ttl: Option<u8>,
    pub auto_params: Option<String>,
    pub auto_mode: Option<String>,
    pub custom_args: Vec<String>,
}

impl Default for ByeDpiConfig {
    fn default() -> Self {
        Self {
            listen_ip: "127.0.0.1".to_string(),
            port: 1080,
            max_conn: None,
            split_pos: None,
            disorder_pos: None,
            fake_pos: None,
            fake_ttl: None,
            auto_params: None,
            auto_mode: None,
            custom_args: Vec::new(),
        }
    }
}

impl ByeDpiConfig {
    /// Builds typed ByeDpiConfig from a validated JSON profile definition.
    pub fn from_profile(profile: &ProfileDefinition) -> Result<Self, ByeDpiAdapterError> {
        let target = profile.target_engine.to_lowercase();
        if target != "byedpi" && target != "ciadpi" {
            return Err(ByeDpiAdapterError::EngineMismatch(
                profile.target_engine.clone(),
            ));
        }

        let mut config = Self::default();
        let mut iter = profile.arguments.iter().peekable();

        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-i" | "--ip" => {
                    if let Some(val) = iter.next() {
                        if val == "0.0.0.0" {
                            return Err(ByeDpiAdapterError::InsecureBinding);
                        }
                        config.listen_ip = val.clone();
                    }
                }
                "-p" | "--port" => {
                    if let Some(val) = iter.next() {
                        let parsed: u16 = val.parse().map_err(|_| {
                            ByeDpiAdapterError::InvalidArgument(format!(
                                "Invalid port number: {}",
                                val
                            ))
                        })?;
                        config.port = parsed;
                    }
                }
                "-c" | "--max-conn" => {
                    if let Some(val) = iter.next() {
                        let parsed: u32 = val.parse().map_err(|_| {
                            ByeDpiAdapterError::InvalidArgument(format!(
                                "Invalid max-conn value: {}",
                                val
                            ))
                        })?;
                        config.max_conn = Some(parsed);
                    }
                }
                "-s" | "--split" => {
                    if let Some(val) = iter.next() {
                        config.split_pos = Some(val.clone());
                    }
                }
                "-d" | "--disorder" => {
                    if let Some(val) = iter.next() {
                        config.disorder_pos = Some(val.clone());
                    }
                }
                "-f" | "--fake" => {
                    if let Some(val) = iter.next() {
                        config.fake_pos = Some(val.clone());
                    }
                }
                "-t" | "--ttl" => {
                    if let Some(val) = iter.next() {
                        let parsed: u8 = val.parse().map_err(|_| {
                            ByeDpiAdapterError::InvalidArgument(format!(
                                "Invalid TTL value: {}",
                                val
                            ))
                        })?;
                        config.fake_ttl = Some(parsed);
                    }
                }
                "-A" | "--auto" => {
                    if let Some(val) = iter.next() {
                        config.auto_params = Some(val.clone());
                    }
                }
                "-L" | "--auto-mode" => {
                    if let Some(val) = iter.next() {
                        config.auto_mode = Some(val.clone());
                    }
                }
                other => {
                    config.custom_args.push(other.to_string());
                }
            }
        }

        Ok(config)
    }

    /// Converts typed configuration to safe command line arguments.
    pub fn to_command_args(&self) -> Vec<OsString> {
        let mut args = vec![
            OsString::from("-i"),
            OsString::from(&self.listen_ip),
            OsString::from("-p"),
            OsString::from(self.port.to_string()),
        ];

        if let Some(mc) = self.max_conn {
            args.push(OsString::from("-c"));
            args.push(OsString::from(mc.to_string()));
        }
        if let Some(ref s) = self.split_pos {
            args.push(OsString::from("-s"));
            args.push(OsString::from(s));
        }
        if let Some(ref d) = self.disorder_pos {
            args.push(OsString::from("-d"));
            args.push(OsString::from(d));
        }
        if let Some(ref f) = self.fake_pos {
            args.push(OsString::from("-f"));
            args.push(OsString::from(f));
        }
        if let Some(t) = self.fake_ttl {
            args.push(OsString::from("-t"));
            args.push(OsString::from(t.to_string()));
        }
        if let Some(ref a) = self.auto_params {
            args.push(OsString::from("-A"));
            args.push(OsString::from(a));
        }
        if let Some(ref l) = self.auto_mode {
            args.push(OsString::from("-L"));
            args.push(OsString::from(l));
        }
        for custom in &self.custom_args {
            args.push(OsString::from(custom));
        }

        args
    }
}

/// Process adapter controlling ByeDPI (ciadpi) SOCKS5 worker lifecycle and port availability.
pub struct ByeDpiAdapter {
    supervisor: ProcessSupervisor,
    config: Option<ByeDpiConfig>,
}

impl ByeDpiAdapter {
    pub fn new(supervisor: ProcessSupervisor) -> Self {
        Self {
            supervisor,
            config: None,
        }
    }

    /// Checks whether the target TCP port is available on localhost prior to spawn.
    pub fn preflight_check_port(ip: &str, port: u16) -> Result<(), ByeDpiAdapterError> {
        let addr = format!("{}:{}", ip, port);
        match TcpListener::bind(&addr) {
            Ok(listener) => {
                drop(listener);
                Ok(())
            }
            Err(_) => Err(ByeDpiAdapterError::PortAlreadyInUse(port)),
        }
    }

    /// Spawns the ByeDPI worker with the specified profile definition.
    pub async fn start(
        &mut self,
        profile: &ProfileDefinition,
        custom_base: Option<&Path>,
    ) -> Result<(), ByeDpiAdapterError> {
        let exe_path = EngineBinaryResolver::resolve(EngineId::ByeDpi, custom_base)
            .map_err(|e| ByeDpiAdapterError::BinaryNotFound(e.to_string()))?;

        let typed_config = ByeDpiConfig::from_profile(profile)?;
        Self::preflight_check_port(&typed_config.listen_ip, typed_config.port)?;

        let args = typed_config.to_command_args();
        let working_dir = exe_path.parent().map(|p| p.to_path_buf());
        let mut sup_config = SupervisorConfig::new("ByeDPI-Worker", exe_path, args);
        sup_config.working_directory = working_dir;

        self.config = Some(typed_config);
        self.supervisor = ProcessSupervisor::new(sup_config);
        self.supervisor.spawn().await?;
        info!(profile = %profile.id, "ByeDPI worker process started");
        Ok(())
    }

    /// Stops the ByeDPI worker gracefully.
    pub async fn stop(&mut self) -> Result<(), ByeDpiAdapterError> {
        self.supervisor.graceful_stop().await?;
        Ok(())
    }

    /// Executes live lifecycle and SOCKS5 protocol handshake health check.
    pub async fn health_check(&self) -> HealthCheckResult {
        let status = self.supervisor.status().await;
        if status.state != SupervisorState::Running {
            return HealthCheckResult::unhealthy(format!(
                "ZonDPI uyumluluk servisi çalışmıyor (durum: {}, son hata: {:?})",
                status.state, status.last_error
            ));
        }

        let (ip, port) = if let Some(ref cfg) = self.config {
            (cfg.listen_ip.as_str(), cfg.port)
        } else {
            ("127.0.0.1", 1080)
        };

        // Give process a small startup window if just started
        let start_time = Instant::now();
        let target_addr: SocketAddr = match format!("{}:{}", ip, port).parse() {
            Ok(a) => a,
            Err(e) => {
                return HealthCheckResult::unhealthy(format!("Invalid socket address: {}", e))
            }
        };

        // Perform TCP connect and SOCKS5 handshake probe
        match TcpStream::connect_timeout(&target_addr, Duration::from_millis(600)) {
            Ok(mut stream) => {
                use std::io::{Read, Write};
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));

                // SOCKS5 handshake: VER 5, 1 METHOD, NO AUTH (0x00)
                let greeting = [0x05, 0x01, 0x00];
                if let Err(e) = stream.write_all(&greeting) {
                    return HealthCheckResult::unhealthy(format!(
                        "Failed to write SOCKS5 greeting probe: {}",
                        e
                    ));
                }

                let mut response = [0u8; 2];
                if let Err(e) = stream.read_exact(&mut response) {
                    return HealthCheckResult::unhealthy(format!(
                        "Failed to read SOCKS5 server greeting response: {}",
                        e
                    ));
                }

                // SOCKS5 response: VER 5, METHOD 0 (No authentication required)
                if response[0] == 0x05 && response[1] == 0x00 {
                    let elapsed = start_time.elapsed();
                    HealthCheckResult::healthy(
                        elapsed,
                        format!("ZonDPI uyumluluk yöntemi etkin ({}:{}).", ip, port),
                    )
                } else {
                    HealthCheckResult::unhealthy(format!(
                        "Unexpected SOCKS5 handshake response: [{:#04x}, {:#04x}]",
                        response[0], response[1]
                    ))
                }
            }
            Err(e) => HealthCheckResult::unhealthy(format!(
                "ZonDPI uyumluluk bağlantı noktasına bağlanılamadı ({}:{}): {}",
                ip, port, e
            )),
        }
    }

    pub fn supervisor(&self) -> &ProcessSupervisor {
        &self.supervisor
    }

    pub fn config(&self) -> Option<&ByeDpiConfig> {
        self.config.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_byedpi_config_from_profile_kaspersky_mode() {
        let profile = ProfileDefinition {
            id: "byedpi_kaspersky_mode".to_string(),
            name: "ByeDPI Kaspersky Mode".to_string(),
            description: "Safe userspace mode".to_string(),
            target_engine: "byedpi".to_string(),
            arguments: vec![
                "-i".to_string(),
                "127.0.0.1".to_string(),
                "-p".to_string(),
                "1080".to_string(),
                "-s".to_string(),
                "1".to_string(),
                "-d".to_string(),
                "1".to_string(),
            ],
        };

        let config = ByeDpiConfig::from_profile(&profile).expect("parse profile");
        assert_eq!(config.listen_ip, "127.0.0.1");
        assert_eq!(config.port, 1080);
        assert_eq!(config.split_pos.as_deref(), Some("1"));
        assert_eq!(config.disorder_pos.as_deref(), Some("1"));

        let args = config.to_command_args();
        assert!(args.contains(&OsString::from("-i")));
        assert!(args.contains(&OsString::from("127.0.0.1")));
        assert!(args.contains(&OsString::from("1080")));
    }

    #[test]
    fn test_byedpi_rejects_insecure_0000_binding() {
        let profile = ProfileDefinition {
            id: "bad_bind".to_string(),
            name: "Bad".to_string(),
            description: "Bad".to_string(),
            target_engine: "byedpi".to_string(),
            arguments: vec!["-i".to_string(), "0.0.0.0".to_string()],
        };

        let config_res = ByeDpiConfig::from_profile(&profile);
        assert!(config_res.is_err());
        assert!(matches!(
            config_res.unwrap_err(),
            ByeDpiAdapterError::InsecureBinding
        ));
    }
}
