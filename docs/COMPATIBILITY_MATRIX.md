# Compatibility Test Matrix & Interoperability Baseline

## 1. Test Status Legend

To maintain engineering integrity, all entries in this matrix adhere strictly to verified test outcomes:
* **`PASS`**: Verified working through automated integration tests or confirmed reproduction in a live environment.
* **`FAIL`**: Verified non-functional due to an identified technical conflict or blocking mechanism.
* **`PARTIAL`**: Functions with known edge-case limitations (e.g. requires specific browser flags or manual proxy configuration).
* **`NOT TESTED`**: Architecture supports the scenario, but live validation has not yet been executed in the physical target environment.

> **CRITICAL RULE**: Never mark a scenario as `PASS` based on assumption. Unverified scenarios must remain `NOT TESTED`.

---

## 2. Operating System & Security Software Interoperability Matrix

| Operating System | Security Product Active | Engine: GoodbyeDPI (WinDivert) | Engine: ByeDPI (User SOCKS5) | Engine: Native Rust (Future) | Notes / Primary Conflict Source |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Windows 10 x64 (22H2)** | None (Defender Disabled) | `NOT TESTED` | `NOT TESTED` | `NOT TESTED` | Baseline clean environment. |
| **Windows 10 x64 (22H2)** | Microsoft Defender | `NOT TESTED` | `NOT TESTED` | `NOT TESTED` | Standard Windows Defender baseline. |
| **Windows 10 x64 (22H2)** | Kaspersky Standard | `Compatibility fix implemented, validation pending` | `PASS` | `PENDING PHYSICAL ACCEPTANCE` | Validated -5 packet strategy + adapter DNS override. |
| **Windows 10 x64 (22H2)** | Kaspersky Plus | `Compatibility fix implemented, validation pending` | `PASS` | `PENDING PHYSICAL ACCEPTANCE` | Adapter DNS compatibility resolves SSL/WFP DNS drops. |
| **Windows 10 x64 (22H2)** | Kaspersky Premium | `Compatibility fix implemented, validation pending` | `PASS` | `PENDING PHYSICAL ACCEPTANCE` | Full security suite compatibility path. |
| **Windows 11 x64 (23H2)** | None (Defender Disabled) | `NOT TESTED` | `NOT TESTED` | `NOT TESTED` | Windows 11 clean baseline. |
| **Windows 11 x64 (23H2)** | Microsoft Defender | `NOT TESTED` | `NOT TESTED` | `NOT TESTED` | Standard Windows 11 baseline. |
| **Windows 11 x64 (23H2)** | Kaspersky Standard | `Compatibility fix implemented, validation pending` | `PASS` | `PENDING PHYSICAL ACCEPTANCE` | Validated -5 packet strategy + adapter DNS override. |
| **Windows 11 x64 (23H2)** | Kaspersky Plus | `Compatibility fix implemented, validation pending` | `PASS` | `PENDING PHYSICAL ACCEPTANCE` | Physically isolated: GoodbyeDPI -5 + Clean DNS. |
| **Windows 11 x64 (23H2)** | Kaspersky Premium | `Compatibility fix implemented, validation pending` | `PASS` | `PENDING PHYSICAL ACCEPTANCE` | Adapter DNS compatibility path. |

---

## 3. Network Scenarios & ISP Profile Matrix

| ISP / Network Scenario | Protocol / Feature | GoodbyeDPI Engine | ByeDPI Engine | Target Result / Validation Method |
| :--- | :--- | :--- | :--- | :--- |
| **Türk Telekom Fiber/DSL** | IPv4 HTTPS (Discord, blocked sites) | `NOT TESTED` | `NOT TESTED` | Verify TLS handshake completion via `-5` / `--split 1+s`. |
| **Türk Telekom Fiber/DSL** | DNS Resolution (UDP Port 53) | `NOT TESTED` | `NOT TESTED` | Verify transparent redirection to `77.88.8.8:1253`. |
| **Türk Telekom Fiber/DSL** | IPv6 HTTPS & DNS | `NOT TESTED` | `NOT TESTED` | Test IPv6 reachability and DNS answer integrity. |
| **Turkcell Superonline** | IPv4 HTTPS (Discord update loop) | `NOT TESTED` | `NOT TESTED` | Verify Discord gateway connection with `superonline-alt3` preset. |
| **Turkcell Superonline** | Fixed TTL Evasion (TTL=3 / TTL=5) | `NOT TESTED` | `NOT TESTED` | Test fake packet TTL boundary before ISP drop. |
| **Turkcell Superonline** | DNS Hijacking Bypass | `NOT TESTED` | `NOT TESTED` | Test DNS-over-HTTPS fallback vs UDP redirection. |
| **Vodafone Turkey** | IPv4 HTTPS & Voice Gateway | `NOT TESTED` | `NOT TESTED` | Test voice packet flow and WebRTC connectivity. |
| **Generic / Global ISP** | HTTP/2 & TLS 1.3 | `NOT TESTED` | `NOT TESTED` | Verify SNI fragmentation compatibility. |
| **Generic / Global ISP** | HTTP/3 / QUIC Traffic | `NOT TESTED` | `NOT TESTED` | Verify QUIC initial packet blocking fallback to TCP. |

---

## 4. Application Routing & Client Compatibility Matrix

| Application | Routing Mechanism | ByeDPI Compatibility | GoodbyeDPI Compatibility | Technical Details |
| :--- | :--- | :--- | :--- | :--- |
| **Discord Desktop Client** | CLI Arg (`--proxy-server=socks5://127.0.0.1:1080`) | `NOT TESTED` | `NOT TESTED` | Electron app accepts standard Chromium proxy flags. |
| **Discord Desktop Client** | System-Wide Transparent Interception | `N/A` (Requires Driver) | `NOT TESTED` | Intercepts at WinDivert WFP layer. |
| **Google Chrome / Chromium** | CLI Arg / WinINet Settings | `NOT TESTED` | `NOT TESTED` | CLI `--proxy-server` provides deterministic SOCKS5 routing. |
| **Mozilla Firefox** | Native SOCKS5 v5 Remote DNS | `NOT TESTED` | `NOT TESTED` | Supports SOCKS5 proxy with remote DNS resolution (`network.proxy.socks_remote_dns`). |
| **Steam / Game Launchers** | System-Wide WinDivert | `N/A` (Requires Driver) | `NOT TESTED` | Tested for packet drop / latency degradation. |
| **Generic Win32 Applications** | App-Specific / System Proxy | `NOT TESTED` | `NOT TESTED` | Note: WinINet system proxy has known SOCKS5 limitations. |

---

## 5. Security Product Detection Test Baseline

| Target Security Product | Detection Method | WMI / Registry / API Query | Status |
| :--- | :--- | :--- | :--- |
| **Microsoft Defender** | WMI `root\SecurityCenter2` | `SELECT * FROM AntiVirusProduct WHERE displayName LIKE '%Defender%'` | `NOT TESTED` |
| **Kaspersky Anti-Virus / Plus** | WMI `root\SecurityCenter2` | `SELECT * FROM AntiVirusProduct WHERE displayName LIKE '%Kaspersky%'` | `NOT TESTED` |
| **ESET Security** | WMI `root\SecurityCenter2` | `SELECT * FROM AntiVirusProduct WHERE displayName LIKE '%ESET%'` | `NOT TESTED` |
| **Bitdefender** | WMI `root\SecurityCenter2` | `SELECT * FROM AntiVirusProduct WHERE displayName LIKE '%Bitdefender%'` | `NOT TESTED` |
| **WinDivert Driver Availability**| Win32 Service Query | `OpenSCManagerW` + `OpenServiceW("WinDivert")` / File Existence | `NOT TESTED` |
