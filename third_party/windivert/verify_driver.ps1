# verify_driver.ps1 - Validates cryptographic integrity of pinned WinDivert assets
$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

$ExpectedHashes = @{
    "x64/WinDivert.dll"   = "6110BFA44667405179C3E15E12AF1B62037E447ED59B054B19042032995E6C7E"
    "x64/WinDivert64.sys" = "E69B5BA3F0CD6CFB2983E442636E7F0B342B61B15264B0328317D4559C82CF50"
}

Write-Host "Verifying WinDivert pinned assets..." -ForegroundColor Cyan

$AllValid = $true
foreach ($file in $ExpectedHashes.Keys) {
    $fullPath = Join-Path $ScriptDir $file
    if (-not (Test-Path $fullPath)) {
        Write-Error "Missing required dependency file: $fullPath"
        $AllValid = $false
        continue
    }

    $actualHash = (Get-FileHash -Path $fullPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $expectedHash = $ExpectedHashes[$file].ToUpperInvariant()

    if ($actualHash -eq $expectedHash) {
        Write-Host "  [OK] $file (SHA256: $actualHash)" -ForegroundColor Green
    } else {
        Write-Error "  [FAIL] $file HASH MISMATCH!`n  Expected: $expectedHash`n  Actual:   $actualHash"
        $AllValid = $false
    }
}

if (-not $AllValid) {
    exit 1
}

Write-Host "All WinDivert binary assets verified successfully." -ForegroundColor Green
exit 0
