# ZonDPI Compatibility Matrix & Security Software Guidance

This document details verified hardware/ISP configurations, antivirus software interactions, and known limitations.

---

## 1. Operating System Compatibility

| Operating System | Architecture | Compatibility Status | Notes |
| :--- | :--- | :--- | :--- |
| **Windows 11** | x86_64 (x64) | :white_check_mark: **Verified** | Standard environment; requires Administrator for service installation. |
| **Windows 10 (1809+)** | x86_64 (x64) | :white_check_mark: **Supported** | Standard x64 builds supported. |
| **Windows on ARM64** | ARM64 | :x: **Unsupported** | WinDivert kernel driver is x64 native; ARM64 kernel drivers are not provided. |
| **Windows 7 / 8 / 8.1**| x86_64 | :x: **Unsupported** | Modern Windows Service and Tauri runtime require Windows 10+. |
| **Linux / macOS** | Any | :x: **Unsupported** | Relies on the Windows Filtering Platform (WFP) kernel driver. |

---

## 2. Real-World ISP Verification Status (Türkiye)

| ISP / Network | Protocol / Profile | Physical Status | Validation Details |
| :--- | :--- | :--- | :--- |
| **Türk Telekom** (Fiber / VDSL) | Default Profile (`-f 2 -e 2 --native-frag --reverse-frag --auto-ttl 1-4-10 --max-payload 1200` + Cloudflare DNS) | :white_check_mark: **Verified Working** | Discord web (`HTTP 200`, ~0.41s) and Discord desktop client voice/media fully operational on production Turkish hardware. |
| **Turkcell Superonline** | Default / Auto Mode | :warning: **In Progress / Pending** | Preliminary reports working; community verification in progress. |
| **Vodafone Net** | Default / Auto Mode | :warning: **Pending Community Testing** | Expected compatible with default mode; awaiting structured user logs. |
| **TurkNet** | Default / Direct Mode | :white_check_mark: **Compatible** | TurkNet uses standard non-intrusive DNS routing; runs without packet desynchronization issues. |

---

## 3. Antivirus & Security Software Compatibility

ZonDPI includes a built-in security product detection module that inspects WMI on startup to identify installed security software.

| Security Product | Known Behavior | Recommended Configuration | Status |
| :--- | :--- | :--- | :--- |
| **Windows Defender** | Fully compatible. Does not interfere with WinDivert packet filter or loopback traffic. | Default settings. | :white_check_mark: Verified |
| **Kaspersky (Premium / Plus / Standard)** | Kaspersky's Network Attack Blocker & SSL/TLS Inspection may drop out-of-order or reverse-fragmented packets produced by raw packet filters. | Switch to **Kaspersky Compatibility Profile** in ZonDPI settings (uses SOCKS5 local proxy mode to bypass kernel packet filter conflicts). | :warning: Implementation complete; pending wide field verification |
| **Bitdefender Total Security** | "Encrypted Web Scan" may alert on fragmented TLS ClientHello packets. | Add an exclusion for `zondpi-service.exe` and `goodbyedpi.exe` in Advanced Threat Defense. | :warning: Preliminary |
| **ESET Internet Security** | "Protocol Filtering" can occasionally reset fragmented TCP handshakes. | Exclude ZonDPI binaries from SSL/TLS protocol filtering. | :warning: Preliminary |
| **Cloudflare WARP / WireGuard / OpenVPN** | Running a full-tunnel VPN simultaneously with ZonDPI is redundant. VPN tunnels capture all traffic at the virtual NIC level before WFP inspection. | Disable VPN while running ZonDPI, or use ZonDPI independently for unthrottled local ISP speeds. | :information_source: Expected behavior |

---

## 4. Submitting Compatibility Reports

If you test ZonDPI on an ISP or antivirus configuration not listed above, please submit a report using the [Compatibility Report Issue Template](https://github.com/egecagintepe/ZonDPI/issues/new?template=compatibility_report.yml).
