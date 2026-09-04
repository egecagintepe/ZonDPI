# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.0.5] - 2026-09-04

### Fixed
- **Windows Service Startup Persistence (HOTFIX)**:
  - Fixed Windows Service registration using `SERVICE_AUTO_START` (`ServiceStartType::AutoStart` / SCM start type 2) instead of manual on-demand start (`SERVICE_DEMAND_START` / 3).
  - Ensured ZonDPI service starts automatically upon Windows reboot and remains available in the background for IPC connections.
  - Added explicit SCM configuration command (`sc.exe config ZonDPI start= auto`) to NSIS post-install hooks to guarantee persistent automatic startup across all installation/reinstallation paths.
  - Handled upgrade/reinstallation idempotently by updating existing service configuration via Windows SCM `ChangeServiceConfig`.
  - Maintained architectural separation between background service startup (Automatic) and packet protection startup (controlled by user preference).
  - Added automated regression tests for service startup configuration constants.

## [1.0.4] - 2026-09-03

### Added
- **First Public Open-Source Release**: Initial public release of ZonDPI under the Apache-2.0 license.
- **Physical Validation on Turkish ISPs**: Production-validated default profile for Turkish ISP networks (specifically Türk Telekom / Superonline infrastructure), resolving SNI and TLS packet inspection on blocked platforms including Discord.
- **Windows Service Architecture**: Privileged Windows Background Service (`zondpi-service.exe`) with native Windows Service Control Manager (SCM) integration, named-pipe IPC, and automatic crash recovery.
- **Modern Desktop GUI**: Tauri v2 desktop application with live status indicators, system tray controls, and one-click start/stop controls.
- **Operator CLI**: Scriptable, multi-command CLI (`zondpi-cli.exe`) with human-friendly and JSON output modes (`status`, `start`, `stop`, `restart`, `health`, `diag`).
- **Cryptographic Driver Validation**: Built-in SHA-256 verification of the official signed WinDivert kernel driver (`verify_driver.ps1`) before service startup.
- **Security Software Compatibility Detection**: Read-only WMI security suite detection for Kaspersky, Bitdefender, ESET, and Sophos to prevent packet filtering conflicts.
- **DNS Recovery & Rollback**: Built-in DNS configuration rollback to protect network adapter configurations during unexpected shutdowns.
- **Automated CI & Packaging**: End-to-end Windows build pipeline (`build.ps1`), NSIS installer generation, portable zip generation, and automated GitHub Actions verification.

### Changed
- **Unified ZonDPI Engine Abstraction**: Completely separated user-facing product naming from underlying engine implementation details across CLI, GUI diagnostics, and IPC status models.
- **Strict Supply Chain Verification**: All userspace packet-manipulation binaries are compiled 100% from source; driver binaries are pinned and cryptographically attested.

### Security
- Zero remote telemetry, zero analytics collection, zero network tracking.
- IPC protected by local Windows named pipe Access Control Lists (ACLs).
