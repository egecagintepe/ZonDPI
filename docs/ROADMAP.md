# Project Roadmap & Implementation Milestones

## 1. Roadmap Overview

This roadmap defines the structured progression from the initial audit and repository bootstrap through the implementation of the privileged Windows service, multi-engine architecture, Kaspersky compatibility engine, profile engine, desktop GUI, diagnostic subsystem, installer, and future native Rust packet engine.

---

## 2. Milestone Breakdown

### Milestone 0: Repository Bootstrap & Architecture Baseline
* **Objective**: Establish monorepo structure, Rust workspace, documentation, licenses, and dependency manifests.
* **Dependencies**: None.
* **Files & Modules Affected**:
  * Root configuration: `Cargo.toml` (workspace), `.gitignore`, `.editorconfig`, `LICENSE`, `DEPENDENCIES.md`.
  * Documentation: `docs/ARCHITECTURE.md`, `docs/UPSTREAM_AUDIT.md`, `docs/SECURITY_MODEL.md`, `docs/BUILD.md`, `docs/ROADMAP.md`, `docs/COMPATIBILITY_KASPERSKY.md`, `docs/COMPATIBILITY_MATRIX.md`.
  * Build scripts: `build.ps1`, `third_party/windivert/verify_driver.ps1`.
* **Tasks**:
  1. Initialize monorepo workspace directory tree.
  2. Setup Rust root workspace manifest and crate package placeholders.
  3. Pin third-party WinDivert assets and create SHA-256 verification script.
  4. Setup GitHub Actions CI baseline.
* **Expected Output**: Clean, organized repository with fully documented architectural requirements and automated driver integrity verification.
* **Tests**:
  * Execute `verify_driver.ps1` to validate pinned WinDivert hashes.
  * Verify Rust workspace compiles with `cargo check --workspace`.
* **Completion Criteria**: Workspace initializes without errors; CI workflow triggers and passes baseline checks.

---

### Milestone 1: Reproducible GoodbyeDPI Engine Source Build
* **Objective**: Vendor upstream GoodbyeDPI userspace C source and build it reproducibly as an isolated worker executable.
* **Dependencies**: Milestone 0.
* **Files & Modules Affected**:
  * `engines/goodbye/upstream/src/` (Vendored C source files: `goodbyedpi.c`, `dnsredir.c`, `fakepackets.c`, `ttltrack.c`, `blackwhitelist.c`, `utils/*`).
  * `engines/goodbye/build.rs` or custom `Makefile`.
  * `engines/goodbye/src/` (Worker process wrapper & CLI launcher).
* **Tasks**:
  1. Vendor clean `ValdikSS/GoodbyeDPI` v0.2.3rc3 source into `engines/goodbye/upstream/`.
  2. Configure reproducible compilation flags using MinGW-w64 targeting `x86_64`.
  3. Create automated C unit test harness for protocol parsers (`dnsredir`, `fakepackets`).
  4. Build standalone worker binary `zondpi-engine-worker.exe`.
* **Expected Output**: Standalone `zondpi-engine-worker.exe` built 100% from source with zero foreign binary dependencies.
* **Tests**:
  * Launch `zondpi-engine-worker.exe -h` and verify all help options match upstream.
  * Execute packet fragmentation integration tests against mock HTTP/TLS test server.
* **Completion Criteria**: C engine compiles cleanly without warnings on both MinGW-w64 and CI; passes mock packet tests.

---

### Milestone 1.5: Compatibility Engine & Kaspersky Interoperability
* **Objective**: Implement security product detection, automated non-destructive health-checks, and the source-built ByeDPI SOCKS5 engine fallback.
* **Dependencies**: Milestone 1.
* **Files & Modules Affected**:
  * `core/compatibility/` (Security product detector, health checks, compatibility manager).
  * `engines/byedpi/` (Vendored ByeDPI C source, worker wrapper, and build scripts).
  * `docs/COMPATIBILITY_KASPERSKY.md`, `docs/COMPATIBILITY_MATRIX.md`.
* **Tasks**:
  1. Implement read-only antivirus product detection via WMI `root\SecurityCenter2`.
  2. Vendor clean `hufrea/byedpi` source into `engines/byedpi/upstream/` and build `zondpi-engine-byedpi.exe` from source.
  3. Implement non-destructive, bounded health checks for WinDivert packet capture & reinjection.
  4. Implement automated engine selection (GoodbyeDPI if healthy; ByeDPI fallback if AV collision is detected).
  5. Implement application routing helper (CLI launcher for Chromium/Discord + system proxy config).
* **Expected Output**: Working dual-engine subsystem capable of running GoodbyeDPI or driver-free ByeDPI with automatic compatibility switching.
* **Tests**:
  * Unit test for WMI security product enumeration.
  * Integration test: ByeDPI SOCKS5 listener startup, test HTTPS connection through proxy, and verify TLS segmentation.
  * Bounded health check test: verify safe rollback if packet reinjection fails.
* **Completion Criteria**: ByeDPI builds from source; security detector accurately reports active antivirus; engine manager switches backends dynamically.

---

### Milestone 2: Privileged Rust Windows Service & Secure Named Pipe IPC [COMPLETED]
* **Objective**: Implement the elevated Windows Service daemon with secure Named Pipe IPC, single active engine coordination, transactional switching, and multi-engine process supervision inside Windows Job Objects.
* **Status**: **Completed & Fully Tested** (51/51 automated tests passing, real ByeDPI lifecycle & SOCKS5 handshake verified).
* **Files & Modules Implemented**:
  * `service/windows-service/src/main.rs`: Multi-mode CLI entrypoint (Service Host, Foreground Dev Host, SCM Management subcommands).
  * `service/windows-service/src/service_scm.rs`: Windows Service SCM registration (`install`, `uninstall`, `start`, `stop`, `status`).
  * `service/windows-service/src/hosts/`: Foreground console host (`hosts/foreground.rs`) and Windows SCM host (`hosts/service.rs`).
  * `service/windows-service/src/supervisor/`: RAII Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`), process supervisor with bounded crash detection (3 restarts in 60s with exponential backoff), and 200-line thread-safe log ring buffer.
  * `service/windows-service/src/engine_adapter/`: Typed GoodbyeDPI & ByeDPI argument builders and health check probes (SOCKS5 handshake & WinDivert presence).
  * `service/windows-service/src/engine_mgr/`: Transactional engine coordinator with Auto mode compatibility resolution and transparent fallback.
  * `service/windows-service/src/ipc_server/`: Length-prefixed Named Pipe server (`\\.\pipe\zondpi-service-ipc`).
  * `core/ipc-protocol/`: Wire protocol DTOs, codec, and max 1 MiB framing protection.
  * `tools/zondpi-cli/`: Standalone control CLI with rich human-readable status, logs, recommendation, and profile management cards.
  * `docs/IPC_PROTOCOL.md`, `docs/SERVICE.md`: Formal wire protocol and service lifecycle specifications.

---

### Milestone 3: Profile Engine, Turkish ISP Presets & DNS Manager
* **Objective**: Build structured profile management system with complete coverage of Turkish ISP presets and fail-safe DNS rollback.
* **Dependencies**: Milestone 2.
* **Files & Modules Affected**:
  * `core/profile-engine/` (JSON Schema parser, validator, preset registry).
  * `core/dns/` (Adapter DNS manager, DoH/DoT resolvers, cache flusher).
  * `profiles/turkey/*.json` (Türk Telekom, Superonline, Vodafone structured profiles).
  * `profiles/schema/profile.schema.json`.
* **Tasks**:
  1. Define formal JSON schema for DPI bypass profiles (fragmentation offsets, fake packet modes, TTL, DNS settings, engine target).
  2. Port all 7 GoodbyeDPI-Turkey preset modes and ByeDPI modes into structured profile definitions.
  3. Implement adapter DNS snapshot and rollback mechanism in `core/dns/`.
  4. Implement `core/dns/flusher.rs` using `DnsFlushResolverCache`.
* **Expected Output**: Fully validated profile engine and atomic DNS management subsystem.
* **Tests**:
  * Validate all JSON profile files against JSON Schema in CI.
  * Test DNS adapter snapshot/rollback: modify adapter DNS, trigger simulated service crash, verify rollback restores original DNS on service reboot.
* **Completion Criteria**: All Turkish ISP presets reproducible and validated; zero DNS leaks or stuck configurations upon abnormal exit.

---

### Milestone 4: Desktop GUI & System Tray Application
* **Objective**: Create modern, unprivileged Tauri v2 desktop application with real-time status, compatibility mode toggle, and profile selection.
* **Dependencies**: Milestone 3.
* **Files & Modules Affected**:
  * `apps/desktop/src/` (React + TypeScript UI components, Zustand stores, IPC client).
  * `apps/desktop/src-tauri/` (Tauri host, system tray controller, auto-start registry manager).
  * `apps/desktop/package.json`.
* **Tasks**:
  1. Initialize Tauri v2 application with Vite, React, and TypeScript.
  2. Implement IPC client connecting to `\\.\pipe\zondpi-service-ipc`.
  3. Build UI views: Dashboard, Profile Selector (with Turkish ISP quick-presets), Kaspersky Compatibility Banner, DNS Configuration, Live Logs, Settings.
  4. Implement Windows System Tray with quick Start/Stop/Switch toggles and status indicators.
  5. Implement Windows auto-start on logon via unprivileged registry key (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
* **Expected Output**: Polished desktop client running without administrator privileges, communicating seamlessly with background service.
* **Tests**:
  * Frontend unit tests with Vitest/React Testing Library.
  * End-to-end UI interaction testing with simulated IPC responses.
* **Completion Criteria**: GUI starts in <500ms; consumes <30MB RAM; allows switching profiles and displays real-time engine status.

---

### Milestone 5: Network Diagnostics & DPI Probing Subsystem
* **Objective**: Implement automated network health checks, ISP detection, and DPI bypass verification.
* **Dependencies**: Milestone 4.
* **Files & Modules Affected**:
  * `core/diagnostics/` (HTTP/TLS probe, DNS poison test, hop distance tracer, AV diagnostic report).
  * `apps/desktop/src/views/DiagnosticsView.tsx`.
* **Tasks**:
  1. Implement automated DNS poisoning detector (queries known poisoned domains and compares answers against secure DoH resolvers).
  2. Implement passive/active DPI probe (tests HTTP Host modification and SNI fragmentation against test endpoints).
  3. Implement ISP auto-detection (queries AS/ISP info via HTTPS endpoint to recommend optimal profile).
  4. Implement Security Software Interoperability Diagnostics (reports specific layer failures: driver load vs WFP filter vs TLS MITM).
* **Expected Output**: Diagnostic suite that pinpoints DPI blocking methods and recommends working profiles and compatible engines automatically.
* **Tests**:
  * Automated diagnostic test against simulated mock censorship server.
* **Completion Criteria**: Diagnostic probe accurately identifies blocked test endpoints and outputs structured JSON diagnostic report.

---

### Milestone 6: Installer, Clean Uninstaller & Release Pipeline
* **Objective**: Create professional Windows installer with atomic service registration, driver setup, and clean uninstallation.
* **Dependencies**: Milestone 5.
* **Files & Modules Affected**:
  * `installer/` (WiX Toolset / NSIS installer definitions).
  * `.github/workflows/release.yml`.
* **Tasks**:
  1. Author installer script packaging all binaries (`zondpi-desktop.exe`, `zondpi-service.exe`, `zondpi-engine-worker.exe`, `zondpi-engine-byedpi.exe`, `WinDivert.dll`, `WinDivert64.sys`).
  2. Implement atomic Windows Service registration and driver installation actions.
  3. Implement clean uninstaller that stops service, deletes driver registration, flushes DNS cache, and removes all leftover files.
  4. Configure GitHub Actions release workflow to compile, package, and generate `SHA256SUMS.txt`.
* **Expected Output**: Signed MSI/EXE installer delivering single-click install and 100% clean uninstall.
* **Tests**:
  * Silent install and uninstall validation in clean Windows VM / CI sandbox.
  * Verify no registry keys, driver entries, or locked files remain after uninstallation.
* **Completion Criteria**: One-click install functions cleanly; uninstallation leaves zero residual driver/service state.

---

### Milestone 7: Native Rust Packet Engine & Upstream Sync Subsystem
* **Objective**: Build pure native Rust packet engine and automated upstream synchronization tool.
* **Dependencies**: Milestone 6.
* **Files & Modules Affected**:
  * `engines/native/src/` (Native async packet pipeline, zero-copy TCP/TLS parsers).
  * `core/protocol-parsers/` (Expanded zero-copy parsers using `nom` / `smoltcp`).
  * `tools/upstream-sync/` (Automated tracker for `ValdikSS/GoodbyeDPI` and `hufrea/byedpi` commits).
* **Tasks**:
  1. Implement safe Rust wrapper around WinDivert 2.2 WFP handles.
  2. Build native packet modification pipeline (TCP segmentation, fake packet synthesis, auto-TTL calculation).
  3. Implement runtime strategy engine (allows applying different evasion techniques per domain/port).
  4. Add upstream sync CLI tool that checks for new commits/techniques in `ValdikSS/GoodbyeDPI` and `hufrea/byedpi`.
* **Expected Output**: High-performance native Rust packet engine running alongside or replacing C workers, with dynamic per-domain rule capabilities.
* **Tests**:
  * Benchmark throughput and CPU usage comparing Native Rust Engine vs C Engines.
  * Comprehensive packet fuzzing tests.
* **Completion Criteria**: Native Rust engine matches or exceeds C engine throughput while offering per-flow configuration and memory safety guarantees.
