# build.ps1 - Unified Automated Build Pipeline for ZonDPI
[CmdletBinding()]
param (
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    [switch]$SkipTests,
    [switch]$PackageOnly
)

$ErrorActionPreference = "Stop"
$ProjectRoot = $PSScriptRoot
$ArtifactsDir = Join-Path $ProjectRoot "artifacts"
$DistDir = Join-Path $ProjectRoot "dist"

# Toolchain discovery helper
$ToolchainPaths = @(
    (Join-Path $env:USERPROFILE ".cargo\bin"),
    "C:\tools\mingw64\bin",
    "C:\msys64\ucrt64\bin",
    "C:\msys64\mingw64\bin",
    "C:\ProgramData\chocolatey\bin"
)
foreach ($path in $ToolchainPaths) {
    if ((Test-Path $path) -and ($env:PATH -notlike "*$path*")) {
        $env:PATH = "$path;$env:PATH"
    }
}

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "         ZonDPI Unified Build Pipeline            " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

if ($PackageOnly) {
    Write-Host "`n****************************************************************" -ForegroundColor Yellow
    Write-Host " [NOTICE] RUNNING IN -PackageOnly MODE" -ForegroundColor Yellow
    Write-Host " Assembling pre-compiled artifacts into distribution package." -ForegroundColor Yellow
    Write-Host " Note: Pre-compiled binaries MUST already exist." -ForegroundColor Yellow
    Write-Host "****************************************************************`n" -ForegroundColor Yellow
}

# Step 1: Check Toolchains
Write-Host "`n[1/7] Validating build toolchains..." -ForegroundColor Yellow
$CargoCmd = Get-Command cargo -ErrorAction SilentlyContinue
$GccCmd = (Get-Command x86_64-w64-mingw32-gcc -ErrorAction SilentlyContinue)
if (-not $GccCmd) { $GccCmd = (Get-Command gcc -ErrorAction SilentlyContinue) }
$MakeCmd = (Get-Command mingw32-make -ErrorAction SilentlyContinue)
if (-not $MakeCmd) { $MakeCmd = (Get-Command make -ErrorAction SilentlyContinue) }

Write-Host "  Rust (cargo):   $(if ($CargoCmd) { $CargoCmd.Source } else { 'Not found' })"
Write-Host "  C Compiler:     $(if ($GccCmd) { $GccCmd.Source } else { 'Not found' })"
Write-Host "  GNU Make:       $(if ($MakeCmd) { $MakeCmd.Source } else { 'Not found' })"

if (-not $PackageOnly) {
    $Missing = @()
    if (-not $CargoCmd) { $Missing += "Rust ('cargo')" }
    if (-not $GccCmd) { $Missing += "MinGW-w64 C Compiler ('gcc')" }
    if (-not $MakeCmd) { $Missing += "GNU Make ('mingw32-make' / 'make')" }

    if ($Missing.Count -gt 0) {
        Write-Error "ERROR: Missing required toolchains: $($Missing -join ', '). Real compilation is mandatory."
        exit 1
    }
}

# Step 2: Verify WinDivert Driver Hashes
Write-Host "`n[2/7] Verifying third-party driver dependencies..." -ForegroundColor Yellow
& (Join-Path $ProjectRoot "third_party\windivert\verify_driver.ps1")
if ($LASTEXITCODE -ne 0) {
    Write-Error "WinDivert dependency verification failed! Aborting."
    exit 1
}

# Directories setup
$GoodbyeArtifactDir = Join-Path $ArtifactsDir "engines\goodbye"
$ByeDpiArtifactDir = Join-Path $ArtifactsDir "engines\byedpi"
New-Item -ItemType Directory -Force -Path $GoodbyeArtifactDir | Out-Null
New-Item -ItemType Directory -Force -Path $ByeDpiArtifactDir | Out-Null

# Copy WinDivert.dll into Goodbye artifact dir for runtime dependency resolution
Copy-Item "$ProjectRoot\third_party\windivert\x64\WinDivert.dll" "$GoodbyeArtifactDir\" -Force

# Step 3: Native C Engine Compilation
if (-not $PackageOnly) {
    Write-Host "`n[3/7] Compiling Userspace C Networking Engines from Source..." -ForegroundColor Yellow

    # 3.1 GoodbyeDPI Build
    Write-Host "  [GoodbyeDPI] Compiling worker from engines/goodbye/upstream/src..." -ForegroundColor Gray
    $GoodbyeSrcDir = Join-Path $ProjectRoot "engines\goodbye\upstream\src"
    $Headers = Join-Path $ProjectRoot "third_party\windivert\include"
    $Libs = Join-Path $ProjectRoot "third_party\windivert\x64"

    Push-Location $GoodbyeSrcDir
    try {
        & $MakeCmd.Source clean
        & $MakeCmd.Source CPREFIX="" BIT64=1 "WINDIVERTHEADERS=$Headers" "WINDIVERTLIBS=$Libs"
        if ($LASTEXITCODE -ne 0) {
            throw "GoodbyeDPI make compilation returned exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }

    $CompiledGoodbye = Join-Path $GoodbyeSrcDir "goodbyedpi.exe"
    if (-not (Test-Path $CompiledGoodbye)) {
        Write-Error "GoodbyeDPI build artifact not found at $CompiledGoodbye"
        exit 1
    }
    $FinalGoodbye = Join-Path $GoodbyeArtifactDir "goodbyedpi.exe"
    Copy-Item $CompiledGoodbye $FinalGoodbye -Force
    $GoodbyeHash = (Get-FileHash $FinalGoodbye -Algorithm SHA256).Hash
    $GoodbyeSize = (Get-Item $FinalGoodbye).Length
    Write-Host "  [OK] GoodbyeDPI compiled -> $FinalGoodbye ($GoodbyeSize bytes, SHA256: $GoodbyeHash)" -ForegroundColor Green

    # 3.2 ByeDPI Build
    Write-Host "  [ByeDPI] Compiling worker from engines/byedpi/upstream..." -ForegroundColor Gray
    $ByeDpiSrcDir = Join-Path $ProjectRoot "engines\byedpi\upstream"

    Push-Location $ByeDpiSrcDir
    try {
        & $MakeCmd.Source clean
        & $MakeCmd.Source windows CC="$($GccCmd.Source)"
        if ($LASTEXITCODE -ne 0) {
            throw "ByeDPI make compilation returned exit code $LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }

    $CompiledByeDpi = Join-Path $ByeDpiSrcDir "ciadpi.exe"
    if (-not (Test-Path $CompiledByeDpi)) {
        Write-Error "ByeDPI build artifact not found at $CompiledByeDpi"
        exit 1
    }
    $FinalByeDpi = Join-Path $ByeDpiArtifactDir "ciadpi.exe"
    Copy-Item $CompiledByeDpi $FinalByeDpi -Force
    $ByeDpiHash = (Get-FileHash $FinalByeDpi -Algorithm SHA256).Hash
    $ByeDpiSize = (Get-Item $FinalByeDpi).Length
    Write-Host "  [OK] ByeDPI compiled -> $FinalByeDpi ($ByeDpiSize bytes, SHA256: $ByeDpiHash)" -ForegroundColor Green

    # 3.3 Smoke-test worker executables
    Write-Host "  [Smoke-Test] Validating worker invocation..." -ForegroundColor Gray
    
    # GoodbyeDPI smoke test
    $GoodbyeOutput = cmd.exe /c """$FinalGoodbye"" -h 2>&1" | Out-String
    if ($GoodbyeOutput -notlike "*GoodbyeDPI*") {
        Write-Error "GoodbyeDPI smoke test failed! Output did not contain expected banner.`n$GoodbyeOutput"
        exit 1
    }
    Write-Host "  [OK] GoodbyeDPI smoke test passed (help banner verified)." -ForegroundColor Green

    # ByeDPI smoke test
    $ByeDpiOutput = cmd.exe /c """$FinalByeDpi"" -h 2>&1" | Out-String
    if ($ByeDpiOutput -notlike "*--port*") {
        Write-Error "ByeDPI smoke test failed! Output did not contain expected help flags.`n$ByeDpiOutput"
        exit 1
    }
    Write-Host "  [OK] ByeDPI smoke test passed (CLI flags verified)." -ForegroundColor Green
} else {
    Write-Host "`n[3/7] Verifying pre-existing native engine artifacts (-PackageOnly mode)..." -ForegroundColor DarkYellow
    $FinalGoodbye = Join-Path $GoodbyeArtifactDir "goodbyedpi.exe"
    $FinalByeDpi = Join-Path $ByeDpiArtifactDir "ciadpi.exe"

    if (-not (Test-Path $FinalGoodbye)) {
        Write-Error "PackageOnly requested but required artifact is missing: $FinalGoodbye"
        exit 1
    }
    if (-not (Test-Path $FinalByeDpi)) {
        Write-Error "PackageOnly requested but required artifact is missing: $FinalByeDpi"
        exit 1
    }
    $GoodbyeHash = (Get-FileHash $FinalGoodbye -Algorithm SHA256).Hash
    $GoodbyeSize = (Get-Item $FinalGoodbye).Length
    $ByeDpiHash = (Get-FileHash $FinalByeDpi -Algorithm SHA256).Hash
    $ByeDpiSize = (Get-Item $FinalByeDpi).Length
}

# Step 4: Rust Workspace Verification & Compilation
if (-not $PackageOnly) {
    Write-Host "`n[4/7] Compiling & Verifying Rust Workspace..." -ForegroundColor Yellow
    
    Write-Host "  Running cargo fmt --check..." -ForegroundColor Gray
    & $CargoCmd.Source fmt --check
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Code formatting check failed! Run 'cargo fmt' to fix."
        exit 1
    }

    Write-Host "  Running cargo check --workspace --all-targets..." -ForegroundColor Gray
    & $CargoCmd.Source check --workspace --all-targets
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Cargo check failed!"
        exit 1
    }

    if (-not $SkipTests) {
        Write-Host "  Running cargo test --workspace --all-targets..." -ForegroundColor Gray
        & $CargoCmd.Source test --workspace --all-targets
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Cargo test execution failed!"
            exit 1
        }
    }

    Write-Host "  Running cargo clippy --workspace --all-targets -- -D warnings..." -ForegroundColor Gray
    & $CargoCmd.Source clippy --workspace --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Clippy linting failed!"
        exit 1
    }

    Write-Host "  Building GUI frontend (Vite)..." -ForegroundColor Gray
    Push-Location (Join-Path $ProjectRoot "app\gui")
    try {
        # Ensure dependencies are installed (in case it's a fresh clone)
        & npm install
        & npm run build
        if ($LASTEXITCODE -ne 0) {
            Write-Error "GUI frontend build failed!"
            exit 1
        }
    } finally {
        Pop-Location
    }

    $BuildArgs = @("build", "--workspace", "--all-targets")
    if ($Configuration -eq "Release") {
        $BuildArgs += "--release"
    }
    Write-Host "  Running cargo build ($Configuration)..." -ForegroundColor Gray
    & $CargoCmd.Source @BuildArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Cargo workspace build failed!"
        exit 1
    }
} else {
    Write-Host "`n[4/7] Skipping Rust compilation (-PackageOnly mode)." -ForegroundColor DarkYellow
}

$TargetSubdir = if ($Configuration -eq "Release") { "release" } else { "debug" }
$CompiledServiceExe = Join-Path $ProjectRoot "target\$TargetSubdir\zondpi-service.exe"
$CompiledCliExe = Join-Path $ProjectRoot "target\$TargetSubdir\zondpi-cli.exe"
if (-not (Test-Path $CompiledServiceExe)) {
    Write-Error "Windows Service binary missing at $CompiledServiceExe"
    exit 1
}
if (-not (Test-Path $CompiledCliExe)) {
    Write-Error "CLI binary missing at $CompiledCliExe"
    exit 1
}

# Step 5: Generate Native Artifact Provenance Manifest
Write-Host "`n[5/7] Generating Native Engine Provenance Manifest..." -ForegroundColor Yellow
$GccVersion = if ($GccCmd) { (& $GccCmd.Source --version | Select-Object -First 1) } else { "GCC UCRT 16.1.0" }
$ManifestObj = [ordered]@{
    "goodbye" = [ordered]@{
        "upstream_repo" = "https://github.com/ValdikSS/GoodbyeDPI"
        "upstream_commit" = "3114036d096865fba19e8b379d5fb8216423d21c"
        "version" = "v0.2.3rc3"
        "compiler" = $GccVersion
        "target" = "x86_64-w64-mingw32"
        "size_bytes" = $GoodbyeSize
        "sha256" = $GoodbyeHash
    }
    "byedpi" = [ordered]@{
        "upstream_repo" = "https://github.com/hufrea/byedpi"
        "upstream_commit" = "ba532298de7b28cfe854aea83d061369d13ca290"
        "version" = "post-v0.17.3 (ba53229)"
        "compiler" = $GccVersion
        "target" = "x86_64-w64-mingw32"
        "size_bytes" = $ByeDpiSize
        "sha256" = $ByeDpiHash
    }
    "generated_at" = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
}
$ManifestJson = $ManifestObj | ConvertTo-Json -Depth 4
$ManifestPath = Join-Path $ArtifactsDir "engines\manifest.json"
$ManifestJson | Out-File -FilePath $ManifestPath -Encoding utf8
Write-Host "  [OK] Engine manifest written to $ManifestPath" -ForegroundColor Green

# Step 6: Assemble Distributable Layout
Write-Host "`n[6/7] Assembling Distributable Output in dist/..." -ForegroundColor Yellow
if (Test-Path $DistDir) {
    Remove-Item -Recurse -Force $DistDir
}
New-Item -ItemType Directory -Force -Path "$DistDir\engines\goodbye" | Out-Null
New-Item -ItemType Directory -Force -Path "$DistDir\engines\byedpi" | Out-Null
New-Item -ItemType Directory -Force -Path "$DistDir\third_party\windivert" | Out-Null
New-Item -ItemType Directory -Force -Path "$DistDir\profiles" | Out-Null
New-Item -ItemType Directory -Force -Path "$DistDir\docs" | Out-Null

Copy-Item $CompiledServiceExe "$DistDir\" -Force
Copy-Item $CompiledCliExe "$DistDir\" -Force

$CompiledGuiExe = Join-Path $ProjectRoot "target\$TargetSubdir\zondpi-gui.exe"
if (Test-Path $CompiledGuiExe) {
    Copy-Item $CompiledGuiExe "$DistDir\zondpi.exe" -Force
    Copy-Item $CompiledGuiExe "$DistDir\zondpi-gui.exe" -Force
}

Copy-Item $FinalGoodbye "$DistDir\engines\goodbye\" -Force
Copy-Item $FinalByeDpi "$DistDir\engines\byedpi\" -Force
Copy-Item "$ProjectRoot\third_party\windivert\x64\WinDivert.dll" "$DistDir\third_party\windivert\" -Force
Copy-Item "$ProjectRoot\third_party\windivert\x64\WinDivert64.sys" "$DistDir\third_party\windivert\" -Force

# Also place driver & dll in engines/goodbye for zero-lookup worker dependency
Copy-Item "$ProjectRoot\third_party\windivert\x64\WinDivert.dll" "$DistDir\engines\goodbye\" -Force
Copy-Item "$ProjectRoot\third_party\windivert\x64\WinDivert64.sys" "$DistDir\engines\goodbye\" -Force

Copy-Item -Recurse -Force "$ProjectRoot\profiles\*" "$DistDir\profiles\"
Copy-Item "$ProjectRoot\LICENSE" "$DistDir\" -Force
Copy-Item "$ProjectRoot\DEPENDENCIES.md" "$DistDir\" -Force
Copy-Item "$ProjectRoot\README.md" "$DistDir\" -Force
if (Test-Path "$ProjectRoot\README_PORTABLE.txt") { Copy-Item "$ProjectRoot\README_PORTABLE.txt" "$DistDir\" -Force }
if (Test-Path "$ProjectRoot\SBOM.spdx.json") { Copy-Item "$ProjectRoot\SBOM.spdx.json" "$DistDir\" -Force }
Copy-Item "$ProjectRoot\docs\*" "$DistDir\docs\" -Force
Copy-Item $ManifestPath "$DistDir\" -Force

# Step 7: Generate & Validate Cryptographic SHA-256 Manifest
Write-Host "`n[7/9] Generating & Validating dist SHA-256 Checksums..." -ForegroundColor Yellow
$manifestLines = Get-ChildItem -Path $DistDir -Recurse -File | 
    Where-Object { $_.Name -ne "SHA256SUMS.txt" } | 
    ForEach-Object {
        $fileHash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash
        $relPath = $_.FullName.Substring($DistDir.Length).Replace("\", "/")
        "$fileHash  .$relPath"
    }
$SumsFile = Join-Path $DistDir "SHA256SUMS.txt"
$manifestLines | Out-File -FilePath $SumsFile -Encoding utf8

# Verification pass
$AllVerified = $true
foreach ($line in (Get-Content $SumsFile)) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $parts = $line.Split(" ", [System.StringSplitOptions]::RemoveEmptyEntries)
    $expectedHash = $parts[0].Trim().ToUpperInvariant()
    $rel = $parts[1].Trim().TrimStart('.').TrimStart('/').Replace('/', '\')
    $targetFile = Join-Path $DistDir $rel
    if (-not (Test-Path $targetFile)) {
        Write-Error "File missing in manifest check: $targetFile"
        $AllVerified = $false
        continue
    }
    $actualHash = (Get-FileHash -Path $targetFile -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($actualHash -ne $expectedHash) {
        Write-Error "Checksum mismatch for $rel!`nExpected: $expectedHash`nActual:   $actualHash"
        $AllVerified = $false
    }
}

if (-not $AllVerified) {
    Write-Error "SHA256 manifest verification failed!"
    exit 1
}
Write-Host "  [OK] All $($manifestLines.Count) files in dist/ verified successfully against SHA256SUMS.txt." -ForegroundColor Green

# Step 8: Prepare Installer Staging Directory & Build NSIS
Write-Host "`n[8/10] Preparing Installer Staging Directory..." -ForegroundColor Yellow
$ReleaseDir = Join-Path $ProjectRoot "release"
if (Test-Path $ReleaseDir) { Remove-Item -Recurse -Force $ReleaseDir }
New-Item -ItemType Directory -Force -Path $ReleaseDir | Out-Null

# Create flat staging tree inside src-tauri so Tauri resources use NO parent traversal
$StageDir = Join-Path $ProjectRoot "app\gui\src-tauri\installer-stage"
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force -Path "$StageDir\engines\goodbye" | Out-Null
New-Item -ItemType Directory -Force -Path "$StageDir\engines\byedpi" | Out-Null
New-Item -ItemType Directory -Force -Path "$StageDir\third_party\windivert" | Out-Null
New-Item -ItemType Directory -Force -Path "$StageDir\profiles" | Out-Null
New-Item -ItemType Directory -Force -Path "$StageDir\docs" | Out-Null

Copy-Item "$DistDir\zondpi-service.exe" $StageDir -Force
Copy-Item "$DistDir\zondpi-cli.exe" $StageDir -Force
Copy-Item "$DistDir\engines\goodbye\goodbyedpi.exe" "$StageDir\engines\goodbye" -Force
Copy-Item "$DistDir\engines\goodbye\WinDivert.dll" "$StageDir\engines\goodbye" -Force
Copy-Item "$DistDir\engines\goodbye\WinDivert64.sys" "$StageDir\engines\goodbye" -Force
Copy-Item "$DistDir\engines\byedpi\ciadpi.exe" "$StageDir\engines\byedpi" -Force
Copy-Item "$DistDir\third_party\windivert\WinDivert.dll" "$StageDir\third_party\windivert" -Force
Copy-Item "$DistDir\third_party\windivert\WinDivert64.sys" "$StageDir\third_party\windivert" -Force
Copy-Item -Recurse -Force "$DistDir\profiles\*" "$StageDir\profiles"
Copy-Item "$DistDir\docs\*" "$StageDir\docs" -Force

# PACKAGING VALIDATION GATE - Fail build if _up_ or parent traversal detected
Write-Host "  Validating staging tree for _up_ path regression..." -ForegroundColor Gray
$BadPaths = Get-ChildItem -Path $StageDir -Recurse | Where-Object { $_.Name -eq "_up_" -or $_.Name -like "*..*" }
if ($BadPaths.Count -gt 0) {
    Write-Error "INSTALLER PACKAGING GATE FAILED! _up_ or parent traversal path detected in staging tree:"
    $BadPaths | ForEach-Object { Write-Error "  $($_.FullName)" }
    exit 1
}
Write-Host "  [OK] Staging tree validated - no _up_ or traversal paths." -ForegroundColor Green

# Step 9: Build Windows Installer (NSIS via Tauri)
Write-Host "`n[9/10] Compiling Windows Installer (NSIS via Tauri)..." -ForegroundColor Yellow
Push-Location (Join-Path $ProjectRoot "app\gui")
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    & npx tauri build --bundles nsis -c tauri.installer.json
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Tauri NSIS installer packaging failed!"
        exit 1
    }
} finally {
    Pop-Location
}

# Regression validation gate: Verify installer.nsi does NOT contain _up_
$GeneratedNsi = Join-Path $ProjectRoot "target\release\nsis\x64\installer.nsi"
if (Test-Path $GeneratedNsi) {
    $UpMatches = Select-String -Path $GeneratedNsi -Pattern "_up_"
    if ($UpMatches) {
        Write-Error "REGRESSION DETECTED! installer.nsi contains '_up_' path traversal!"
        exit 1
    }
    Write-Host "  [OK] Verified: installer.nsi contains 0 '_up_' occurrences." -ForegroundColor Green
}

$GeneratedSetup = Get-ChildItem -Path "$ProjectRoot\target\release\bundle\nsis\ZonDPI*.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
$CargoTomlContent = Get-Content (Join-Path $ProjectRoot "Cargo.toml") -Raw
if ($CargoTomlContent -match 'version\s*=\s*"([^"]+)"') {
    $PkgVersion = $matches[1]
} else {
    $PkgVersion = "1.0.5"
}

if ($GeneratedSetup) {
    Copy-Item $GeneratedSetup.FullName "$ReleaseDir\ZonDPI-$PkgVersion-Setup.exe" -Force
    $SetupHash = (Get-FileHash "$ReleaseDir\ZonDPI-$PkgVersion-Setup.exe" -Algorithm SHA256).Hash
    $SetupSize = (Get-Item "$ReleaseDir\ZonDPI-$PkgVersion-Setup.exe").Length
    Write-Host "  [OK] Installer produced -> $ReleaseDir\ZonDPI-$PkgVersion-Setup.exe ($SetupSize bytes, SHA256: $SetupHash)" -ForegroundColor Green
} else {
    Write-Error "NSIS setup executable not found in target/release/bundle/nsis!"
    exit 1
}

# Step 10: Assemble Portable Zip and Release Manifests
Write-Host "`n[10/10] Creating Portable Distribution Zip & Release Manifest..." -ForegroundColor Yellow
$PortableZip = Join-Path $ReleaseDir "ZonDPI-$PkgVersion-Windows-x64-portable.zip"
Compress-Archive -Path "$DistDir\*" -DestinationPath $PortableZip -Force
$ZipHash = (Get-FileHash $PortableZip -Algorithm SHA256).Hash
$ZipSize = (Get-Item $PortableZip).Length
Write-Host "  [OK] Portable package created -> $PortableZip ($ZipSize bytes, SHA256: $ZipHash)" -ForegroundColor Green

if (Test-Path "$ProjectRoot\RELEASE_NOTES.md") { Copy-Item "$ProjectRoot\RELEASE_NOTES.md" $ReleaseDir -Force }
if (Test-Path "$ProjectRoot\SBOM.spdx.json") { Copy-Item "$ProjectRoot\SBOM.spdx.json" $ReleaseDir -Force }

# Generate release manifest.json
$ReleaseManifest = [ordered]@{
    "name" = "ZonDPI"
    "version" = $PkgVersion
    "release_date" = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    "architecture" = "x86_64"
    "os" = "windows"
    "artifacts" = @(
        [ordered]@{
            "name" = "ZonDPI-$PkgVersion-Setup.exe"
            "type" = "installer"
            "sha256" = $SetupHash
            "size_bytes" = $SetupSize
        },
        [ordered]@{
            "name" = "ZonDPI-$PkgVersion-Windows-x64-portable.zip"
            "type" = "portable_zip"
            "sha256" = $ZipHash
            "size_bytes" = $ZipSize
        }
    )
}
$ReleaseManifest | ConvertTo-Json -Depth 4 | Out-File -FilePath "$ReleaseDir\manifest.json" -Encoding utf8

# Generate release SHA256SUMS.txt
$relSums = Get-ChildItem -Path $ReleaseDir -File | 
    Where-Object { $_.Name -ne "SHA256SUMS.txt" } | 
    ForEach-Object {
        $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash
        "$hash  $($_.Name)"
    }
$relSums | Out-File -FilePath "$ReleaseDir\SHA256SUMS.txt" -Encoding utf8

# Verify release checksums
Write-Host "  Validating release SHA256SUMS.txt..." -ForegroundColor Gray
foreach ($line in (Get-Content "$ReleaseDir\SHA256SUMS.txt")) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $parts = $line.Split(" ", [System.StringSplitOptions]::RemoveEmptyEntries)
    $expectedHash = $parts[0].Trim().ToUpperInvariant()
    $fname = $parts[1].Trim()
    $targetFile = Join-Path $ReleaseDir $fname
    $actualHash = (Get-FileHash -Path $targetFile -Algorithm SHA256).Hash.ToUpperInvariant()
    if ($actualHash -ne $expectedHash) {
        Write-Error "Release checksum mismatch for $fname!"
        exit 1
    }
}
Write-Host "  [OK] Release artifacts verified against release/SHA256SUMS.txt." -ForegroundColor Green

Write-Host "`n==================================================" -ForegroundColor Green
Write-Host "   ZonDPI v$PkgVersion RELEASE BUILD COMPLETE           " -ForegroundColor Green
Write-Host "   Distribution: dist/                            " -ForegroundColor Green
Write-Host "   Release:      release/                         " -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
exit 0
