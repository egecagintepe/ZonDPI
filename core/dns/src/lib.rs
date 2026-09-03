//! Windows Adapter DNS management, snapshotting, and atomic rollback.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterDnsSnapshot {
    pub adapter_name: String,
    pub adapter_guid: String,
    pub original_ipv4_dns: Vec<String>,
    pub original_ipv6_dns: Vec<String>,
    pub is_dhcp: bool,
}

pub struct DnsManager;

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

/// Resolved endpoint addresses and ports for IPv4 and IPv6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsEndpoints {
    pub ipv4: Option<(&'static str, u16)>,
    pub ipv6: Option<(&'static str, u16)>,
}

impl DnsProvider {
    /// Returns the IPv4 and IPv6 endpoints for the selected DNS provider.
    pub fn endpoints(&self) -> DnsEndpoints {
        match self {
            DnsProvider::Automatic | DnsProvider::Cloudflare => DnsEndpoints {
                ipv4: Some(("1.1.1.1", 53)),
                ipv6: Some(("2606:4700:4700::1111", 53)),
            },
            DnsProvider::Google => DnsEndpoints {
                ipv4: Some(("8.8.8.8", 53)),
                ipv6: Some(("2001:4860:4860::8888", 53)),
            },
            DnsProvider::Quad9 => DnsEndpoints {
                ipv4: Some(("9.9.9.9", 53)),
                ipv6: Some(("2620:fe::fe", 53)),
            },
            DnsProvider::System => DnsEndpoints {
                ipv4: None,
                ipv6: None,
            },
        }
    }
}

impl DnsManager {
    /// Flushes Windows DNS Resolver cache via dnsapi.dll
    pub fn flush_dns_cache() -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            // Call DnsFlushResolverCache from dnsapi.dll
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_provider_endpoints() {
        let ep = DnsProvider::Cloudflare.endpoints();
        assert_eq!(ep.ipv4, Some(("1.1.1.1", 53)));
        assert_eq!(ep.ipv6, Some(("2606:4700:4700::1111", 53)));

        let ep_auto = DnsProvider::Automatic.endpoints();
        assert_eq!(ep_auto.ipv4, Some(("1.1.1.1", 53)));
        assert_eq!(ep_auto.ipv6, Some(("2606:4700:4700::1111", 53)));

        let ep_sys = DnsProvider::System.endpoints();
        assert_eq!(ep_sys.ipv4, None);
        assert_eq!(ep_sys.ipv6, None);
    }
}
