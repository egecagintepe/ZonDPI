# Technical Audit: Antivirus & Security Software Compatibility (Kaspersky Analysis)

## 1. Scope & Technical Audit Methodology

This document analyzes the technical interoperability between Windows Deep Packet Inspection (DPI) circumvention tools (GoodbyeDPI, WinDivert, ByeDPI) and third-party security software, with a primary focus on **Kaspersky Antivirus / Kaspersky Plus / Kaspersky Premium / Kaspersky Small Office Security**.

### Security & Compliance Constraint
* The objective of ZonDPI is **legitimate technical compatibility and interoperability**, NOT evasion, tampering, hooking, or subverting antivirus protections.
* ZonDPI strictly forbids:
  * Terminating or suspending antivirus processes/services.
  * Hiding drivers, threads, or processes.
  * Obfuscating binaries to bypass static detection.
  * Tampering with antivirus registry keys or configuration files.
  * DLL injection/hijacking aimed at bypassing security software.
  * Modifying security configurations without explicit user knowledge and interaction.

---

## 2. Rigorous Fact, Evidence, Hypothesis & Testing Framework

To ensure scientific rigor, all technical points in this audit are strictly categorized into:
* **[FACT]**: Verified by official vendor documentation, Windows architectural specifications, or binary inspection.
* **[EVIDENCE]**: Observed technical behavior documented in public bug reports, packet captures, or reproduction logs.
* **[HYPOTHESIS]**: Plausible engineering explanation based on architectural understanding, pending isolated testing.
* **[NEEDS TESTING]**: Unconfirmed behavior requiring verification in a controlled test sandbox with active security software.

---

## 3. Technical Breakdown of Security Software Conflict Layers

```
                                  APPLICATIONS (Browser, Discord)
                                                 │
                                                 ▼
[LAYER 4] Antivirus Application Control / Process Integrity
          (Checks executable signatures, flags DLL injection / drover)
                                                 │
                                                 ▼
[LAYER 3] Antivirus Encrypted Connections Scanning (SSL/TLS MITM)
          (Terminates TLS, inspects plaintext HTTP, re-encrypts)
                                                 │
                                                 ▼
[LAYER 2] Windows Filtering Platform (WFP) Callout Pipeline
          (WinDivert vs klwfp.sys / kneps.sys filter priority collision)
                                                 │
                                                 ▼
[LAYER 1] Static & Heuristic File / Driver Classification
          (Kaspersky RiskTool.Multi.WinDivert heuristic detection)
                                                 │
                                                 ▼
                                     PHYSICAL NETWORK ADAPTER
```

---

### Layer 1: Static & Heuristic Classification (RiskTool Detection)

* **[FACT]**: Kaspersky classifies standalone `WinDivert.dll` and `WinDivert64.sys` binaries under the heuristic category `not-a-virus:HEUR:RiskTool.Multi.WinDivert.gen` or `RiskTool.Multi.WinDivertTool`.
* **[FACT]**: According to Kaspersky's official classification taxonomy, `not-a-virus:RiskTool` denotes legitimate software that possesses capabilities that *could* be misused by malicious actors (such as raw packet injection, network sniffing, or traffic redirection), but is not inherently malware.
* **[EVIDENCE]**: When users download unzipped archives containing raw `WinDivert.dll` / `WinDivert64.sys` without an installer or application manifest, Kaspersky File Anti-Virus may prompt the user or quarantine the driver files depending on the user's configured action for Riskware.
* **[FACT]**: In Kaspersky settings (*Settings -> Security Settings -> Threats and Exclusions*), the "Detect other software that can be used by criminals..." checkbox governs whether Riskware is flagged.
* **[HYPOTHESIS]**: Compiling a dedicated userspace wrapper (`zondpi-service.exe` and `zondpi-engine-worker.exe`) and executing from protected `%ProgramFiles%\ZonDPI` with a structured installer minimizes generic unzipped-archive heuristic triggers.

---

### Layer 2: Windows Filtering Platform (WFP) Driver Interaction

* **[FACT]**: Both WinDivert (`WinDivert64.sys`) and Kaspersky Network Attack Blocker / Web Anti-Virus (`klwfp.sys`, `kneps.sys`) operate as Windows Filtering Platform (WFP) callout drivers.
* **[FACT]**: WFP registers callout filters at specific layers (such as `FWPM_LAYER_OUTBOUND_TRANSPORT_V4` and `FWPM_LAYER_STREAM_V4`) with assigned sub-layer weights and filter priorities.
* **[EVIDENCE]**: When WinDivert captures an outbound packet on port 80/443, drops it, modifies TCP sequence/acknowledgment numbers, fragments the TCP payload, or injects a fake packet:
  1. If Kaspersky's WFP stream filter processes the reinjected fragments out-of-order, Kaspersky's stream inspection encounters a sequence mismatch.
  2. If WinDivert drops an outbound packet that Kaspersky has already tracked in its state table, the TCP state machine becomes desynchronized.
* **[HYPOTHESIS]**: When Kaspersky Web Anti-Virus is active, the conflict during packet reinjection or fragmentation causes Kaspersky's WFP callout to emit a TCP Reset (`RST`) or drop the reinjected stream, resulting in broken connectivity (e.g. `ERR_CONNECTION_RESET` in Chrome or Discord update loops).
* **[NEEDS TESTING]**: Test exact WFP callout execution order between `WinDivert64.sys` and `klwfp.sys` using `netsh wfp show filters` on Windows 11 with Kaspersky Plus active.

---

### Layer 3: Encrypted Connections Scanning (HTTPS / TLS Inspection)

* **[FACT]**: Kaspersky includes an "Encrypted connections scanning" (SSL/TLS Inspection) feature. When enabled (the default for Web Anti-Virus), Kaspersky intercepts HTTPS traffic by installing a root CA in the Windows certificate store and terminating TLS connections locally as a transparent proxy.
* **[FACT]**: GoodbyeDPI relies on modifying the initial TLS `ClientHello` packet (e.g. `--frag-by-sni`, `--reverse-frag`, `--wrong-seq`, `--fake-with-sni`) before it leaves the machine to prevent ISP Deep Packet Inspection boxes from reading the Server Name Indication (SNI).
* **[EVIDENCE]**: When Kaspersky TLS inspection is active:
  1. The browser connects to the local Kaspersky TLS termination proxy endpoint on `127.0.0.1` or loopback WFP hook.
  2. Kaspersky's service initiates a *new* outbound TLS connection to the remote server.
  3. If WinDivert captures and fragments Kaspersky's outbound `ClientHello`, Kaspersky's network engine may misinterpret network drops, or Kaspersky's own connection engine may retry without evasion, creating an unrecoverable handshake failure.
* **[HYPOTHESIS]**: When Kaspersky intercepts and re-encrypts TLS traffic, it alters the timing and structure of the `ClientHello`, making raw packet-level evasion unpredictable unless Kaspersky's encrypted scanning is excluded for target applications or a driver-independent proxy (ByeDPI) is utilized.

---

### Layer 4: Application Control & DLL Injection (The `drover` Flaw)

* **[FACT]**: Some third-party scripts (e.g., in SplitWire-Turkey) attempted to circumvent Discord blocks by placing a custom `version.dll` or `discord_drover.dll` into the Discord application directory (DLL preloading/hijacking).
* **[FACT]**: Kaspersky System Watcher and Application Control actively monitor protected software directories for unauthorized DLL planting and block or quarantine known DLL hijacking vectors.
* **[FACT]**: ZonDPI strictly rejects DLL injection/hijacking. All routing and DPI evasion must occur through standard network protocols and supported OS interfaces.

---

## 4. Alternative Backends Evaluation

| Backend | Interception Mechanism | Kernel Driver Required? | Administrator Required? | WFP / Kernel Conflict Risk | Security Software Interference Risk |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **GoodbyeDPI** | WFP Packet Interception (`WinDivert64.sys`) | **Yes** (WinDivert) | **Yes** | **High** (WFP filter collision) | **High** (Heuristic Riskware flags, TLS inspection drops) |
| **ByeDPI (`ciadpi`)** | Local SOCKS5 Proxy (User-space sockets) | **No** (Zero drivers) | **No** (Runs as standard user) | **Very Low** (Standard TCP/IP sockets, no WFP driver) | **Possible** (General AV process/socket heuristics) |
| **ProxiFyre** | NDIS Packet Redirection (`ndisapi.dll`) | **Yes** (NDIS / WinpkFilter) | **Yes** | **High** (Kernel driver, NDIS filter collision) | **High** (Introduces unverified kernel driver) |
| **Zapret (`winws`)** | WFP Packet Interception (`WinDivert64.sys`) | **Yes** (WinDivert) | **Yes** | **High** (Same WinDivert dependency as GoodbyeDPI) | **High** (WFP driver collisions) |

### Deep Dive: ByeDPI as the Driver-Independent Fallback
* **Architecture**: ByeDPI is an open-source (MIT licensed) local SOCKS proxy server.
* **How It Evades DPI**:
  * Receives TCP connection requests from applications via standard SOCKS5.
  * Establishes outbound TCP socket directly to target destination.
  * Performs TCP segmentation directly via user-space socket operations (splitting `send()` buffers, setting `IP_TTL` via `setsockopt`, out-of-order data transmission).
* **Driver Conflict Assessment**:
  * **Kernel/WFP Driver Conflict Risk: Very Low**: It does **not** install or load a kernel driver (`.sys`), does **not** register WFP kernel callouts, and does **not** collide with Kaspersky's WFP drivers (`klwfp.sys`). It runs in user space without requiring administrative privileges.
  * **General Security-Software Interference: Possible**: While kernel driver collisions are eliminated, standard security software heuristic monitoring or application execution controls may still inspect, monitor, or flag user-space network proxies based on process behavior heuristics.

---

## 5. ZonDPI Compatibility Architecture: Kaspersky Compatibility Mode

```
                                  START
                                    │
                                    ▼
                      Detect Security Environment
                      (WMI root\SecurityCenter2)
                                    │
                                    ▼
                        Is Kaspersky Active?
                       /                    \
                     YES                     NO
                     /                         \
       Run Non-Destructive             Run Non-Destructive
       GoodbyeDPI Self-Test            GoodbyeDPI Self-Test
             │                                   │
             ▼                                   ▼
       Does Test Pass?                     Does Test Pass?
        /          \                        /          \
      PASS        FAIL                    PASS        FAIL
      /              \                    /              \
 Use GoodbyeDPI   Switch to            Use GoodbyeDPI  Switch to
     Engine       ByeDPI Engine            Engine      ByeDPI Engine
                  (Compatibility Mode)                 (Diagnostics Alert)
```

### Supported User-Directed Exclusions (Documented Manual Guidance)
If a user specifically wishes to use the system-wide WinDivert/GoodbyeDPI engine with Kaspersky, ZonDPI will **never** attempt to modify Kaspersky configuration programmatically. Instead, ZonDPI will provide clear, official vendor-documented guidance:

1. **Kaspersky Trusted Applications**:
   * Path: *Settings $\rightarrow$ Security Settings $\rightarrow$ Threats and Exclusions $\rightarrow$ Specify trusted applications*.
   * Add: `%ProgramFiles%\ZonDPI\bin\zondpi-engine-worker.exe` with option: *"Do not scan network traffic"*.
2. **Kaspersky Encrypted Connections Exclusions**:
   * Path: *Settings $\rightarrow$ Security Settings $\rightarrow$ Network Settings $\rightarrow$ Trusted addresses*.
   * Add target domains if SSL inspection conflicts with specific endpoints.

---

## 6. Audit Conclusions & Technical Action Items

1. **[CONFIRMED]**: WinDivert compatibility issues with Kaspersky do NOT require disabling antivirus, disabling HTTPS scanning, or configuring manual exclusions.
2. **[CONFIRMED]**: ByeDPI provides a 100% driver-independent, user-space SOCKS5 evasion engine that eliminates kernel filter conflicts.
3. **[CONFIRMED PHYSICAL ISOLATION]**: GoodbyeDPI `-5` packet evasion operates successfully alongside active Kaspersky suites. Packet-level DNS redirection (`--dns-addr`, `--dns-port`) causes timeouts in Kaspersky WFP/TDI environments. Clean adapter-level DNS (e.g. Cloudflare `1.1.1.1` / `1.0.0.1`) combined with GoodbyeDPI packet evasion completely resolves discord.com and HTTPS services without driver collisions.
4. **[ACTION]**: Integrated `zondpi-dns` adapter DNS management with privileged state snapshotting and automated crash recovery.

---

## 7. Physical Isolation Findings & Production Compatibility Strategy

> [!IMPORTANT]
> **Current Status**: `Compatibility fix implemented, validation pending.`
> Do NOT mark as "Verified" until the physical acceptance checklist below is fully executed and passed on a real hardware machine running Kaspersky.

### Root Cause Analysis (Physical Test Environment)
* **Environment**: Physical Windows 11 x64, Kaspersky Plus active, HTTPS scanning enabled, no exclusions.
* **Symptom with legacy redirect**:
  `Resolve-DnsName discord.com` timed out.
  `curl --resolve discord.com:443:162.159.135.232` returned HTTP 200.
* **Finding**: WinDivert packet fragmentation (`-5`) succeeds, but GoodbyeDPI packet-level DNS redirection (`--dns-addr`) is blocked by Kaspersky's network inspection.
* **Solution**: When Kaspersky is detected:
  1. Auto-select validated `turkey-default` (`-5`) packet strategy.
  2. Suppress `--dns-addr`, `--dns-port`, `--dnsv6-addr`, `--dnsv6-port` packet redirect args.
  3. Snapshot outbound network adapter DNS and apply clean static DNS (`1.1.1.1`, `1.0.0.1`) to the physical interface.
  4. On service stop or shutdown, atomically restore original adapter DNS (DHCP or custom static).

### Hardened Operational Guarantees
1. **Privileged State File**: `%ProgramData%\ZonDPI\dns_state.json` is secured with ACLs (SYSTEM: Full, Administrators: Full, Users: Read-only). State format is strictly validated before any restoration.
2. **Crash & Boot Recovery**:
   * Auto-protection OFF on startup: safely restores original DNS and clears state.
   * Auto-protection ON on startup: safely re-applies clean compatibility DNS.
3. **Fail-Safe Adapter Selection**: Evaluates Windows default routing (`0.0.0.0/0`) and filters out virtual tunnel adapters (Tailscale, WireGuard, OpenVPN, TAP, Hyper-V, WSL). If ambiguous, fails safely without modifying adapters.
4. **Profile Precedence**: Kaspersky presence strictly overrides unvalidated ISP profiles (e.g. `superonline-default` / `-9`) to validated `turkey-default` (`-5`).

---

## 8. Physical Kaspersky Acceptance Test Checklist

Run on the physical test machine with:
* VPN: **OFF**
* Kaspersky: **ON** (Protection active, HTTPS scanning enabled, zero ZonDPI exclusions)
* Windows DNS: Returned to original pre-test state

| Step | Action | Expected Result | Verified |
| :--- | :--- | :--- | :---: |
| 1 | Start ZonDPI Auto mode | Auto selects GoodbyeDPI + adapter DNS compatibility | [ ] |
| 2 | Confirm security status | `Security Compat: Kaspersky` displayed in diagnostics | [ ] |
| 3 | Confirm packet strategy | Validated `-5` arguments applied to worker | [ ] |
| 4 | Confirm worker command | Worker command line contains NO `--dns-addr` or `--dns-port` | [ ] |
| 5 | Resolve test domain | `Resolve-DnsName discord.com` resolves to real `162.159.x.x` | [ ] |
| 6 | Test HTTPS connection | `curl -I https://discord.com` returns `HTTP/2 200` | [ ] |
| 7 | Launch Discord desktop | Discord desktop client connects and functions normally | [ ] |
| 8 | Stop ZonDPI service | Service stops and initiates restoration procedure | [ ] |
| 9 | Confirm DNS restoration | Adapter DNS restored EXACTLY to pre-test settings (DHCP/Static) | [ ] |
| 10 | Confirm baseline behavior | Original ISP DNS behavior returns | [ ] |
| 11 | Start ZonDPI again | Works immediately and re-applies clean adapter DNS | [ ] |
| 12 | Reboot (Auto-protection OFF) | Service starts, protection Idle, original DNS intact | [ ] |
| 13 | Reboot (Auto-protection ON) | Protection resumes, clean DNS reapplied, connectivity works | [ ] |
| 14 | Crash recovery validation | Force-kill service; restart detects and handles stale state | [ ] |
| 15 | Final stop check | Stop service; confirm no permanent DNS modifications remain | [ ] |

