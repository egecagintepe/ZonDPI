//! Network health, DNS poison testing, and DPI probing diagnostics.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub ipv4_working: bool,
    pub ipv6_working: bool,
    pub dns_poisoned: bool,
    pub passive_dpi_detected: bool,
    pub active_dpi_detected: bool,
    pub recommended_preset: String,
}

pub struct DiagnosticRunner;
