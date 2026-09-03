# Contributing to ZonDPI

Thank you for your interest in contributing to ZonDPI! We welcome contributions from the community.

Please take a moment to review this document to ensure smooth collaboration.

---

## 1. Code of Conduct

By participating in this project, you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md). Please report unacceptable behavior through GitHub Security Advisories or by contacting the repository maintainers.

---

## 2. Core Project Principles

Any proposed contribution must adhere to our core project tenets:

1. **Strict Zero-Telemetry Policy**: ZonDPI collects **zero** metrics, telemetry, crash reports, or user analytics. We will not accept PRs introducing remote logging, tracking, or phone-home mechanisms.
2. **Evidence-Based Network Changes**: We do **not** accept speculative network parameter tweaks or unverified DPI preset PRs. Any PR modifying `profiles/turkey/*.json` or engine packet parameters **MUST** include documented physical test evidence on real Turkish hardware (ISP name, city, `curl -v` outputs, DNS resolution logs, and before/after latency measurements).
3. **Upstream License Integrity**: All third-party copyright notices, licenses, and attribution files must be strictly preserved. We do not re-license upstream code.
4. **Reproducible & Clean Architecture**: All userspace code must build cleanly from source via `build.ps1` with standard open-source toolchains (Rust stable, MinGW-w64, GNU Make).

---

## 3. Development Workflow

### Prerequisites
* **Windows 10 / 11 (64-bit)**
* **Rust Toolchain**: Stable (1.80.0+) with `clippy` and `rustfmt`
* **C Toolchain**: MinGW-w64 (`gcc`) and GNU Make (`make`)
* **Node.js**: v20+ with `npm` (for GUI frontend)
* **PowerShell**: 5.1 or 7+

### Step-by-Step Contribution Guide
1. **Fork the repository** on GitHub.
2. **Clone your fork**:
   ```bash
   git clone https://github.com/<your-username>/ZonDPI.git
   cd ZonDPI
   ```
3. **Create a topic branch**:
   ```bash
   git checkout -b feature/my-enhancement
   # or
   git checkout -b fix/issue-description
   ```
4. **Develop and test locally**:
   ```powershell
   # Run Rust tests
   cargo test --workspace

   # Check formatting
   cargo fmt -- --check

   # Run clippy with strict warnings
   cargo clippy --workspace --all-targets -- -D warnings

   # Build frontend
   cd app/gui
   npm ci
   npm run typecheck
   npm run lint
   cd ../..
   ```
5. **Run full packaging build**:
   ```powershell
   powershell -ExecutionPolicy Bypass -File .\build.ps1 -Configuration Release
   ```
6. **Commit with clean, conventional messages**:
   * `feat: add IPv6 fallback probe in diagnostics`
   * `fix: handle edge case in adapter DNS rollback`
   * `docs: update troubleshooting guide for Kaspersky`
7. **Push to your fork and submit a Pull Request** against the `main` branch.

---

## 4. Reporting Security Issues

**DO NOT report security vulnerabilities through public GitHub Issues.**

Please consult our [Security Policy](SECURITY.md) and report security concerns privately using GitHub's **Private Vulnerability Reporting** feature.
