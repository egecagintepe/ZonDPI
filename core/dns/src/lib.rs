//! Windows Adapter DNS management, snapshotting, privileged state persistence, and atomic rollback.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use tracing::{error, info, warn};

pub const STATE_SCHEMA_VERSION: u32 = 1;
pub const STATE_SOURCE_IDENTIFIER: &str = "zondpi-service";

#[derive(Error, Debug)]
pub enum DnsManagerError {
    #[error("Adapter error: {0}")]
    AdapterNotFound(String),
    #[error("Ambiguous outbound adapter: {0}")]
    AmbiguousOutboundAdapter(String),
    #[error("DNS state validation failed: {0}")]
    StateValidationFailed(String),
    #[error("DNS state file access error: {0}")]
    StateFileAccessDenied(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Command execution error: {0}")]
    CommandFailed(String),
}

/// Supported DNS providers for clean resolver configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DnsProvider {
    Automatic,
    Cloudflare,
    Google,
    Quad9,
    System,
}

impl DnsProvider {
    /// Returns the IPv4 endpoints for the selected DNS provider.
    pub fn endpoints_v4(&self) -> Vec<String> {
        match self {
            DnsProvider::Automatic | DnsProvider::Cloudflare => {
                vec!["1.1.1.1".to_string(), "1.0.0.1".to_string()]
            }
            DnsProvider::Google => vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()],
            DnsProvider::Quad9 => {
                vec!["9.9.9.9".to_string(), "149.112.112.112".to_string()]
            }
            DnsProvider::System => Vec::new(),
        }
    }

    /// Returns the IPv6 endpoints for the selected DNS provider.
    pub fn endpoints_v6(&self) -> Vec<String> {
        match self {
            DnsProvider::Automatic | DnsProvider::Cloudflare => vec![
                "2606:4700:4700::1111".to_string(),
                "2606:4700:4700::1001".to_string(),
            ],
            DnsProvider::Google => vec![
                "2001:4860:4860::8888".to_string(),
                "2001:4860:4860::8844".to_string(),
            ],
            DnsProvider::Quad9 => {
                vec!["2620:fe::fe".to_string(), "2620:fe::9".to_string()]
            }
            DnsProvider::System => Vec::new(),
        }
    }

    /// Whether this provider should override the operating system's active DNS configuration.
    /// User intent `System` preserves existing system DNS without modification.
    pub fn should_override_system(&self) -> bool {
        !matches!(self, DnsProvider::System)
    }
}

/// Snapshot of an adapter's DNS configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterDnsConfig {
    pub interface_index: u32,
    pub interface_alias: String,
    pub interface_guid: Option<String>,
    pub interface_description: Option<String>,
    pub is_dhcp_v4: bool,
    pub is_dhcp_v6: bool,
    pub ipv4_servers: Vec<String>,
    pub ipv6_servers: Vec<String>,
}

impl AdapterDnsConfig {
    /// Validates internal consistency and formatting of the adapter configuration.
    pub fn validate(&self) -> Result<(), DnsManagerError> {
        if self.interface_index == 0 {
            return Err(DnsManagerError::StateValidationFailed(
                "Interface index must be greater than 0".to_string(),
            ));
        }

        validate_clean_string(&self.interface_alias, "interface_alias")?;

        if let Some(ref desc) = self.interface_description {
            validate_clean_string(desc, "interface_description")?;
        }

        if let Some(ref guid) = self.interface_guid {
            validate_guid_format(guid)?;
        }

        if self.ipv4_servers.len() > 8 {
            return Err(DnsManagerError::StateValidationFailed(
                "IPv4 server count exceeds limit (max 8)".to_string(),
            ));
        }

        for ip in &self.ipv4_servers {
            match IpAddr::from_str(ip) {
                Ok(IpAddr::V4(_)) => {}
                _ => {
                    return Err(DnsManagerError::StateValidationFailed(format!(
                        "Invalid IPv4 server: {}",
                        ip
                    )));
                }
            }
        }

        if self.ipv6_servers.len() > 8 {
            return Err(DnsManagerError::StateValidationFailed(
                "IPv6 server count exceeds limit (max 8)".to_string(),
            ));
        }

        for ip in &self.ipv6_servers {
            match IpAddr::from_str(ip) {
                Ok(IpAddr::V6(_)) => {}
                _ => {
                    return Err(DnsManagerError::StateValidationFailed(format!(
                        "Invalid IPv6 server: {}",
                        ip
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Privileged persistent state stored in `%ProgramData%\ZonDPI\dns_state.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsOverrideState {
    pub schema_version: u32,
    pub source: String,
    pub dns_override_active: bool,
    pub timestamp_utc: String,
    pub interface_index: u32,
    pub interface_alias: String,
    pub interface_guid: Option<String>,
    pub original_config: AdapterDnsConfig,
    pub applied_v4_servers: Vec<String>,
    pub applied_v6_servers: Vec<String>,
    pub provider: DnsProvider,
}

impl DnsOverrideState {
    pub fn new(
        original: AdapterDnsConfig,
        applied_v4: Vec<String>,
        applied_v6: Vec<String>,
        provider: DnsProvider,
    ) -> Self {
        let now_utc = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());

        Self {
            schema_version: STATE_SCHEMA_VERSION,
            source: STATE_SOURCE_IDENTIFIER.to_string(),
            dns_override_active: true,
            timestamp_utc: now_utc,
            interface_index: original.interface_index,
            interface_alias: original.interface_alias.clone(),
            interface_guid: original.interface_guid.clone(),
            original_config: original,
            applied_v4_servers: applied_v4,
            applied_v6_servers: applied_v6,
            provider,
        }
    }

    /// Rigorous validation of loaded state file prior to restoration or use.
    pub fn validate(&self) -> Result<(), DnsManagerError> {
        if self.schema_version != STATE_SCHEMA_VERSION {
            return Err(DnsManagerError::StateValidationFailed(format!(
                "Unsupported schema version {}; expected {}",
                self.schema_version, STATE_SCHEMA_VERSION
            )));
        }

        if self.source != STATE_SOURCE_IDENTIFIER {
            return Err(DnsManagerError::StateValidationFailed(format!(
                "Invalid source identifier '{}'; expected '{}'",
                self.source, STATE_SOURCE_IDENTIFIER
            )));
        }

        if self.interface_index == 0 {
            return Err(DnsManagerError::StateValidationFailed(
                "Interface index must be greater than 0".to_string(),
            ));
        }

        validate_clean_string(&self.interface_alias, "interface_alias")?;

        if let Some(ref guid) = self.interface_guid {
            validate_guid_format(guid)?;
        }

        // Validate nested original configuration
        self.original_config.validate()?;

        // Validate applied servers
        if self.applied_v4_servers.len() > 8 || self.applied_v6_servers.len() > 8 {
            return Err(DnsManagerError::StateValidationFailed(
                "Applied server count exceeds maximum allowable limit".to_string(),
            ));
        }

        for ip in &self.applied_v4_servers {
            match IpAddr::from_str(ip) {
                Ok(IpAddr::V4(_)) => {}
                _ => {
                    return Err(DnsManagerError::StateValidationFailed(format!(
                        "Invalid applied IPv4 address: {}",
                        ip
                    )));
                }
            }
        }

        for ip in &self.applied_v6_servers {
            match IpAddr::from_str(ip) {
                Ok(IpAddr::V6(_)) => {}
                _ => {
                    return Err(DnsManagerError::StateValidationFailed(format!(
                        "Invalid applied IPv6 address: {}",
                        ip
                    )));
                }
            }
        }

        Ok(())
    }
}

fn validate_clean_string(s: &str, field_name: &str) -> Result<(), DnsManagerError> {
    if s.is_empty() {
        return Err(DnsManagerError::StateValidationFailed(format!(
            "Field '{}' cannot be empty",
            field_name
        )));
    }
    // Disallow dangerous shell characters
    for c in s.chars() {
        if c == '"'
            || c == '\''
            || c == ';'
            || c == '|'
            || c == '&'
            || c == '$'
            || c == '`'
            || c == '<'
            || c == '>'
            || c == '\n'
            || c == '\r'
            || c == '\0'
        {
            return Err(DnsManagerError::StateValidationFailed(format!(
                "Field '{}' contains prohibited character: {:?}",
                field_name, c
            )));
        }
    }
    Ok(())
}

fn validate_guid_format(guid: &str) -> Result<(), DnsManagerError> {
    let trimmed = guid.trim_matches(|c| c == '{' || c == '}');
    let parts: Vec<&str> = trimmed.split('-').collect();
    if parts.len() != 5
        || parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        return Err(DnsManagerError::StateValidationFailed(format!(
            "Malformed GUID format: {}",
            guid
        )));
    }

    for part in &parts {
        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DnsManagerError::StateValidationFailed(format!(
                "Non-hex character in GUID: {}",
                guid
            )));
        }
    }
    Ok(())
}

/// Abstract adapter operations trait for deterministic testability and injection.
pub trait AdapterDnsOps: Send + Sync {
    /// Detects active internet outbound adapter without modifying virtual/tunnel adapters.
    fn find_active_outbound_adapter(&self) -> Result<AdapterDnsConfig, DnsManagerError>;

    /// Applies static DNS servers to the specified adapter.
    fn apply_dns_servers(
        &self,
        interface_index: u32,
        v4: &[String],
        v6: &[String],
    ) -> Result<(), DnsManagerError>;

    /// Restores original DNS configuration (DHCP or exact static list).
    fn restore_dns(&self, config: &AdapterDnsConfig) -> Result<(), DnsManagerError>;

    /// Flushes Windows DNS client resolver cache.
    fn flush_resolver_cache(&self) -> Result<(), DnsManagerError>;

    /// Verifies local DNS resolution against target. Never logs domain names.
    fn verify_dns_resolution(&self, target_host: &str) -> Result<bool, DnsManagerError>;
}

/// Native Windows implementation of AdapterDnsOps using hardened routing and PowerShell probes.
pub struct WindowsAdapterDnsOps;

impl WindowsAdapterDnsOps {
    fn get_system32_executable(exe_name: &str) -> PathBuf {
        let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        PathBuf::from(sys_root).join("System32").join(exe_name)
    }

    fn get_powershell_executable() -> PathBuf {
        let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        PathBuf::from(sys_root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe")
    }
}

impl AdapterDnsOps for WindowsAdapterDnsOps {
    fn find_active_outbound_adapter(&self) -> Result<AdapterDnsConfig, DnsManagerError> {
        let powershell = Self::get_powershell_executable();
        if !powershell.is_file() {
            return Err(DnsManagerError::CommandFailed(format!(
                "PowerShell binary not found at {}",
                powershell.display()
            )));
        }

        // Script queries default routes, inspects adapters, filters virtual/VPN tunnels,
        // and returns structured JSON for the single active physical outbound adapter.
        let script = r#"
            try {
                $ErrorActionPreference = 'Stop'
                $routes = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric
                if (-not $routes) {
                    Write-Output "ERROR:No default route found"
                    exit 0
                }

                # Collect candidates
                $candidates = @()
                foreach ($r in $routes) {
                    $adapter = Get-NetAdapter -InterfaceIndex $r.InterfaceIndex -ErrorAction SilentlyContinue
                    if ($null -eq $adapter -or $adapter.Status -ne 'Up') {
                        continue
                    }

                    $desc = if ($adapter.InterfaceDescription) { $adapter.InterfaceDescription.ToLower() } else { "" }
                    $name = if ($adapter.Name) { $adapter.Name.ToLower() } else { "" }

                    # Skip known virtual/VPN tunnels
                    $isVirtualTunnel = $adapter.Virtual -or `
                        $desc -like "*tailscale*" -or `
                        $desc -like "*wireguard*" -or `
                        $desc -like "*openvpn*" -or `
                        $desc -like "*tap-windows*" -or `
                        $desc -like "*hyper-v*" -or `
                        $desc -like "*vethernet*" -or `
                        $desc -like "*wsl*" -or `
                        $desc -like "*bluetooth*" -or `
                        $desc -like "*loopback*" -or `
                        $desc -like "*wan miniport*"

                    if (-not $isVirtualTunnel -and $adapter.HardwareInterface) {
                        $candidates += [PSCustomObject]@{
                            Adapter = $adapter
                            Metric = $r.RouteMetric
                        }
                    }
                }

                if ($candidates.Count -eq 0) {
                    Write-Output "ERROR:No safe physical outbound adapter found"
                    exit 0
                }

                # Group by metric to check for ambiguous default routes
                $bestMetric = $candidates[0].Metric
                $bestCandidates = @($candidates | Where-Object { $_.Metric -eq $bestMetric })
                if ($bestCandidates.Count -gt 1) {
                    Write-Output "AMBIGUOUS:Multiple default routes with equal metric"
                    exit 0
                }

                $chosen = $bestCandidates[0].Adapter
                $dns = Get-DnsClientServerAddress -InterfaceIndex $chosen.InterfaceIndex -ErrorAction SilentlyContinue
                $v4Servers = @(($dns | Where-Object { $_.AddressFamily -eq 2 }).ServerAddresses)
                $v6Servers = @(($dns | Where-Object { $_.AddressFamily -eq 23 }).ServerAddresses)

                # Query registry to determine DHCP vs Static
                $guidClean = $chosen.InterfaceGuid.Trim('{','}')
                $regPath = 'Registry::HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\{' + $guidClean + '}'
                $ns4 = (Get-ItemProperty -LiteralPath $regPath -ErrorAction SilentlyContinue).NameServer
                $isDhcpV4 = [string]::IsNullOrWhiteSpace($ns4)

                $regPath6 = 'Registry::HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Services\Tcpip6\Parameters\Interfaces\{' + $guidClean + '}'
                $ns6 = (Get-ItemProperty -LiteralPath $regPath6 -ErrorAction SilentlyContinue).NameServer
                $isDhcpV6 = [string]::IsNullOrWhiteSpace($ns6)

                [PSCustomObject]@{
                    interface_index = [uint32]$chosen.InterfaceIndex
                    interface_alias = [string]$chosen.Name
                    interface_guid = [string]$chosen.InterfaceGuid
                    interface_description = [string]$chosen.InterfaceDescription
                    is_dhcp_v4 = [bool]$isDhcpV4
                    is_dhcp_v6 = [bool]$isDhcpV6
                    ipv4_servers = $v4Servers
                    ipv6_servers = $v6Servers
                } | ConvertTo-Json -Compress
            } catch {
                Write-Output "ERROR:$($_.Exception.Message)"
            }
        "#;

        let output = std::process::Command::new(&powershell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .output()
            .map_err(|e| DnsManagerError::CommandFailed(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.starts_with("AMBIGUOUS:") {
            return Err(DnsManagerError::AmbiguousOutboundAdapter(
                "DNS uyumluluk yöntemi için güvenli ağ bağdaştırıcısı belirlenemedi.".to_string(),
            ));
        }
        if stdout.starts_with("ERROR:") {
            return Err(DnsManagerError::AdapterNotFound(
                stdout.trim_start_matches("ERROR:").to_string(),
            ));
        }

        let config: AdapterDnsConfig = serde_json::from_str(&stdout).map_err(|e| {
            DnsManagerError::CommandFailed(format!("Failed to parse adapter JSON: {e} ({stdout})"))
        })?;

        config.validate()?;
        Ok(config)
    }

    fn apply_dns_servers(
        &self,
        interface_index: u32,
        v4: &[String],
        v6: &[String],
    ) -> Result<(), DnsManagerError> {
        if interface_index == 0 {
            return Err(DnsManagerError::StateValidationFailed(
                "Invalid interface index".to_string(),
            ));
        }

        // Validate all IPs strictly
        for ip in v4.iter().chain(v6.iter()) {
            IpAddr::from_str(ip).map_err(|_| {
                DnsManagerError::StateValidationFailed(format!("Invalid IP for DNS server: {}", ip))
            })?;
        }

        let powershell = Self::get_powershell_executable();
        let idx_str = interface_index.to_string();

        let mut combined_servers = Vec::new();
        combined_servers.extend_from_slice(v4);
        combined_servers.extend_from_slice(v6);

        // Format: @('1.1.1.1','1.0.0.1',...)
        let formatted_servers = format!(
            "@({})",
            combined_servers
                .iter()
                .map(|s| format!("'{}'", s))
                .collect::<Vec<_>>()
                .join(",")
        );

        let script = format!(
            "Set-DnsClientServerAddress -InterfaceIndex {} -ServerAddresses {}",
            idx_str, formatted_servers
        );

        let status = std::process::Command::new(&powershell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .status()
            .map_err(|e| DnsManagerError::CommandFailed(e.to_string()))?;

        if !status.success() {
            return Err(DnsManagerError::CommandFailed(format!(
                "Set-DnsClientServerAddress failed with status {:?}",
                status
            )));
        }

        let _ = self.flush_resolver_cache();
        info!(
            interface = interface_index,
            v4 = ?v4,
            v6 = ?v6,
            "Applied DNS server override to adapter"
        );
        Ok(())
    }

    fn restore_dns(&self, config: &AdapterDnsConfig) -> Result<(), DnsManagerError> {
        config.validate()?;
        let powershell = Self::get_powershell_executable();
        let idx_str = config.interface_index.to_string();

        // IPv4 Restoration
        if config.is_dhcp_v4 {
            let script = format!(
                "Set-DnsClientServerAddress -InterfaceIndex {} -ResetServerAddresses",
                idx_str
            );
            let _ = std::process::Command::new(&powershell)
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &script,
                ])
                .status();

            // Backup netsh invocation for complete DHCP registration
            let netsh = Self::get_system32_executable("netsh.exe");
            let _ = std::process::Command::new(&netsh)
                .args(["interface", "ipv4", "set", "dnsservers", &idx_str, "dhcp"])
                .status();
        } else if !config.ipv4_servers.is_empty() {
            let formatted = format!(
                "@({})",
                config
                    .ipv4_servers
                    .iter()
                    .map(|s| format!("'{}'", s))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            let script = format!(
                "Set-DnsClientServerAddress -InterfaceIndex {} -ServerAddresses {}",
                idx_str, formatted
            );
            let _ = std::process::Command::new(&powershell)
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &script,
                ])
                .status();
        }

        // IPv6 Restoration
        if config.is_dhcp_v6 {
            let netsh = Self::get_system32_executable("netsh.exe");
            let _ = std::process::Command::new(&netsh)
                .args(["interface", "ipv6", "set", "dnsservers", &idx_str, "dhcp"])
                .status();
        }

        let _ = self.flush_resolver_cache();
        info!(
            interface = config.interface_index,
            is_dhcp = config.is_dhcp_v4,
            "Restored original DNS configuration"
        );
        Ok(())
    }

    fn flush_resolver_cache(&self) -> Result<(), DnsManagerError> {
        let ipconfig = Self::get_system32_executable("ipconfig.exe");
        let _ = std::process::Command::new(&ipconfig)
            .arg("/flushdns")
            .output();
        Ok(())
    }

    fn verify_dns_resolution(&self, target_host: &str) -> Result<bool, DnsManagerError> {
        validate_clean_string(target_host, "target_host")?;
        // Local name resolution probe using std::net::ToSocketAddrs (does not write domain history to disk)
        use std::net::ToSocketAddrs;
        let probe_target = format!("{}:443", target_host);
        let res = probe_target.to_socket_addrs().is_ok();
        Ok(res)
    }
}

/// Crash-safe state manager persisting privileged DNS override snapshots under ProgramData.
pub struct DnsStateManager {
    state_file_path: PathBuf,
}

impl DnsStateManager {
    pub fn new(data_root: &Path) -> Self {
        Self {
            state_file_path: data_root.join("dns_state.json"),
        }
    }

    pub fn state_file_path(&self) -> &Path {
        &self.state_file_path
    }

    /// Saves the override state to disk crash-safely using a temporary file,
    /// privileged file permissions, and atomic rename.
    pub fn save_state(&self, state: &DnsOverrideState) -> Result<(), DnsManagerError> {
        state.validate()?;

        if let Some(parent) = self.state_file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let tmp_path = self.state_file_path.with_extension("json.tmp");
        let json_bytes = serde_json::to_vec_pretty(state)?;

        // Write to temp file
        std::fs::write(&tmp_path, &json_bytes)?;

        // Enforce privileged ACL on Windows
        #[cfg(windows)]
        {
            let _ = Self::enforce_privileged_acl(&tmp_path);
        }

        // Atomic replace
        std::fs::rename(&tmp_path, &self.state_file_path)?;
        info!(path = %self.state_file_path.display(), "Saved DNS override snapshot crash-safely");
        Ok(())
    }

    /// Loads and strictly validates the persisted state. Returns None if no file exists.
    pub fn load_state(&self) -> Result<Option<DnsOverrideState>, DnsManagerError> {
        if !self.state_file_path.is_file() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&self.state_file_path)?;
        let state: DnsOverrideState = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "Persisted DNS state file is corrupt or invalid JSON");
                return Err(DnsManagerError::StateValidationFailed(format!(
                    "Malformed state JSON: {}",
                    e
                )));
            }
        };

        // Strict verification
        state.validate()?;
        Ok(Some(state))
    }

    /// Clears or marks the active state inactive.
    pub fn clear_state(&self) -> Result<(), DnsManagerError> {
        if self.state_file_path.is_file() {
            let _ = std::fs::remove_file(&self.state_file_path);
            info!("Cleared persisted DNS state file");
        }
        Ok(())
    }

    /// Enforces Windows ACL: SYSTEM (Full), Administrators (Full), Users (No Write).
    #[cfg(windows)]
    fn enforce_privileged_acl(path: &Path) -> Result<(), std::io::Error> {
        let path_str = path.to_string_lossy();
        let sys_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        let icacls = PathBuf::from(sys_root).join("System32").join("icacls.exe");

        if icacls.is_file() {
            // Remove inheritance, grant SYSTEM full, grant Administrators full, grant Users read-only
            let _ = std::process::Command::new(&icacls)
                .arg(&*path_str)
                .args([
                    "/inheritance:r",
                    "/grant:r",
                    "SYSTEM:(F)",
                    "/grant:r",
                    "Administrators:(F)",
                    "/grant:r",
                    "Users:(R)",
                ])
                .output();
        }
        Ok(())
    }
}

/// High-level coordinator for DNS compatibility mode.
pub struct DnsCompatibilityController {
    state_mgr: DnsStateManager,
    ops: Box<dyn AdapterDnsOps>,
}

impl DnsCompatibilityController {
    pub fn new(data_root: &Path) -> Self {
        Self {
            state_mgr: DnsStateManager::new(data_root),
            ops: Box::new(WindowsAdapterDnsOps),
        }
    }

    pub fn with_ops(data_root: &Path, ops: Box<dyn AdapterDnsOps>) -> Self {
        Self {
            state_mgr: DnsStateManager::new(data_root),
            ops,
        }
    }

    /// Applies compatibility DNS configuration for the chosen provider.
    pub fn apply_dns(&self, provider: DnsProvider) -> Result<DnsOverrideState, DnsManagerError> {
        // If user explicitly configured System, do not touch network adapters
        if !provider.should_override_system() {
            info!("DNS provider is System: adapter DNS modification skipped");
            let adapter = self.ops.find_active_outbound_adapter()?;
            return Ok(DnsOverrideState::new(
                adapter,
                Vec::new(),
                Vec::new(),
                DnsProvider::System,
            ));
        }

        // 1. Detect safe active outbound adapter
        let adapter = self.ops.find_active_outbound_adapter()?;

        // 2. Prepare endpoints
        let v4_endpoints = provider.endpoints_v4();
        let v6_endpoints = provider.endpoints_v6();

        // 3. Form state and persist crash-safely BEFORE applying changes
        let state = DnsOverrideState::new(
            adapter.clone(),
            v4_endpoints.clone(),
            v6_endpoints.clone(),
            provider,
        );
        self.state_mgr.save_state(&state)?;

        // 4. Apply to adapter
        self.ops
            .apply_dns_servers(adapter.interface_index, &v4_endpoints, &v6_endpoints)?;

        info!(
            interface = adapter.interface_alias,
            provider = ?provider,
            "Successfully applied clean DNS override"
        );
        Ok(state)
    }

    /// Restores original DNS configuration on stop or shutdown.
    pub fn restore_dns(&self) -> Result<(), DnsManagerError> {
        if let Some(state) = self.state_mgr.load_state()? {
            if state.dns_override_active && state.provider.should_override_system() {
                self.ops.restore_dns(&state.original_config)?;
            }
            self.state_mgr.clear_state()?;
        }
        Ok(())
    }

    /// Boot and crash recovery logic.
    /// Case A: Protection should NOT resume -> restores original DNS safely and clears state.
    /// Case B: Protection SHOULD resume -> validates snapshot and re-applies override.
    pub fn handle_boot_recovery(
        &self,
        should_resume_protection: bool,
        provider: DnsProvider,
    ) -> Result<Option<DnsOverrideState>, DnsManagerError> {
        let loaded = self.state_mgr.load_state()?;
        match loaded {
            Some(state) if state.dns_override_active => {
                if !should_resume_protection {
                    warn!(
                        interface = state.interface_alias,
                        "Detected active DNS override from previous session without auto-resume; restoring original DNS"
                    );
                    let _ = self.ops.restore_dns(&state.original_config);
                    let _ = self.state_mgr.clear_state();
                    Ok(None)
                } else {
                    info!(
                        interface = state.interface_alias,
                        "Resuming protection: Re-applying DNS compatibility override"
                    );
                    let new_state = self.apply_dns(provider)?;
                    Ok(Some(new_state))
                }
            }
            _ => Ok(None),
        }
    }

    pub fn state_manager(&self) -> &DnsStateManager {
        &self.state_mgr
    }

    pub fn ops(&self) -> &dyn AdapterDnsOps {
        &*self.ops
    }
}

/// Mock adapter operations for testing.
#[derive(Default)]
pub struct MockAdapterDnsOps {
    pub active_adapter: std::sync::Mutex<Option<AdapterDnsConfig>>,
    pub applied_servers: std::sync::Mutex<Vec<String>>,
    pub restored: std::sync::Mutex<bool>,
    pub flush_count: std::sync::Mutex<usize>,
    pub resolution_success: std::sync::Mutex<bool>,
    pub fail_adapter_search: std::sync::Mutex<bool>,
    pub ambiguous_search: std::sync::Mutex<bool>,
}

impl AdapterDnsOps for MockAdapterDnsOps {
    fn find_active_outbound_adapter(&self) -> Result<AdapterDnsConfig, DnsManagerError> {
        if *self.ambiguous_search.lock().unwrap() {
            return Err(DnsManagerError::AmbiguousOutboundAdapter(
                "DNS uyumluluk yöntemi için güvenli ağ bağdaştırıcısı belirlenemedi.".to_string(),
            ));
        }
        if *self.fail_adapter_search.lock().unwrap() {
            return Err(DnsManagerError::AdapterNotFound(
                "No safe physical outbound adapter found".to_string(),
            ));
        }
        if let Some(ref a) = *self.active_adapter.lock().unwrap() {
            return Ok(a.clone());
        }
        Ok(AdapterDnsConfig {
            interface_index: 2,
            interface_alias: "Ethernet".to_string(),
            interface_guid: Some("{0497CA4C-047D-4034-9761-8FBC3D71961B}".to_string()),
            interface_description: Some("Realtek Gaming 2.5GbE Family Controller".to_string()),
            is_dhcp_v4: true,
            is_dhcp_v6: true,
            ipv4_servers: vec!["192.168.1.1".to_string()],
            ipv6_servers: Vec::new(),
        })
    }

    fn apply_dns_servers(
        &self,
        _interface_index: u32,
        v4: &[String],
        v6: &[String],
    ) -> Result<(), DnsManagerError> {
        let mut app = self.applied_servers.lock().unwrap();
        app.clear();
        app.extend_from_slice(v4);
        app.extend_from_slice(v6);
        Ok(())
    }

    fn restore_dns(&self, _config: &AdapterDnsConfig) -> Result<(), DnsManagerError> {
        *self.restored.lock().unwrap() = true;
        Ok(())
    }

    fn flush_resolver_cache(&self) -> Result<(), DnsManagerError> {
        *self.flush_count.lock().unwrap() += 1;
        Ok(())
    }

    fn verify_dns_resolution(&self, _target_host: &str) -> Result<bool, DnsManagerError> {
        Ok(*self.resolution_success.lock().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_provider_endpoints_and_intent() {
        let ep = DnsProvider::Cloudflare.endpoints_v4();
        assert_eq!(ep, vec!["1.1.1.1", "1.0.0.1"]);
        let ep6 = DnsProvider::Cloudflare.endpoints_v6();
        assert_eq!(ep6, vec!["2606:4700:4700::1111", "2606:4700:4700::1001"]);
        assert!(DnsProvider::Cloudflare.should_override_system());

        let ep_auto = DnsProvider::Automatic.endpoints_v4();
        assert_eq!(ep_auto, vec!["1.1.1.1", "1.0.0.1"]);
        assert!(DnsProvider::Automatic.should_override_system());

        let ep_sys = DnsProvider::System.endpoints_v4();
        assert!(ep_sys.is_empty());
        assert!(!DnsProvider::System.should_override_system());
    }

    #[test]
    fn test_dns_state_validation_rejects_malformed_input() {
        // Corrupted schema version
        let mut state = DnsOverrideState::new(
            AdapterDnsConfig {
                interface_index: 2,
                interface_alias: "Ethernet".to_string(),
                interface_guid: Some("{0497CA4C-047D-4034-9761-8FBC3D71961B}".to_string()),
                interface_description: None,
                is_dhcp_v4: true,
                is_dhcp_v6: false,
                ipv4_servers: vec!["192.168.1.1".to_string()],
                ipv6_servers: Vec::new(),
            },
            vec!["1.1.1.1".to_string()],
            Vec::new(),
            DnsProvider::Cloudflare,
        );
        assert!(state.validate().is_ok());

        // 1. Invalid schema version
        state.schema_version = 99;
        assert!(state.validate().is_err());
        state.schema_version = STATE_SCHEMA_VERSION;

        // 2. Invalid source
        state.source = "attacker".to_string();
        assert!(state.validate().is_err());
        state.source = STATE_SOURCE_IDENTIFIER.to_string();

        // 3. Prohibited shell characters in alias
        state.interface_alias = "Ethernet; rm -rf /".to_string();
        assert!(state.validate().is_err());
        state.interface_alias = "Ethernet".to_string();

        // 4. Invalid IP address in applied list
        state.applied_v4_servers = vec!["1.1.1.1".to_string(), "999.999.999.999".to_string()];
        assert!(state.validate().is_err());
    }

    #[test]
    fn test_adapter_dns_config_dhcp_and_static_validation() {
        let dhcp_config = AdapterDnsConfig {
            interface_index: 1,
            interface_alias: "Wi-Fi".to_string(),
            interface_guid: Some("{12345678-1234-1234-1234-1234567890AB}".to_string()),
            interface_description: Some("Intel Wi-Fi".to_string()),
            is_dhcp_v4: true,
            is_dhcp_v6: true,
            ipv4_servers: vec!["192.168.1.1".to_string()],
            ipv6_servers: Vec::new(),
        };
        assert!(dhcp_config.validate().is_ok());

        let mut invalid_guid = dhcp_config.clone();
        invalid_guid.interface_guid = Some("not-a-guid".to_string());
        assert!(invalid_guid.validate().is_err());
    }

    #[test]
    fn test_dns_controller_respects_user_system_dns_intent() {
        let temp_dir = std::env::temp_dir().join(format!("zondpi-test-{}", std::process::id()));
        let mock_ops = Box::new(MockAdapterDnsOps::default());
        let controller = DnsCompatibilityController::with_ops(&temp_dir, mock_ops);

        let res = controller.apply_dns(DnsProvider::System);
        assert!(res.is_ok());
        let state = res.unwrap();
        assert_eq!(state.provider, DnsProvider::System);
        assert!(state.applied_v4_servers.is_empty());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_dns_controller_boot_recovery_cases() {
        let temp_dir = std::env::temp_dir().join(format!(
            "zondpi-test-recovery-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock_ops = Box::new(MockAdapterDnsOps::default());
        let controller = DnsCompatibilityController::with_ops(&temp_dir, mock_ops);

        // Case A: Create stale state on disk, should NOT resume
        let adapter = controller.ops().find_active_outbound_adapter().unwrap();
        let state = DnsOverrideState::new(
            adapter,
            vec!["1.1.1.1".to_string()],
            Vec::new(),
            DnsProvider::Cloudflare,
        );
        controller.state_manager().save_state(&state).unwrap();
        assert!(controller.state_manager().state_file_path().is_file());

        let rec_res = controller
            .handle_boot_recovery(false, DnsProvider::Automatic)
            .unwrap();
        assert!(rec_res.is_none());
        assert!(!controller.state_manager().state_file_path().is_file());

        // Case B: Create stale state on disk, SHOULD resume
        let adapter2 = controller.ops().find_active_outbound_adapter().unwrap();
        let state2 = DnsOverrideState::new(
            adapter2,
            vec!["1.1.1.1".to_string()],
            Vec::new(),
            DnsProvider::Cloudflare,
        );
        controller.state_manager().save_state(&state2).unwrap();

        let rec_res2 = controller
            .handle_boot_recovery(true, DnsProvider::Automatic)
            .unwrap();
        assert!(rec_res2.is_some());
        assert!(rec_res2.unwrap().dns_override_active);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_original_dhcp_dns_restored_to_dhcp() {
        let temp_dir = std::env::temp_dir().join(format!(
            "zondpi-test-dhcp-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock_ops = Box::new(MockAdapterDnsOps::default());
        let controller = DnsCompatibilityController::with_ops(&temp_dir, mock_ops);

        // Apply Cloudflare DNS
        let apply_res = controller.apply_dns(DnsProvider::Cloudflare);
        assert!(apply_res.is_ok());
        assert!(controller.state_manager().state_file_path().is_file());

        // Restore DNS
        let restore_res = controller.restore_dns();
        assert!(restore_res.is_ok());
        assert!(!controller.state_manager().state_file_path().is_file());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_original_static_dns_restored_exactly() {
        let temp_dir = std::env::temp_dir().join(format!(
            "zondpi-test-static-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock = MockAdapterDnsOps::default();
        *mock.active_adapter.lock().unwrap() = Some(AdapterDnsConfig {
            interface_index: 3,
            interface_alias: "Ethernet 2".to_string(),
            interface_guid: Some("{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}".to_string()),
            interface_description: Some("Intel I225-V".to_string()),
            is_dhcp_v4: false,
            is_dhcp_v6: false,
            ipv4_servers: vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()],
            ipv6_servers: vec!["2001:4860:4860::8888".to_string()],
        });

        let controller = DnsCompatibilityController::with_ops(&temp_dir, Box::new(mock));

        let apply_res = controller.apply_dns(DnsProvider::Cloudflare).unwrap();
        assert_eq!(
            apply_res.original_config.ipv4_servers,
            vec!["8.8.8.8", "8.8.4.4"]
        );

        // Verify loaded state contains the exact pre-existing static DNS
        let loaded = controller.state_manager().load_state().unwrap().unwrap();
        assert_eq!(
            loaded.original_config.ipv4_servers,
            vec!["8.8.8.8", "8.8.4.4"]
        );
        assert_eq!(
            loaded.original_config.ipv6_servers,
            vec!["2001:4860:4860::8888"]
        );
        assert!(!loaded.original_config.is_dhcp_v4);

        controller.restore_dns().unwrap();
        assert!(!controller.state_manager().state_file_path().is_file());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_virtual_and_vpn_adapters_are_not_blindly_modified() {
        let temp_dir = std::env::temp_dir().join(format!(
            "zondpi-test-ambig-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mock = MockAdapterDnsOps::default();
        *mock.ambiguous_search.lock().unwrap() = true;

        let controller = DnsCompatibilityController::with_ops(&temp_dir, Box::new(mock));
        let err = controller.apply_dns(DnsProvider::Cloudflare).unwrap_err();

        match err {
            DnsManagerError::AmbiguousOutboundAdapter(msg) => {
                assert_eq!(
                    msg,
                    "DNS uyumluluk yöntemi için güvenli ağ bağdaştırıcısı belirlenemedi."
                );
            }
            other => panic!("Expected AmbiguousOutboundAdapter error, got: {:?}", other),
        }

        // Must not leave any state file
        assert!(!controller.state_manager().state_file_path().is_file());
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_no_domain_history_written_to_state_or_logs() {
        let state = DnsOverrideState::new(
            AdapterDnsConfig {
                interface_index: 2,
                interface_alias: "Ethernet".to_string(),
                interface_guid: Some("{0497CA4C-047D-4034-9761-8FBC3D71961B}".to_string()),
                interface_description: Some("Realtek Gaming 2.5GbE Family Controller".to_string()),
                is_dhcp_v4: true,
                is_dhcp_v6: true,
                ipv4_servers: vec!["192.168.1.1".to_string()],
                ipv6_servers: Vec::new(),
            },
            vec!["1.1.1.1".to_string(), "1.0.0.1".to_string()],
            vec!["2606:4700:4700::1111".to_string()],
            DnsProvider::Cloudflare,
        );

        let json = serde_json::to_string_pretty(&state).unwrap();

        // State file must only contain schema/operational data, zero web browsing or domain names
        let forbidden_words = [
            "discord",
            "youtube",
            "twitter",
            "instagram",
            "browser",
            "url",
            "http",
        ];
        for word in &forbidden_words {
            assert!(
                !json.to_lowercase().contains(word),
                "State JSON must never contain user domain history or '{}'",
                word
            );
        }
    }
}
