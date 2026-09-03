# Dependency Tracking & Supply Chain Trust Manifest

## 1. Supply Chain Trust Policy

Our security model enforces strict rules on all dependencies:
1. **Userspace Source-Built Policy**: All userspace code (GoodbyeDPI C engine, ByeDPI C engine, Rust Windows Service, core libraries, and Tauri frontend) is **100% compiled from source**. We never distribute precompiled third-party `.exe` binaries.
2. **Cryptographic Pinning**: Any third-party binary asset distributed with the application (specifically the official signed WinDivert kernel driver) is pinned to an immutable version and verified with SHA-256 checksums before compilation, packaging, and execution.
3. **License Compliance**: All licenses and copyright notices from upstream projects are preserved, bundled, and accessible within the application.
4. **AGPL Isolation**: AGPL-licensed software (e.g. ProxiFyre) is strictly **reference-only**. No AGPL code is vendored or linked into the ZonDPI codebase.
5. **Exact Commit Provenance**: Every vendored upstream dependency is tracked with its source repository URL, exact commit SHA, upstream license, build mechanism, and exact modification status.

---

## 2. Pinned Vendored Dependencies Manifest

| Dependency Name | Upstream Repository | Exact Commit SHA / Version | License | Vendored Path | Build Mechanism | Modifications Made by ZonDPI |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **GoodbyeDPI** | `https://github.com/ValdikSS/GoodbyeDPI` | `3114036d096865fba19e8b379d5fb8216423d21c` (`v0.2.3rc3`) | Apache-2.0 | `engines/goodbye/upstream` | Source-built with MinGW-w64 (`make` / `gcc`) linking `-lWinDivert -lws2_32 -l:libssp.a` | None to packet filter logic. Added stdout/stderr unbuffering for real-time IPC supervisor log capture. Upstream Makefile adjusted for standard 64-bit Windows PE (`-pie` removed for valid dynamic base execution on x64 Windows; `goodbyedpi-rc.rc` embeds icon). Turkey batch scripts replaced by structured JSON profiles (`profiles/turkey/*.json`). Wrapped via child process supervisor. |
| **ByeDPI (`ciadpi`)** | `https://github.com/hufrea/byedpi` | `ba532298de7b28cfe854aea83d061369d13ca290` (post-`v0.17.3`) | MIT | `engines/byedpi/upstream` | Source-built with MinGW-w64 (`make windows` / `gcc`) linking `-lws2_32 -lmswsock` | None to core proxy code. Wrapped via child process supervisor (`zondpi-engine-byedpi`). |
| **WinDivert (Userspace DLL & Headers)** | `https://github.com/basil00/WinDivert` | `v2.2` (declared/provisioned as `v2.2.0-D` in metadata) | LGPL-3.0 / GPL-2.0 | `third_party/windivert/` | Header (`include/windivert.h`) used at compile time; DLL (`x64/WinDivert.dll`) pinned binary | None. Verified with SHA-256 (`6110BFA44667405179C3E15E12AF1B62037E447ED59B054B19042032995E6C7E`). |
| **WinDivert (x64 Kernel Driver)** | `https://github.com/basil00/WinDivert` | `v2.2` (declared/provisioned as `v2.2.0-D` in metadata) | LGPL-3.0 / GPL-2.0 | `third_party/windivert/x64/WinDivert64.sys` | Official signed binary driver | None. Pinned official driver verified with SHA-256 (`E69B5BA3F0CD6CFB2983E442636E7F0B342B61B15264B0328317D4559C82CF50`). |
| **uthash** | `https://github.com/troydhanson/uthash` | `79f90641cb9cdb34005b6329bf336ee3661eb782` (`v2.3.0`) | BSD-1-Clause | `engines/goodbye/upstream/src/utils/uthash.h` | Header-only C macro library | None. Preserved original header. |

---

## 3. Rust Workspace Crates Manifest

| Crate Name | Upstream Repository | Version | License | Role in ZonDPI |
| :--- | :--- | :--- | :--- | :--- |
| **zondpi-packet-engine** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Core abstractions, `EngineId`, `EngineCapabilities`, `NetworkEngine` trait. |
| **zondpi-compatibility** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Read-only WMI security detector, deterministic `CompatibilityManager`. |
| **zondpi-flow-tracker** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Connection tracking and TCP auto-TTL hop calculator. |
| **zondpi-protocol-parsers** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Zero-copy TLS ClientHello SNI and HTTP parser primitives. |
| **zondpi-dns** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Windows adapter DNS snapshotting and rollback manager. |
| **zondpi-profile-engine** | Internal / Monorepo | `0.1.0` | Apache-2.0 | JSON profile validation and preset compiler. |
| **zondpi-diagnostics** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Network telemetry, DPI probing, and health diagnostics. |
| **zondpi-engine-goodbye** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Process supervisor wrapper for GoodbyeDPI C worker. |
| **zondpi-engine-byedpi** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Process supervisor wrapper for ByeDPI C worker. |
| **zondpi-service** | Internal / Monorepo | `0.1.0` | Apache-2.0 | Privileged Windows Service daemon, IPC protocol, supervisor. |

---

## 4. Reference-Only Audit Projects (Not Distributed / Not Linked)

| Project Name | Source Repository | License | Role in Project | Copyleft & Security Implications |
| :--- | :--- | :--- | :--- | :--- |
| **SplitWire-Turkey** | `https://github.com/cagritaskn/SplitWire-Turkey` | MIT | **Reference Only** | Analyzed for Turkish ISP presets and Discord routing workarounds. |
| **ProxiFyre** | `https://github.com/wiresock/proxifyre` | **AGPL-3.0** | **Reference Only** | Analyzed for transparent SOCKS routing. **DO NOT VENDOR**: AGPL-3.0 imposes strict copyleft; relies on NDIS packet filter driver. |
| **WireSockUI** | `https://github.com/wiresock/WireSockUI` | MIT | **Reference Only** | Analyzed for Windows tunnel UI patterns and process routing interfaces. |

---

## 5. Kernel Driver Verification Analysis: WinDivert Official Driver

### 5.1 Driver Signing Scope & Attestation
WinDivert consists of both userspace components (`WinDivert.dll`) and a kernel-mode driver (`WinDivert64.sys`).

1. **Official Upstream Digitally Signed Driver**:
   * The binary `WinDivert64.sys` distributed with official releases is digitally signed with an Authenticode certificate and Microsoft cross-signature, permitting it to load on standard 64-bit Windows 10 and Windows 11 systems without enabling test-signing modes.
   * Compiling the `.sys` driver from source without an active Microsoft Hardware Developer Center attestation signature would fail Driver Signature Enforcement (DSE) on production Windows installations.
2. **Integrity Verification Mechanism**:
   * The driver binary is pinned, checked into `third_party/windivert/x64/`, and verified using SHA-256 checksums (`verify_driver.ps1`) during local builds, CI pipelines, and before service driver initialization.
3. **Userspace Independence**:
   * All userspace networking logic interacting with the driver is built 100% from source in our repository.
