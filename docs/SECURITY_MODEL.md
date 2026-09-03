# Security Model & Threat Mitigation Strategy

## 1. Security Philosophy & Antivirus Interoperability

ZonDPI is a **security-sensitive networking platform** because it interacts with the Windows Filtering Platform (WFP), manages DNS resolvers, and processes raw network traffic.

Our core security philosophy is founded on:
1. **Least Privilege**: The Desktop UI runs completely unprivileged. Elevated operations are strictly isolated to the background service.
2. **Strict Antivirus Coexistence**: We achieve compatibility with antivirus suites (Kaspersky, Defender, ESET) through **supported OS interfaces and clean architecture**, NOT evasion, hooking, process termination, or tampering.
3. **Zero Implicit Trust & Input Validation**: All IPC parameters from the GUI and local clients are treated as untrusted and validated against strict schemas.

---

## 2. Antivirus Interaction & Compatibility Constraints

ZonDPI adheres to strict non-invasive boundaries when interacting with third-party security software:

### 2.1 Prohibited Actions (Strictly Forbidden)
* ❌ **NO Process/Service Termination**: Never attempt to stop, suspend, or kill antivirus services (e.g. `avp.exe`, `MsMpEng.exe`).
* ❌ **NO Driver or Process Hiding**: Never use rootkit techniques, unhooking, or process hiding to conceal drivers or workers.
* ❌ **NO Binary Obfuscation**: Binaries are compiled cleanly from source without anti-analysis packers or malware-like obfuscation.
* ❌ **NO Registry Tampering**: Never modify security software registry entries directly.
* ❌ **NO DLL Injection / Hijacking**: Never plant arbitrary DLLs (e.g. `drover`, `version.dll`) into third-party application directories.
* ❌ **NO Silent Configuration Changes**: Never attempt to silently add exclusions via private/undocumented APIs.

### 2.2 Legitimate Detection & Compatibility Mechanisms
* ✅ **Read-Only Detection**: Enumerate installed antivirus products via standard Windows Management Instrumentation (WMI):
  ```
  Namespace: root\SecurityCenter2
  Query: SELECT * FROM AntiVirusProduct
  ```
* ✅ **Non-Destructive Health Checks**: Execute bounded test connections through the selected engine before activating system-wide routing. If packet reinjection fails, gracefully fall back to driver-free SOCKS5 mode (ByeDPI).
* ✅ **Documented User Instructions**: Present official vendor-approved configuration steps to the user if they choose to configure exclusions manually.

---

## 3. Privilege Separation & Trust Boundaries

```
┌────────────────────────────────────────────────────────┐
│             Standard User Session (Medium IL)          │
│                                                        │
│   ┌────────────────────────────────────────────────┐   │
│   │        Desktop GUI (Tauri / Webview)           │   │
│   │        - Unprivileged User Account             │   │
│   │        - Cannot alter system drivers or WFP    │   │
│   └───────────────────────┬────────────────────────┘   │
└───────────────────────────┼────────────────────────────┘
                            │
               TRUST BOUNDARY: Windows Named Pipe
               Security Descriptor: Authenticated Users
               Protocol: Typed Schema JSON-RPC
                            │
┌───────────────────────────▼────────────────────────────┐
│              SYSTEM / Elevated Service (High IL)       │
│                                                        │
│   ┌────────────────────────────────────────────────┐   │
│   │           Privileged Windows Service           │   │
│   │           - Validates all requests             │   │
│   │           - Detects Security Environment       │   │
│   │           - Enforces Job Object Limits         │   │
│   └───────────────┬────────────────┬───────────────┘   │
│                   │                │                   │
│   ┌───────────────▼────────┐  ┌────▼───────────────┐   │
│   │ GoodbyeDPI Worker      │  │ ByeDPI Worker      │   │
│   │ (WinDivert Engine)     │  │ (User SOCKS5 Engine│   │
│   └───────────────┬────────┘  └────────────────────┘   │
└───────────────────┼────────────────────────────────────┘
                    │
       KERNEL BOUNDARY: WFP Callout Driver
                    │
┌───────────────────▼────────────────────────────────────┐
│ WinDivert64.sys (Official Digitally Signed Driver)     │
└────────────────────────────────────────────────────────┘
```

---

## 4. Threat Modeling & Vulnerability Mitigations

### 4.1 Command & Argument Injection
* **Threat**: An attacker sends crafted strings over IPC to execute arbitrary binaries.
* **Mitigation**: Zero shell invocations (`cmd.exe` / `powershell.exe`). All IPC methods take strongly-typed enums and verified profile IDs.

### 4.2 DLL Preloading & PATH Hijacking
* **Threat**: Malicious DLLs loaded from current working directory or `PATH`.
* **Mitigation**: All binaries invoke `SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_SEARCH_APPLICATION_DIR)` and load DLLs via canonical absolute paths.

### 4.3 Named Pipe IPC Security
* **Threat**: Unauthorized local users or remote network attackers spoofing IPC commands.
* **Mitigation**: Named Pipe DACL allows only `Authenticated Users` (Read/Write) and `SYSTEM` / `Administrators` (Full Control). Rejects network connections.

### 4.4 Driver Loading & Integrity
* **Threat**: Loading tampered `.sys` driver files.
* **Mitigation**: Official upstream digitally signed `WinDivert64.sys` driver pinned with SHA-256 validation before service registration.

### 4.5 DNS Rollback & Fail-Safe Protection
* **Threat**: Stale DNS redirection on crash or reboot.
* **Mitigation**: Persistent DNS snapshot on disk; automatic rollback executed during service startup or crash recovery.

### 4.6 Process Isolation & Zero Orphan Guarantee via Windows Job Objects
* **Threat**: Orphaned background worker processes remaining active after daemon crash or service stop, causing port collisions and unmonitored traffic evasion.
* **Mitigation**: All worker processes are spawned directly into a dedicated Windows Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. The Windows kernel guarantees that if the service process terminates for any reason, all worker processes are killed immediately. ZonDPI strictly owns and manages only its child processes without blanket `taskkill` calls.
