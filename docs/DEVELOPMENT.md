# Developer Guide & Local Workflow

This guide details how to set up your local development environment, build from source, run automated tests, and debug ZonDPI.

---

## 1. System Requirements

* **Windows 10 / 11 (64-bit)**
* **Rust**: `stable-x86_64-pc-windows-msvc` (1.80.0 or newer)
  ```powershell
  rustup default stable
  rustup component add clippy rustfmt
  ```
* **C Toolchain**: MinGW-w64 (`x86_64-w64-mingw32-gcc`) and GNU Make (`make`)
  * Available via WinLibs or MSYS2: ensure `gcc.exe` and `make.exe` (or `mingw32-make.exe`) are in your system `PATH`.
* **Node.js**: v20+ with npm (for Tauri desktop GUI)
* **NSIS**: (Optional, required only for packaging installer) NSIS 3.0+

---

## 2. Directory Structure

```
zondpi/
├── app/gui/                  # Tauri v2 Desktop GUI (React + TypeScript)
├── core/                     # Reusable Rust Crates
│   ├── compatibility/        # Security suite detection & recommendation logic
│   ├── diagnostics/          # Network probing & DNS diagnostic tools
│   ├── dns/                  # Windows adapter DNS management & rollback
│   ├── flow-tracker/         # Connection tracking & TTL calculation
│   ├── ipc-protocol/         # Shared IPC protocol types & serialization
│   ├── packet-engine/        # Core engine abstractions & trait definitions
│   ├── profile-engine/       # JSON profile validator & compiler
│   └── protocol-parsers/     # TLS ClientHello SNI & HTTP zero-copy parsers
├── engines/                  # Upstream Engine Wrappers & C Vendored Trees
│   ├── goodbye/              # GoodbyeDPI wrapper + upstream C source
│   └── byedpi/               # ByeDPI wrapper + upstream C source
├── profiles/                 # ISP preset configurations (turkey/*.json)
├── service/windows-service/  # Privileged Windows Background Service
├── third_party/windivert/    # Pinned WinDivert DLL, sys driver, & headers
├── tools/zondpi-cli/         # Scriptable operator CLI
└── build.ps1                 # Master build & packaging PowerShell script
```

---

## 3. Daily Development Commands

### Building and Testing Core Crates
```powershell
# Format checking
cargo fmt --all -- --check

# Strict Clippy lint check
cargo clippy --workspace --all-targets -- -D warnings

# Execute all workspace unit and integration tests
cargo test --workspace
```

### Building the Desktop GUI Frontend
```powershell
cd app/gui
npm ci
npm run typecheck
npm run lint
npm run build
cd ../..
```

### Full Local Build & Package Pipeline
Run the master PowerShell build script:
```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1 -Configuration Release
```
This performs:
1. Cryptographic validation of `WinDivert.dll` and `WinDivert64.sys` hashes.
2. Native C compilation of GoodbyeDPI and ByeDPI workers using MinGW-w64.
3. Cargo workspace compilation of the Windows Service, CLI, and core libraries.
4. Frontend build and NSIS installer packaging (if NSIS is installed).
5. Output assembly into `dist/` and `release/` directories with `SHA256SUMS.txt`.
