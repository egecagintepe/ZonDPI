# Security Policy

## 1. Supported Versions

Security updates and critical vulnerability patches are applied to the following versions of ZonDPI:

| Version | Supported          | Notes |
| ------- | ------------------ | ----- |
| 1.0.5   | :white_check_mark: | Current stable release |
| < 1.0.5 | :x:                | Legacy / deprecated |

---

## 2. Reporting a Vulnerability

We take the security of ZonDPI seriously. Because ZonDPI operates with elevated privileges and manages network traffic, security is paramount.

### How to Report Privately
* **Preferred Method**: Submit a private report via **[GitHub Private Vulnerability Reporting](https://github.com/egecagintepe/ZonDPI/security/advisories/new)** on the repository's Security Advisories tab.
* **Do NOT** open a public GitHub Issue, discussion thread, or social media post for suspected vulnerabilities.

### What to Include
When reporting an issue, please include:
1. A clear description of the vulnerability and its potential impact.
2. The affected component (`zondpi-service.exe`, `zondpi-cli.exe`, IPC named pipe, GUI frontend, or installer).
3. Detailed step-by-step reproduction instructions or a minimal Proof-of-Concept (PoC).
4. Any relevant system information (Windows version, security software installed).

### Response Timeline
* **Initial Acknowledgment**: Maintainers aim to acknowledge receipt of private reports within 72 hours.
* **Status Updates**: You will receive updates as the issue is investigated, reproduced, and remediated.
* **Coordinated Disclosure**: We request that you maintain confidentiality until a patched release is published and users have had a reasonable window to upgrade.

---

## 3. Security Architecture & Threat Model Scope

### Elevated Privileges
* The ZonDPI Windows Background Service (`zondpi-service.exe`) runs with elevated system privileges (`LocalSystem` / Administrator) to manage network packet filters and adjust adapter DNS settings.
* The local IPC named pipe (`\\.\pipe\zondpi-ipc`) is secured via Windows Access Control Lists (ACLs) to ensure only authorized local processes can communicate with the service daemon.

### Kernel Driver (`WinDivert64.sys`) Scope
* The kernel-mode packet filtering driver (`WinDivert64.sys`) is an official, signed third-party binary produced by the [WinDivert project](https://github.com/basil00/WinDivert).
* If a potential vulnerability relates to the kernel driver implementation itself rather than ZonDPI's userspace integration, please also consider coordinating with the upstream WinDivert maintainers.
