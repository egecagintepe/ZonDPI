# Target System Architecture & Technical Design

## 1. Architectural Vision & Principles

The goal of ZonDPI is to create an auditable, high-performance, modular Windows DPI circumvention and network traffic management platform. The design guarantees:
1. **Strict Privilege Separation**: The Desktop User Interface runs as a standard unprivileged user process. Only the background Windows Service runs with elevated privileges (`SYSTEM` / Administrator).
2. **Multi-Engine Abstraction**: The core system supports multiple pluggable networking backends (WinDivert-based GoodbyeDPI, user-space socket-based ByeDPI, and future Native Rust async engine) under a unified lifecycle and capability interface.
3. **Antivirus & Security Interoperability**: Balanced compatibility with third-party security software (Kaspersky, Defender, ESET) through read-only environment detection, automated non-destructive self-testing, and driver-free fallback backends (ByeDPI).
4. **Crash & Fault Isolation**: Instability, memory corruption, or worker process crashes cannot compromise or terminate the Windows Service or the GUI.
5. **Zero Shell Scripts**: All configurations, presets, and runtime behaviors are represented as structured, strongly-typed JSON specifications.
6. **Supply Chain Integrity**: Every userspace binary is compiled strictly from source; third-party binary dependencies (official signed WinDivert kernel driver) are pinned with cryptographic SHA-256 verification.

```
┌──────────────────────────────────────────────────────────┐
│                   Desktop Application                    │
│             (Tauri v2 + React + TypeScript)              │
│                Unprivileged (Medium IL)                  │
└────────────────────────────┬─────────────────────────────┘
                             │ Local IPC
                             │ (Named Pipe + Versioned JSON Framing)
┌────────────────────────────▼─────────────────────────────┐
│                 Windows Privileged Service               │
│                        (Rust Core)                       │
│                   Elevated (High / SYSTEM)               │
│                                                          │
│  ┌─────────────────┐ ┌─────────────────┐ ┌────────────┐  │
│  │ Profile Engine  │ │  DNS Subsystem  │ │Diagnostics │  │
│  └────────┬────────┘ └────────┬────────┘ └────────────┘  │
│           │                   │                          │
│  ┌────────▼───────────────────▼───────────────────────┐  │
│  │         Compatibility & Security Detector          │  │
│  │  (Read-only SecurityCenter2 + Health-check Gate)   │  │
│  └────────────────────────────┬───────────────────────┘  │
│                               │                          │
│  ┌────────────────────────────▼───────────────────────┐  │
│  │          Supervisor & Job Object Manager           │  │
│  └──────┬─────────────────────┬─────────────────┬─────┘  │
└─────────┼─────────────────────┼─────────────────┼────────┘
          │ (Process Boundary)  │                 │
┌─────────▼──────────┐ ┌────────▼──────────┐ ┌────▼─────────────┐
│  GoodbyeDPI Worker │ │   ByeDPI Worker   │ │   Native Engine  │
│  (WinDivert / WFP) │ │(User-Space SOCKS5)│ │  (Pure Rust WFP) │
│   [System-Wide]    │ │ [AV-Compatible]   │ │  [Milestone 7+]  │
└─────────┬──────────┘ └───────────────────┘ └──────────────────┘
          │
┌─────────▼────────────────────────────────────────────────┐
│                   WinDivert Subsystem                    │
│    (WinDivert.dll + Official Digitally Signed Driver)    │
└──────────────────────────────────────────────────────────┘
```

---

## 2. Component Breakdown & Monorepo Layout

```
zondpi/
├── apps/
│   └── desktop/                  # Tauri v2 Desktop GUI (React + TypeScript + Vite)
│
├── service/
│   └── windows-service/          # Privileged Windows Service daemon (Rust)
│       ├── src/
│       │   ├── main.rs           # Win32 ServiceMain entrypoint
│       │   ├── ipc/              # Versioned Named Pipe IPC protocol & server
│       │   └── supervisor/       # Worker Process Supervisor & Job Object interface
│       └── Cargo.toml
│
├── core/
│   ├── packet-engine/            # Engine Traits, Capability Matrix & Lifecycle Contracts
│   ├── compatibility/            # Security Product Detector & Health Checks
│   │   ├── security_detector.rs  # Read-only WMI root\SecurityCenter2 probe
│   │   ├── health_check.rs       # Bounded non-destructive network self-tests
│   │   └── compatibility_mgr.rs  # Deterministic recommendation policy
│   ├── protocol-parsers/         # Zero-copy IPv4/IPv6, TCP, UDP, DNS, TLS, HTTP parsers
│   ├── flow-tracker/             # Connection state tracking & TTL hop calculators
│   ├── dns/                      # DNS Manager (DoH, DoT, UDP Proxy, Adapter Rollback)
│   ├── profile-engine/           # Profile validator, schema resolver, preset compiler
│   └── diagnostics/              # DPI probing, DNS health, packet loss & latency tests
│
├── engines/
│   ├── goodbye/                  # Source-built C GoodbyeDPI engine wrapper (WinDivert)
│   │   ├── upstream/             # Vendored GoodbyeDPI C source
│   │   └── src/                  # Worker process wrapper
│   ├── byedpi/                   # Source-built C ByeDPI SOCKS5 engine (Driver-Independent)
│   │   ├── upstream/             # Vendored ByeDPI C source
│   │   └── src/                  # SOCKS5 worker process wrapper
│   └── native/                   # Native Rust packet engine (Future milestone)
│
├── profiles/                     # Structured Profile Definitions
│   ├── turkey/                   # Turkish ISP Presets (Turk Telekom, Superonline, etc.)
│   └── schema/                   # JSON Schema for profile verification
│
├── third_party/
│   └── windivert/                # Pinned WinDivert Headers, Libs, Official Signed Driver
│       ├── include/              # windivert.h
│       ├── x64/                  # WinDivert.dll, WinDivert64.sys (Pinned Official Signed)
│       └── verify_driver.ps1     # Cryptographic integrity validator
│
├── docs/                         # Architecture, Security, Build, Compatibility, and Audit Docs
├── .github/workflows/            # Windows x64 CI Workflows
└── DEPENDENCIES.md               # Supply Chain & Dependency Tracking Manifest
```

---

## 3. Pluggable Engine Abstraction & Capabilities

All networking engines adhere to the unified `NetworkEngine` abstraction in `core/packet-engine`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineId {
    GoodbyeDpi,
    ByeDpi,
    Native,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityCompatibilityRating {
    High,    // Driver-free socket operation (ByeDPI); kernel driver conflict risk: Very Low
    Medium,  // Requires driver with known exclusion mechanisms
    Low,     // Known active WFP collisions without manual configuration (WinDivert)
}
```

---

## 4. Application Routing Strategy for ByeDPI (SOCKS5 Mode)

Unlike WinDivert which captures all system traffic at the kernel WFP layer, a local SOCKS5 proxy operates in user space.

### Technical Analysis: Windows System Proxy vs SOCKS5

> [!IMPORTANT]
> **[FACT]**: The Windows System Proxy (configured via WinINet API / `InternetSetOption` / `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings`) was primarily engineered for HTTP/HTTPS Web proxies.
> 
> **[LIMITATION]**: Setting a SOCKS proxy globally in Windows System Proxy settings has several known limitations:
> 1. Many non-browser Win32 applications ignore system SOCKS proxy settings entirely and connect directly.
> 2. UWP (Universal Windows Platform) and Windows Store applications isolate loopback connections and will fail or bypass the local SOCKS proxy without explicit AppContainer loopback exemptions (`CheckNetIsolation.exe`).
> 3. WinINet SOCKS support does not reliably route DNS queries through the proxy unless explicitly configured by the application, leading to DNS leaks.
> 
> Therefore, ZonDPI **does NOT assume** that setting the system proxy globally routes all Windows applications through ByeDPI.

### Initial Reliable Routing Methods (Baseline)

ZonDPI establishes three reliable, deterministic application routing methods for ByeDPI:

```
┌───────────────────────────────────────────────────────────────────────────┐
│                 ByeDPI (SOCKS5) Reliable Routing Methods                  │
└───────────────────────────────────────────────────────────────────────────┘
  │
  ├─► [Method 1: Native Application SOCKS5 Configuration]
  │   Applications with first-class SOCKS5 and remote DNS support (e.g., Firefox, Telegram).
  │   Configured to use 127.0.0.1:1080 with remote DNS resolution enabled.
  │
  ├─► [Method 2: Chromium / Electron CLI Argument Launcher]
  │   Chromium and Electron applications (Chrome, Edge, Discord, Brave, Spotify).
  │   Launched directly with: --proxy-server="socks5://127.0.0.1:1080"
  │
  └─► [Method 3: Application-Specific Proxy Configuration]
      Individual desktop tools and game clients configured via their respective proxy settings.
```

*Note: Transparent system-wide user-space interception (without kernel drivers) is reserved for a dedicated future milestone.*

---

## 5. IPC Architecture & Communication Flow

### 5.1 Transport & Security
* **Transport**: Windows Named Pipe `\\.\pipe\zondpi-service-ipc`.
* **Access Control**: Pipe created with explicit Windows Security Descriptor allowing only `Authenticated Users` (Read/Write) and `SYSTEM` / `Administrators` (Full Control). Rejects untrusted or network connections.
* **Protocol**: Versioned message framing:
  - `IpcRequest { protocol_version: u32, request_id: String, command: ServiceCommand }`
  - `IpcResponse { protocol_version: u32, request_id: String, result: ServiceResult }`

---

## 6. DNS Management & Fail-Safe Rollback

1. **Adapter Snapshotting**: Prior to modifying any network adapter DNS settings, the service snapshots the current IPv4/IPv6 DNS server list for all active interfaces via the Windows IP Helper API.
2. **Atomic Rollback Hook**:
   * Stored in persistent local state on disk.
   * Registered in Win32 service shutdown and crash handlers.
   * Restored automatically if an unclean shutdown is detected upon reboot.
3. **Flushing Resolver Cache**: Automatically calls `DnsFlushResolverCache` from `dnsapi.dll` upon any DNS transition.
