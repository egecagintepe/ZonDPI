# Release Engineering & Publication Guide

This document defines the release engineering procedure for ZonDPI maintainers.

---

## 1. Release Invariants

Every production release of ZonDPI must satisfy the following criteria:

1. **Clean Workspace & Strict Quality Gates**:
   - `cargo fmt --all -- --check` passes with zero formatting errors.
   - `cargo clippy --workspace --all-targets -- -D warnings` completes without warnings.
   - `cargo test --workspace` passes 100% of automated tests.
2. **Deterministic Cryptographic Hashes**:
   - All release packages (`Setup.exe`, portable zip) must have matching SHA-256 entries in `SHA256SUMS.txt`.
3. **Reproducible SBOM**:
   - `SBOM.spdx.json` generated and bundled with release metadata.
4. **Driver Attestation**:
   - Pinned official WinDivert driver SHA-256 hashes must be verified against `third_party/windivert/verify_driver.ps1`.
5. **No Secret / Local Path Leaks**:
   - Source code, commit logs, and documentation must contain no absolute local filesystem paths (`C:\Users\...`) or private credentials.

---

## 2. Release Steps

### Step 1: Version Bumping
Update version numbers across all package descriptors:
* `Cargo.toml` (workspace package version)
* `app/gui/package.json` and `app/gui/package-lock.json`
* `app/gui/src-tauri/tauri.conf.json`
* `CHANGELOG.md` (add new version entry with release notes)

### Step 2: Build & Verify Artifacts
```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1 -Configuration Release
```

Verify that the following artifacts are generated in `release/`:
* `ZonDPI-<version>-Setup.exe`
* `ZonDPI-<version>-Windows-x64-portable.zip`
* `SHA256SUMS.txt`
* `manifest.json`
* `SBOM.spdx.json`

Compute and double-check SHA-256 hashes:
```powershell
Get-FileHash release\ZonDPI-*.exe, release\ZonDPI-*.zip -Algorithm SHA256
```

### Step 3: Git Tagging
Create an annotated release tag:
```bash
git tag -a v<version> -m "ZonDPI v<version> release"
git push origin v<version>
```

### Step 4: GitHub Release Publication
Publish the release using the GitHub CLI:
```bash
gh release create v<version> \
  release/ZonDPI-<version>-Setup.exe \
  release/ZonDPI-<version>-Windows-x64-portable.zip \
  release/SHA256SUMS.txt \
  release/SBOM.spdx.json \
  release/manifest.json \
  --title "ZonDPI v<version>" \
  --notes-file release/RELEASE_NOTES.md
```
