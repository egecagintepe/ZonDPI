# tools/cleanup_windivert.ps1
# ZonDPI WinDivert Driver & Process Cleanup Script
# Run this script from an elevated PowerShell prompt (Run as Administrator)

$ErrorActionPreference = "SilentlyContinue"

Write-Host "==================================================" -ForegroundColor Cyan
Write-Host "   ZonDPI & WinDivert Driver Cleanup Utility      " -ForegroundColor Cyan
Write-Host "==================================================" -ForegroundColor Cyan

# 1. Check Administrator Privileges
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Warning "Bu betik yonetici yetkileriyle calistirilmalidir! Lutfen PowerShell'i 'Yonetici olarak calistir' ile aciniz."
    exit 1
}

# 2. Terminate all worker and DPI bypass processes holding driver handles
Write-Host "[1/4] Arka plan DPI ve surucu kullanan surecler sonlandiriliyor..." -ForegroundColor Yellow
$processNames = @("goodbyedpi", "ciadpi", "zondpi-engine-worker", "zondpi-gui", "zondpi-cli", "zapret", "byedpi", "spoof", "windivert")
foreach ($name in $processNames) {
    $procs = Get-Process -Name $name -ErrorAction SilentlyContinue
    if ($procs) {
        Write-Host "  -> $name sureci sonlandiriliyor ($($procs.Count) adet)..." -ForegroundColor DarkGray
        $procs | Stop-Process -Force -ErrorAction SilentlyContinue
    }
}
Start-Sleep -Milliseconds 500

# 3. Stop ZonDPI Windows Service if installed
Write-Host "[2/4] ZonDPI Windows Hizmeti kontrol ediliyor..." -ForegroundColor Yellow
$svc = Get-Service -Name "ZonDPI" -ErrorAction SilentlyContinue
if ($svc) {
    if ($svc.Status -ne "Stopped") {
        Write-Host "  -> ZonDPI hizmeti durduruluyor..." -ForegroundColor DarkGray
        Stop-Service -Name "ZonDPI" -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 500
    }
}

# 4. Stop and delete WinDivert driver services
Write-Host "[3/4] WinDivert cekirdek suruculeri durduruluyor ve siliniyor..." -ForegroundColor Yellow
$driverServices = @("WinDivert", "WinDivert14", "WinDivert22", "windivert", "windivert14", "windivert22")
foreach ($d in $driverServices) {
    # Attempt stop first
    & net.exe stop $d /y 2>&1 | Out-Null
    # Attempt SCM delete
    & sc.exe delete $d 2>&1 | Out-Null
}
Start-Sleep -Milliseconds 500

# Second pass to finalize pending marked-for-deletion handles
foreach ($d in $driverServices) {
    & net.exe stop $d /y 2>&1 | Out-Null
}

# 5. Verification
Write-Host "[4/4] Sistem surucu durumu dogrulaniyor..." -ForegroundColor Yellow
$runningDrivers = Get-CimInstance Win32_SystemDriver -ErrorAction SilentlyContinue | Where-Object { $_.Name -match "windivert" }
$scmStatus = & sc.exe query windivert 2>&1

if ($runningDrivers -or ($scmStatus -match "RUNNING")) {
    Write-Warning "WinDivert surucusu halen sistemde aktif gorunuyor."
    Write-Warning "Not: Eger bir surec acik tutuyorsa sistem yeniden baslatildiginda silme islemi otomatik tamamlanacaktir."
} else {
    Write-Host "==================================================" -ForegroundColor Green
    Write-Host "BASARILI: WinDivert surucusu ve servisleri sistemden tamamen kaldirildi." -ForegroundColor Green
    Write-Host "==================================================" -ForegroundColor Green
}
