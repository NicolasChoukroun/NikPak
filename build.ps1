<#
  build.ps1 - build nikpak.exe and unNik.exe on Windows with PowerShell.

  Usage (from the nikpak folder, in PowerShell):
      .\build.ps1                 # release build -> dist\nikpak.exe and dist\unNik.exe
      .\build.ps1 -Test           # run the unit tests first
      .\build.ps1 -Installer      # also make the setup: dist\nikpak-<version>-setup.exe
                                  # (needs Inno Setup 6: winget install JRSoftware.InnoSetup)
  If PowerShell refuses to run scripts:
      powershell -ExecutionPolicy Bypass -File .\build.ps1
#>
param([switch]$Test, [switch]$Installer)

$ErrorActionPreference = 'Stop'
Set-Location -Path $PSScriptRoot

# --- 1. Is Rust (cargo) installed? ------------------------------------------
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (Test-Path (Join-Path $cargoBin 'cargo.exe')) {
        $env:Path = "$cargoBin;$env:Path"
    }
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host ''
    Write-Host 'Rust (cargo) was not found.' -ForegroundColor Yellow
    Write-Host 'Install it first, then open a NEW PowerShell window and run this script again:'
    Write-Host '    winget install Rustlang.Rustup'
    Write-Host '    winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"'
    Write-Host 'or download the installer from https://rustup.rs'
    exit 1
}

Write-Host "Using $(cargo --version)"

# nikpak 1.2+ needs Rust 1.89 or newer (required by the preflate-rs library).
$rv = (rustc --version) -replace '^rustc (\d+)\.(\d+).*$', '$1.$2'
if ([version]$rv -lt [version]'1.89') {
    Write-Host "Rust $rv is too old (1.89 or newer is needed). Updating with: rustup update stable" -ForegroundColor Yellow
    rustup update stable
    if ($LASTEXITCODE -ne 0) { Write-Host 'rustup update failed.' -ForegroundColor Red; exit $LASTEXITCODE }
}

# --- 2. Test (optional) -------------------------------------------------------
if ($Test) {
    cargo test --release
    if ($LASTEXITCODE -ne 0) { Write-Host 'Tests failed.' -ForegroundColor Red; exit $LASTEXITCODE }
}

# --- 3. Build -------------------------------------------------------------------
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Host ''
    Write-Host 'Build failed. Most common cause: the Visual Studio C++ Build Tools are missing.' -ForegroundColor Red
    Write-Host 'Install them with:'
    Write-Host '    winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"'
    exit $LASTEXITCODE
}

# --- 4. Copy the result -----------------------------------------------------------
New-Item -ItemType Directory -Force -Path dist | Out-Null
Copy-Item -Force 'target\release\nikpak.exe' 'dist\nikpak.exe'
Copy-Item -Force 'target\release\unNik.exe' 'dist\unNik.exe'
$size = [math]::Round((Get-Item 'dist\nikpak.exe').Length / 1MB, 2)
$usize = [math]::Round((Get-Item 'dist\unNik.exe').Length / 1MB, 2)
Write-Host ''
Write-Host "OK -> $PSScriptRoot\dist\nikpak.exe  ($size MB)" -ForegroundColor Green
Write-Host "OK -> $PSScriptRoot\dist\unNik.exe   ($usize MB, the free archive browser)" -ForegroundColor Green
Write-Host 'Try:  .\dist\nikpak.exe -help    or    .\dist\unNik.exe some-archive.nik'

# --- 5. Installer (optional) ------------------------------------------------------
if ($Installer) {
    $version = (Select-String -Path 'Cargo.toml' -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
    $iscc = (Get-Command iscc -ErrorAction SilentlyContinue).Source
    if (-not $iscc) {
        $iscc = @(
            (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
            (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
            (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
        ) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    }
    if (-not $iscc) {
        Write-Host ''
        Write-Host 'Inno Setup 6 was not found. Install it once, then run .\build.ps1 -Installer again:' -ForegroundColor Yellow
        Write-Host '    winget install JRSoftware.InnoSetup'
        exit 1
    }
    & $iscc /Q "/DMyAppVersion=$version" 'installer\nikpak.iss'
    if ($LASTEXITCODE -ne 0) { Write-Host 'Installer build failed.' -ForegroundColor Red; exit $LASTEXITCODE }
    Write-Host "OK -> $PSScriptRoot\dist\nikpak-$version-setup.exe" -ForegroundColor Green
    Write-Host 'Run it to install NikPak (Start menu, uninstaller in Settings > Apps).'
}
