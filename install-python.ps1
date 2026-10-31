<#
  install-python.ps1 - install Python properly, with it in the PATH.

  Run this in the PowerShell you already use (elevated is fine):

      .\install-python.ps1

  It does four things, and running it twice is harmless:

    1. removes the Microsoft Store aliases for python.exe / python3.exe - the
       ones that answer "Python was not found; run without arguments to install
       from the Microsoft Store". The Store app itself is not touched;
    2. finds an existing Python that is simply not on the PATH, or installs
       Python 3.12 (winget first, then the official installer) with "Add to
       PATH" enabled;
    3. adds it to your user PATH, for this window and for every window opened
       afterwards;
    4. checks "python --version" and installs Pillow, which the picture scripts
       of this project use.

  Nothing else on the machine changes.
#>
param(
    [switch]$NoPillow,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

function Step([string]$s) { Write-Host ''; Write-Host "== $s ==" -ForegroundColor Cyan }
function Note([string]$s) { Write-Host "  $s" }
function Warm([string]$s) { Write-Host "  $s" -ForegroundColor Yellow }

# --- 1. the Store aliases ---------------------------------------------------
Step 'Microsoft Store aliases'
$aliasDir = Join-Path $env:LOCALAPPDATA 'Microsoft\WindowsApps'
$removed = 0
foreach ($name in @('python.exe', 'python3.exe')) {
    $p = Join-Path $aliasDir $name
    if (Test-Path $p) {
        try {
            Remove-Item $p -Force
            Note "removed $p"
            $removed++
        } catch {
            Warm "could not remove $p - $($_.Exception.Message)"
        }
    }
}
if ($removed -eq 0) { Note 'none to remove' }

# --- 2. Python itself -------------------------------------------------------
Step 'Python'
function Test-Python([string]$exe) {
    if (-not $exe -or -not (Test-Path $exe)) { return $false }
    if ($exe -like '*\WindowsApps\*') { return $false }
    $soft = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & $exe --version *> $null
    $ok = ($LASTEXITCODE -eq 0)
    $ErrorActionPreference = $soft
    return $ok
}

function Find-Python {
    foreach ($name in @('python.exe', 'python3.exe')) {
        $c = Get-Command $name -ErrorAction SilentlyContinue
        if ($c -and (Test-Python $c.Source)) { return $c.Source }
    }
    $roots = @($env:ProgramFiles, 'C:\', (Join-Path $env:LOCALAPPDATA 'Programs\Python')) |
        Where-Object { $_ -and (Test-Path $_) }
    $hits = @()
    foreach ($r in $roots) {
        $hits += @(Get-ChildItem $r -Filter python.exe -Recurse -Depth 3 -ErrorAction SilentlyContinue |
            ForEach-Object { $_.FullName })
    }
    foreach ($h in ($hits | Sort-Object -Descending)) {
        if (Test-Python $h) { return $h }
    }
    return $null
}

$python = Find-Python
if ($python) {
    Note "found: $python"
} else {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
        Note 'installing Python 3.12 with winget (a few minutes)'
        winget install --id Python.Python.3.12 --exact --silent --accept-package-agreements --accept-source-agreements | Out-Host
    }
    if (-not (Find-Python)) {
        Note 'downloading the official installer'
        $exe = Join-Path $env:TEMP 'python-setup.exe'
        Invoke-WebRequest -Uri 'https://www.python.org/ftp/python/3.12.10/python-3.12.10-amd64.exe' -OutFile $exe
        Start-Process -FilePath $exe -Wait -ArgumentList '/quiet', 'InstallAllUsers=0', 'PrependPath=1', 'Include_pip=1'
    }
    $python = Find-Python
    if ($python) { Note "installed: $python" } else { Warm 'Python was not installed (winget and the installer both failed)' }
}

# --- 3. the PATH ------------------------------------------------------------
Step 'PATH'
if ($python) {
    $dirs = @((Split-Path $python), (Join-Path (Split-Path $python) 'Scripts'))
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $parts = @()
    if ($userPath) { $parts = $userPath -split ';' | Where-Object { $_ } }
    foreach ($d in $dirs) {
        if ($parts -notcontains $d) {
            $parts += $d
            Note "added to the user PATH: $d"
        }
        if (($env:Path -split ';') -notcontains $d) { $env:Path = "$d;$env:Path" }
    }
    [Environment]::SetEnvironmentVariable('Path', ($parts -join ';'), 'User')
    Note "this window now uses: $python"
}

# --- 4. check, and Pillow ---------------------------------------------------
Step 'check'
$soft = $ErrorActionPreference
$ErrorActionPreference = 'Continue'
& python --version 2>&1 | Out-Host
if ($LASTEXITCODE -ne 0) {
    $ErrorActionPreference = $soft
    Warm 'python still does not answer in this window - close it, open a new one and run: python --version'
    exit 1
}
if (-not $NoPillow) {
    Note 'installing Pillow (the picture scripts use it)'
    & python -m pip install --upgrade --quiet pillow 2>&1 | Out-Host
    & python -c "import PIL, sys; print('Pillow', PIL.__version__, 'on Python', sys.version.split()[0])" 2>&1 | Out-Host
}
$ErrorActionPreference = $soft

Write-Host ''
Write-Host 'Done.' -ForegroundColor Green
Write-Host '  .\buildskins.ps1        - the skin pictures and the skins page (nikpak)'
Write-Host '  .\tools\site-shots.ps1  - the home-page screenshots'
Write-Host 'A window that was already open keeps its old PATH: open a new one if a'
Write-Host 'script still says it cannot find Python.'
