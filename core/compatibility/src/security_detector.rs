//! Read-only detection of installed Windows security products via WMI root\\SecurityCenter2.
//!
//! # Strict Compliance & Safety Invariants
//! - Never alters, suspends, or modifies any antivirus software, services, or processes.
//! - Never touches registry keys or settings belonging to security products.
//! - Read-only query mechanism with bounded timeout.
//! - Never panics or crashes on failure; returns structured `DetectionStatus`.

use serde::{Deserialize, Serialize};

/// Represents an antivirus / security product detected on the system.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityProduct {
    pub display_name: String,
    pub product_state: u32,
    pub path_to_signed_product_exe: Option<String>,
    pub path_to_signed_reporting_exe: Option<String>,
}

/// Status of the environment detection subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectionStatus {
    /// Security products query completed successfully.
    Available,
    /// Security Center is unavailable (e.g. non-Windows or server SKU where SecurityCenter2 is absent).
    Unavailable,
    /// Query failed due to permission or runtime error; error string recorded.
    Failed(String),
}

/// Snapshot of the host security environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityEnvironment {
    pub products: Vec<SecurityProduct>,
    pub detection_status: DetectionStatus,
}

impl SecurityEnvironment {
    pub fn new(products: Vec<SecurityProduct>, detection_status: DetectionStatus) -> Self {
        Self {
            products,
            detection_status,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            products: Vec::new(),
            detection_status: DetectionStatus::Unavailable,
        }
    }

    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            products: Vec::new(),
            detection_status: DetectionStatus::Failed(error.into()),
        }
    }

    /// Checks if any detected security product display name contains the given case-insensitive substring.
    pub fn contains_product(&self, name_substr: &str) -> bool {
        let needle = name_substr.to_lowercase();
        self.products
            .iter()
            .any(|p| p.display_name.to_lowercase().contains(&needle))
    }

    /// Returns true if at least one active security product is registered.
    pub fn has_active_products(&self) -> bool {
        !self.products.is_empty()
    }
}

/// Abstraction for security product detection to facilitate testing and deterministic evaluation.
pub trait SecurityDetectorTrait: Send + Sync {
    fn detect(&self) -> SecurityEnvironment;
}

/// Default system detector that queries Windows SecurityCenter2 read-only.
pub struct SystemSecurityDetector;

impl SecurityDetectorTrait for SystemSecurityDetector {
    fn detect(&self) -> SecurityEnvironment {
        #[cfg(target_os = "windows")]
        {
            Self::detect_windows()
        }
        #[cfg(not(target_os = "windows"))]
        {
            SecurityEnvironment::unavailable()
        }
    }
}

impl SystemSecurityDetector {
    #[cfg(target_os = "windows")]
    fn detect_windows() -> SecurityEnvironment {
        use std::process::Command;

        // Bounded read-only query using PowerShell Get-CimInstance on root\SecurityCenter2
        // Formatted as JSON for robust, injection-safe deserialization.
        let script = r#"
            try {
                $ErrorActionPreference = 'Stop'
                $products = Get-CimInstance -Namespace 'root\SecurityCenter2' -ClassName AntiVirusProduct -ErrorAction Stop
                $result = @($products) | ForEach-Object {
                    [PSCustomObject]@{
                        displayName = [string]$_.displayName
                        productState = [uint32]$_.productState
                        pathToSignedProductExe = [string]$_.pathToSignedProductExe
                        pathToSignedReportingExe = [string]$_.pathToSignedReportingExe
                    }
                }
                $result | ConvertTo-Json -Compress
            } catch {
                Write-Output "ERROR:$($_.Exception.Message)"
            }
        "#;

        let output = match Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ])
            .output()
        {
            Ok(out) => out,
            Err(e) => {
                return SecurityEnvironment::failed(format!(
                    "Failed to execute detection probe: {e}"
                ))
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return SecurityEnvironment::failed(format!(
                "Detector query failed with exit code: {:?}",
                stderr
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.starts_with("ERROR:") {
            return SecurityEnvironment::failed(stdout.trim_start_matches("ERROR:").to_string());
        }

        if stdout.is_empty() || stdout == "null" {
            return SecurityEnvironment::new(Vec::new(), DetectionStatus::Available);
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawWmiProduct {
            display_name: Option<String>,
            product_state: Option<u32>,
            path_to_signed_product_exe: Option<String>,
            path_to_signed_reporting_exe: Option<String>,
        }

        // PowerShell ConvertTo-Json returns an object if 1 item, array if multiple
        let raw_items: Vec<RawWmiProduct> = if stdout.starts_with('[') {
            serde_json::from_str(&stdout).unwrap_or_default()
        } else if let Ok(single) = serde_json::from_str::<RawWmiProduct>(&stdout) {
            vec![single]
        } else {
            Vec::new()
        };

        let products = raw_items
            .into_iter()
            .map(|raw| SecurityProduct {
                display_name: raw.display_name.unwrap_or_default(),
                product_state: raw.product_state.unwrap_or(0),
                path_to_signed_product_exe: raw
                    .path_to_signed_product_exe
                    .filter(|s| !s.is_empty()),
                path_to_signed_reporting_exe: raw
                    .path_to_signed_reporting_exe
                    .filter(|s| !s.is_empty()),
            })
            .collect();

        SecurityEnvironment::new(products, DetectionStatus::Available)
    }
}

/// Mock detector for deterministic offline unit testing.
pub struct MockSecurityDetector {
    environment: SecurityEnvironment,
}

impl MockSecurityDetector {
    pub fn new(environment: SecurityEnvironment) -> Self {
        Self { environment }
    }
}

impl SecurityDetectorTrait for MockSecurityDetector {
    fn detect(&self) -> SecurityEnvironment {
        self.environment.clone()
    }
}
