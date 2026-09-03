# ZonDPI Troubleshooting & Diagnostic Guide

This guide helps resolve common operational and configuration issues encountered when using ZonDPI.

---

## 1. Quick Diagnostics

Always run the built-in diagnostic tool first to gather baseline information:

```powershell
# Open PowerShell as Administrator
.\zondpi-cli.exe diag
```

This command tests:
* SCM Service status (`RUNNING`, `STOPPED`)
* IPC Named Pipe connectivity
* WinDivert driver load status
* Active DNS configuration on network interfaces
* Outbound HTTP/HTTPS connectivity to reference endpoints

---

## 2. Common Issues & Solutions

### A. "Error: Failed to open WinDivert device (error 1275 / 5)"
* **Cause**: Driver Signature Enforcement (DSE) issue or lack of Administrator permissions.
* **Remedy**:
  1. Ensure you launch ZonDPI / PowerShell with **Administrator privileges** ("Run as administrator").
  2. Confirm `WinDivert64.sys` exists in the installation directory alongside `WinDivert.dll`.
  3. Verify that third-party antivirus is not blocking unsigned kernel driver loading.

### B. "Another application is already using WinDivert" (Error 32)
* **Cause**: Another program (such as standalone GoodbyeDPI, Clumsy, or another packet filter) is currently running and holding an exclusive handle on the WinDivert driver.
* **Remedy**:
  1. Stop other packet filter utilities or close instances of `goodbyedpi.exe`.
  2. Clean up old driver handles from an elevated PowerShell:
     ```powershell
     sc stop windivert
     sc delete windivert
     ```
  3. Restart ZonDPI: `.\zondpi-cli.exe restart`

### C. "Websites load slowly or Discord voice fails to connect"
* **Cause**: Packet fragmentation may interact poorly with certain ISP MTU limits or deep packet inspection equipment.
* **Remedy**:
  1. Verify DNS is resolving properly:
     ```powershell
     Resolve-DnsName discord.com -Server 1.1.1.1
     ```
  2. Test connection with curl:
     ```powershell
     curl.exe -v https://discord.com/
     ```
  3. If your ISP is Turkcell Superonline, select the `superonline_default` profile or try the local proxy mode (`byedpi_kaspersky_mode`).

### D. Windows SmartScreen Warning ("Windows protected your PC")
* **Cause**: ZonDPI release binaries are open-source and not signed with an expensive commercial Extended Validation (EV) code signing certificate.
* **Remedy**:
  1. Click **More info**.
  2. Click **Run anyway**.
  3. You can verify the integrity of the binary before running by checking its SHA-256 hash against the official `SHA256SUMS.txt` published on GitHub Releases:
     ```powershell
     Get-FileHash .\ZonDPI-1.0.4-Setup.exe -Algorithm SHA256
     ```

### E. DNS Did Not Revert After Crash or Force Kill
* **Cause**: System crashed or was hard-reset while custom DNS was applied.
* **Remedy**:
  1. ZonDPI automatically maintains a backup snapshot of your adapter DNS settings.
  2. To restore DHCP / original DNS settings manually:
     ```powershell
     Get-NetAdapter | Set-DnsClientServerAddress -ResetServerAddresses
     ```
