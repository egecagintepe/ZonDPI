# ZonDPI

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows_10_%2F_11_x64-0078D6.svg)](docs/COMPATIBILITY.md)
[![Release](https://img.shields.io/badge/Release-v1.0.4-success.svg)](https://github.com/egecagintepe/ZonDPI/releases/latest)
[![CI](https://github.com/egecagintepe/ZonDPI/actions/workflows/ci.yml/badge.svg)](https://github.com/egecagintepe/ZonDPI/actions/workflows/ci.yml)
[![Zero Telemetry](https://img.shields.io/badge/Telemetry-Zero-brightgreen.svg)](docs/PRIVACY.md)

**English** | [Türkçe](README.tr.md)

Open-source Windows system utility engineered to mitigate Deep Packet Inspection (DPI) interference, TLS ClientHello SNI blocking, and DNS poisoning on Turkish ISP networks.

---

> [!NOTE]
> **What ZonDPI is NOT**: ZonDPI is **not** a VPN, proxy subscription, or anonymity network. It does **not** route your internet traffic through external third-party servers, nor does it hide your public IP address. Instead, it runs entirely locally on your Windows PC to fragment packets and reorder TCP/TLS handshakes, preventing intermediate ISP inspection equipment from intercepting connections to legitimate services.

---

## Key Features

* **Real-World Validated on Turkish ISPs**: Default profiles are physically tested and proven on production Turkish ISP infrastructure (Türk Telekom, Turkcell Superonline), restoring access to platforms such as Discord with native low ping.
* **Dual Operation Modes**:
  * **System Packet Filter (Default)**: Kernel-level transparent TCP segmentation and auto-TTL hop evasion via signed WinDivert driver. Works system-wide across all browsers, games, and desktop applications without manual proxy configuration.
  * **Antivirus Compatibility Mode**: Local loopback proxy designed to eliminate packet inspection conflicts on machines running security suites like Kaspersky, Bitdefender, or ESET.
* **Windows Background Service Architecture**: Unprivileged desktop GUI connects to a privileged Windows Service (`zondpi-service.exe`) running via Windows Service Control Manager (SCM), providing automatic crash recovery and clean startup handling.
* **DNS Poisoning Mitigation & Auto-Rollback**: Protects network adapters against DNS poisoning via Cloudflare 1.1.1.1 DNS over standard ports, automatically taking snapshots and rolling back adapter DNS on service shutdown.
* **Zero Telemetry & Absolute Privacy**: No analytics, no metrics collection, no crash reporting, and no outbound network calls to ZonDPI servers.
* **Modern Desktop GUI & Operator CLI**: Includes a lightweight desktop GUI (Tauri v2) with system tray controls, and a scriptable command-line interface (`zondpi-cli.exe`) for automation and headless diagnostics.

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────┐
│               ZonDPI Desktop GUI (Tauri v2)            │
│            or Scriptable Operator CLI (zondpi-cli)     │
└───────────────────────────┬────────────────────────────┘
                            │ Local Named Pipe IPC
                            ▼
┌────────────────────────────────────────────────────────┐
│            ZonDPI Windows Service Daemon               │
│        (SCM Managed • Auto-Recovery • Health Checks)   │
├───────────────────────────┬────────────────────────────┤
│   Packet Filter Engine    │  Compatibility Engine      │
│   (WinDivert Driver •     │  (WMI Antivirus Detector • │
│    TCP/TLS Fragmentation) │   Local SOCKS5 Loopback)   │
└───────────────────────────┴────────────────────────────┘
```

---

## Quick Start

### 1. Download
Download the latest verified release from the [Releases](https://github.com/egecagintepe/ZonDPI/releases/latest) page:
* **Installer**: `ZonDPI-1.0.4-Setup.exe` (Recommended)
* **Portable**: `ZonDPI-1.0.4-Windows-x64-portable.zip`

### 2. Installation & SmartScreen Notice
1. Run `ZonDPI-1.0.4-Setup.exe` and accept the Windows UAC elevation prompt.
2. *Note on Windows SmartScreen*: Because ZonDPI is a free open-source project without an expensive commercial code-signing certificate, Windows SmartScreen may display an unrecognized app alert. Click **"More info"** and **"Run anyway"**. You can independently verify the cryptographic SHA-256 checksum of your downloaded binary against the published [SHA256SUMS.txt](https://github.com/egecagintepe/ZonDPI/releases/latest).

### 3. Usage
1. Launch **ZonDPI** from your Start Menu or system tray.
2. Click **Start Protection**.
3. All connections are immediately protected. No browser restart or network reconfiguration is required.

---

## Command-Line Interface (CLI)

ZonDPI includes `zondpi-cli.exe` for operator diagnostics and headless control:

```powershell
# Check service and engine status
.\zondpi-cli.exe status

# Run comprehensive end-to-end network diagnostics
.\zondpi-cli.exe diag

# Start / stop / restart the protection service
.\zondpi-cli.exe start
.\zondpi-cli.exe stop
.\zondpi-cli.exe restart
```

---

## Verification & Cryptographic Hashes

Every release publishes verifiable SHA-256 hashes in `SHA256SUMS.txt`:

```powershell
# Verify downloaded installer integrity
Get-FileHash .\ZonDPI-1.0.4-Setup.exe -Algorithm SHA256
```

Expected hash for v1.0.4:
* `ZonDPI-1.0.4-Setup.exe`: `A3A537298792B0507EE5C854BC301C35B77533F5932511AB70C633BD2C186FD2`
* `ZonDPI-1.0.4-Windows-x64-portable.zip`: `55A90FCFB485D93B4938679A4AF03E3658328EAB2BB495C72B98BBF51C685036`

---

## Documentation

* [Hardware & ISP Compatibility Matrix](docs/COMPATIBILITY.md)
* [Troubleshooting & Diagnostic Guide](docs/TROUBLESHOOTING.md)
* [Architecture Specification](docs/ARCHITECTURE.md)
* [Building from Source](docs/DEVELOPMENT.md)
* [Dependency & Supply Chain Trust Manifest](DEPENDENCIES.md)
* [Privacy Architecture](docs/PRIVACY.md)
* [Security Model](docs/SECURITY_MODEL.md)

---

## Third-Party Open-Source Attribution

ZonDPI builds upon and acknowledges the foundational work of the open-source community:

* **[GoodbyeDPI](https://github.com/ValdikSS/GoodbyeDPI)** by ValdikSS (Apache License 2.0)
* **[ByeDPI](https://github.com/hufrea/byedpi)** by hufrea (MIT License)
* **[WinDivert](https://github.com/basil00/WinDivert)** by basil00 (LGPL-3.0 / GPL-2.0)
* **[uthash](https://github.com/troydhanson/uthash)** by Troy D. Hanson (BSD 1-Clause)

All upstream licenses and redistribution requirements are maintained. For full legal texts and exact commit provenance, see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

---

## License

ZonDPI is licensed under the [Apache License, Version 2.0](LICENSE).
