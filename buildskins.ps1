<#
  buildskins.ps1 - rebuild the skin gallery of the NikPak web site.

  One picture per skin, 500 x 560, in website/static/img/skins/
  (<id>.png and <id>-thumb.png at 250 x 280), taken from the real program:

    1. runs cargo build --release (skip with -NoBuild),
    2. starts target\release\nikpak.exe once per new skin, in that skin, forces
       the window to 500 x 560 and captures it,
    3. turns the captures into <id>.png and <id>-thumb.png (System.Drawing, part
       of Windows: nothing to install): the window has rounded corners, so the
       tile is masked with the skin's own radius and sits on the page like the
       ones already there,
    4. adds the entries to website/static/img/skins/skins.json,
    5. puts the real number of skins in the gallery heading of the ten languages,
    6. rebuilds the site with the site builder (skip with -NoSite).

  The tiles are made with System.Drawing and the page text with plain
  PowerShell, so this part needs nothing installed - no Python, no Pillow. The
  site build (step 6) is a Python script: it is found wherever it is installed,
  even when it is not on the PATH. The pictures already on the site are .webp
  and stay as they are; the new ones are .png, and skins.json says which is
  which.

  Usage, from this folder in PowerShell - ADMINISTRATOR OR NOT, both work:

      .\buildskins.ps1                  # build, capture, rebuild the site
      .\buildskins.ps1 -NoBuild         # use the exe already built
      .\buildskins.ps1 -Only win11      # one skin, as a test
      .\buildskins.ps1 -Force           # redo the skins already done
      .\buildskins.ps1 -NoSite          # pictures and list only, no site build

  Only the skins missing from skins.json are captured, so a second run costs
  nothing. Existing pictures are never touched unless -Force is given.

  An administrator shell is fine - in fact it is the expected one. Normally
  NikPak restarts itself through Explorer when it is elevated (so files can be
  dropped on its window), which would hand the window to another process. For a
  capture that would be wrong, so the script sets NIKPAK_GUI_CAPTURE=1: the
  window then stays in the process that was started, keeps its own title bar,
  and the elevated shell copies it straight off the screen. No scheduled task,
  no second shell, nothing to grant.

  If the window still disappears at once (an older build without that switch),
  the script falls back to running the capture as the normal user through
  tools\unelevated.ps1. Keep the desktop unlocked with nothing over the window
  while it runs: the pictures are copied from the screen.
#>
param(
    [switch]$NoBuild,
    [switch]$Force,
    [string[]]$Only,
    [switch]$NoSite,
    # set by tools\unelevated.ps1 on the copy that runs as the normal user
    [switch]$Inner,
    [string]$Root
)

$ErrorActionPreference = 'Stop'
if (-not $Root) { $Root = $PSScriptRoot }
Set-Location $Root

. (Join-Path $Root 'tools\unelevated.ps1')
if ($Inner) { Start-Transcript -Path $script:UnelevatedLog -Force | Out-Null }

if ($Only) {
    $Only = @($Only | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}

# id, name, dark, corner radius in pixels (the "radius" of the skin)
$NEW = @(
    @('win10', 'Windows 10', $false, 2), @('win11', 'Windows 11', $false, 8),
    @('winvista', 'Windows Vista', $true, 6), @('win7', 'Windows 7', $false, 5),
    @('win95', 'Windows 95', $false, 0),
    @('macos-tahoe', 'macOS Tahoe', $false, 14), @('macos-sequoia', 'macOS Sequoia', $false, 12),
    @('macos-sonoma', 'macOS Sonoma', $false, 11), @('macos-ventura', 'macOS Ventura', $false, 10),
    @('macos-monterey', 'macOS Monterey', $false, 10), @('macos-big-sur', 'macOS Big Sur', $false, 10),
    @('macos-catalina', 'macOS Catalina', $true, 10), @('macos-mojave', 'macOS Mojave', $true, 8),
    @('macos-sierra', 'macOS Sierra', $false, 6),
    @('osx-el-capitan', 'OS X El Capitan', $false, 5), @('osx-yosemite', 'OS X Yosemite', $false, 4),
    @('osx-mavericks', 'OS X Mavericks', $false, 4), @('osx-lion', 'Mac OS X Lion', $false, 4),
    @('osx-leopard', 'Mac OS X Leopard', $false, 6), @('osx-tiger', 'Mac OS X Tiger', $false, 3),
    @('ubuntu-resolute', 'Ubuntu Resolute Raccoon', $false, 8), @('ubuntu-noble', 'Ubuntu Noble Numbat', $true, 8),
    @('ubuntu-jammy', 'Ubuntu Jammy Jellyfish', $false, 8), @('ubuntu-focal', 'Ubuntu Focal Fossa', $false, 6),
    @('ubuntu-bionic', 'Ubuntu Bionic Beaver', $false, 6), @('ubuntu-xenial', 'Ubuntu Xenial Xerus', $false, 4),
    @('ubuntu-trusty', 'Ubuntu Trusty Tahr', $true, 4), @('ubuntu-precise', 'Ubuntu Precise Pangolin', $false, 4),
    @('ubuntu-natty', 'Ubuntu Natty Narwhal', $false, 4), @('ubuntu-lucid', 'Ubuntu Lucid Lynx', $true, 4),
    @('ubuntu-karmic', 'Ubuntu Karmic Koala', $true, 4), @('ubuntu-hardy', 'Ubuntu Hardy Heron', $true, 3),
    @('ubuntu-gutsy', 'Ubuntu Gutsy Gibbon', $false, 3), @('ubuntu-dapper', 'Ubuntu Dapper Drake', $false, 3),
    # a painted skin, listed so -Only / -Force can redo it on its own
    @('amiga', 'Amiga Computer', $false, 3)
)

# --- python, wherever it is ------------------------------------------------
# The site builder is a Python script. Python is often installed but not on the
# PATH (the Microsoft Store alias shadows it), so the usual places are searched:
# the PATH, the "py" launcher, the registry, and the default install folders.
function Find-Python {
    $candidates = @()
    foreach ($name in @('python.exe', 'python3.exe', 'python')) {
        $c = Get-Command $name -ErrorAction SilentlyContinue
        if ($c) { $candidates += $c.Source }
    }
    try {
        $py = Get-Command py.exe -ErrorAction SilentlyContinue
        if ($py) { $candidates += $py.Source }
    } catch { }
    $local = $env:LOCALAPPDATA
    $roots = @('C:\Python313', 'C:\Python312', 'C:\Python311', 'C:\Python310',
               'C:\Program Files\Python313', 'C:\Program Files\Python312',
               'C:\Program Files\Python311', 'C:\Program Files\Python310')
    if ($local) { $roots += (Join-Path $local 'Programs\Python'); $roots += $local }
    foreach ($root in $roots) {
        if ($root -and (Test-Path $root)) {
            $candidates += @(Get-ChildItem $root -Filter python.exe -Recurse -Depth 2 -ErrorAction SilentlyContinue |
                ForEach-Object { $_.FullName })
        }
    }
    try {
        foreach ($hive in @('HKCU:\SOFTWARE\Python\PythonCore', 'HKLM:\SOFTWARE\Python\PythonCore')) {
            if (Test-Path $hive) {
                foreach ($key in Get-ChildItem $hive -ErrorAction SilentlyContinue) {
                    $candidates += (Join-Path (Get-ItemProperty $key.PSPath -Name InstallPath -ErrorAction SilentlyContinue).InstallPath 'python.exe')
                }
            }
        }
    } catch { }
    foreach ($c in $candidates) {
        if (-not $c -or -not (Test-Path $c)) { continue }
        # the Microsoft Store stub answers but cannot run a script
        if ($c -like '*\WindowsApps\*') { continue }
        $soft = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        & $c -c "import sys; print(sys.version_info[0])" *> $null
        $ok = ($LASTEXITCODE -eq 0)
        $ErrorActionPreference = $soft
        if ($ok) { return $c }
    }
    return $null
}

# Read a JSON list the same way in Windows PowerShell 5.1 and in PowerShell 7.
# 5.1 hands back a whole array as ONE object (so a loop over it sees the array,
# not its items); 7 unrolls it. Piping through ForEach-Object flattens both.
function Read-JsonList([string]$path) {
    $data = Get-Content $path -Raw -Encoding UTF8 | ConvertFrom-Json
    return @($data | ForEach-Object { $_ })
}

$jsonPath = Join-Path $Root 'website\static\img\skins\skins.json'
$imgDir = Join-Path $Root 'website\static\img\skins'
$siteDir = Join-Path $Root 'website'
if (-not (Test-Path $jsonPath)) { Write-Host "Not found: $jsonPath" -ForegroundColor Red; exit 1 }

$version = (Select-String -Path (Join-Path $Root 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$stage = Join-Path $Root "screenshots\$version\skin-tiles"
New-Item -ItemType Directory -Force -Path $stage | Out-Null
$jobsPath = Join-Path $stage 'jobs.json'

if (-not $Inner) {
    Write-Host '== NikPak skin gallery ==' -ForegroundColor Cyan

    # --- the skins still to do ----------------------------------------------
    $have = @{}
    foreach ($line in (Get-Content $jsonPath)) {
        if ($line -match '"id"\s*:\s*"([^"]+)"') { $have[$Matches[1]] = $true }
    }
    $wanted = @()
    foreach ($s in $NEW) {
        if ($Only -and ($Only -notcontains $s[0])) { continue }
        if ($have.ContainsKey($s[0]) -and -not $Force) { continue }
        $wanted += , $s
    }
    if ($wanted.Count -eq 0) {
        Write-Host '  every skin is already in skins.json' -ForegroundColor Green
        Write-Host '  (-Force redoes them, -Only <id> picks one)'
        exit 0
    }

    if (-not $NoBuild) {
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
            if (Test-Path (Join-Path $cargoBin 'cargo.exe')) { $env:Path = "$cargoBin;$env:Path" }
        }
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            Write-Host 'Rust (cargo) was not found. Install it with:  winget install Rustlang.Rustup' -ForegroundColor Red
            exit 1
        }
        Write-Host '== cargo build --release ==' -ForegroundColor Cyan
        cargo build --release
        if ($LASTEXITCODE -ne 0) { Write-Host 'Build failed - nothing captured.' -ForegroundColor Red; exit 1 }
    }

    # the list travels to the normal-user copy through a small JSON file
    $jobs = @()
    foreach ($s in $wanted) { $jobs += [pscustomobject]@{ id = $s[0]; name = $s[1]; dark = $s[2]; radius = $s[3] } }
    [System.IO.File]::WriteAllText($jobsPath, ($jobs | ConvertTo-Json -Depth 3), (New-Object System.Text.UTF8Encoding($false)))

}

# ===========================================================================
# the capture. In this process - NIKPAK_GUI_CAPTURE makes the window stay where
# it is, elevated or not. Only if that window vanishes (a build without the
# switch) does the work go to a normal-user copy through tools\unelevated.ps1.
# ===========================================================================
if (-not (Test-Path $jobsPath)) { Write-Host "Not found: $jobsPath" -ForegroundColor Red; exit 1 }
$jobs = Read-JsonList $jobsPath
$wanted = @($jobs | ForEach-Object { , @([string]$_.id, [string]$_.name, [bool]$_.dark, [int]$_.radius) })
Write-Host ("== capturing {0} skin(s) -> {1} ==" -f $wanted.Count, $stage) -ForegroundColor Cyan
if (Test-Elevated) { Write-Host '  (elevated shell: the window stays in this process, thanks to NIKPAK_GUI_CAPTURE)' -ForegroundColor DarkGray }

$exe = Join-Path $Root 'target\release\nikpak.exe'
if (-not (Test-Path $exe)) { Write-Host "Not found: $exe" -ForegroundColor Red; exit 1 }

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class SkinShot {
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
[DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
[DllImport("user32.dll")] public static extern uint GetDpiForSystem();
[DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
[DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
[DllImport("user32.dll")] static extern IntPtr GetParent(IntPtr h);
delegate bool EnumProc(IntPtr h, IntPtr l);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
public static void Raise(IntPtr h) { SetWindowPos(h, new IntPtr(-1), 0, 0, 0, 0, 0x0003); }
public static void Unraise(IntPtr h) { SetWindowPos(h, new IntPtr(-2), 0, 0, 0, 0, 0x0003); }
public static void Resize(IntPtr h, int w, int hh) { SetWindowPos(h, IntPtr.Zero, 0, 0, w, hh, 0x0002 | 0x0004 | 0x0010); }
public static IntPtr BiggestWindow(int pid) {
    IntPtr best = IntPtr.Zero;
    long bestArea = 0;
    EnumWindows(delegate(IntPtr h, IntPtr l) {
        uint p;
        GetWindowThreadProcessId(h, out p);
        if (p != (uint)pid) return true;
        if (!IsWindowVisible(h)) return true;
        if (GetParent(h) != IntPtr.Zero) return true;
        RECT r;
        if (!GetWindowRect(h, out r)) return true;
        long w = r.Right - r.Left, hh = r.Bottom - r.Top;
        if (w * hh > bestArea) { bestArea = w * hh; best = h; }
        return true;
    }, IntPtr.Zero);
    return best;
}
}
"@
[SkinShot]::SetProcessDPIAware() | Out-Null

# the window is sized in logical pixels; on a 150 % screen that is 1.5x as many
$dpi = 96
try { $dpi = [int][SkinShot]::GetDpiForSystem() } catch { }
if ($dpi -lt 96) { $dpi = 96 }
$scale = $dpi / 96.0
$winW = [int][Math]::Round(500 * $scale)
$winH = [int][Math]::Round(560 * $scale)

function Get-Window($proc) {
    for ($i = 0; $i -lt 200; $i++) {
        $proc.Refresh()
        if ($proc.HasExited) { return [IntPtr]::Zero }
        $h = [SkinShot]::BiggestWindow($proc.Id)
        if ($h.ToInt64() -ne 0) {
            $r = [SkinShot+RECT]::new()
            [SkinShot]::GetWindowRect($h, [ref]$r) | Out-Null
            if (($r.Right - $r.Left) -ge 400 -and ($r.Bottom - $r.Top) -ge 400) { return $h }
        }
        Start-Sleep -Milliseconds 100
    }
    return [IntPtr]::Zero
}

function Save-Shot($h, $file) {
    [SkinShot]::ShowWindow($h, 5) | Out-Null
    [SkinShot]::SetForegroundWindow($h) | Out-Null
    [SkinShot]::Raise($h)
    Start-Sleep -Milliseconds 700
    $r = [SkinShot+RECT]::new()
    [SkinShot]::GetWindowRect($h, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left
    $hh = $r.Bottom - $r.Top
    if ($w -le 0 -or $hh -le 0) { throw 'the window has no size' }
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try {
        $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $hh))
        $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $g.Dispose()
        $bmp.Dispose()
        [SkinShot]::Unraise($h)
    }
}

function Invoke-Shot($skin, $file) {
    $env:NIKPAK_GUI_NO_NAG = '1'
    $env:NIKPAK_GUI_NO_UPDATE = '1'
    # keep the window in this process, even when the shell is elevated
    $env:NIKPAK_GUI_CAPTURE = '1'
    $env:NIKPAK_GUI_SKIN = $skin
    Write-Host ("  {0,-18}" -f $skin) -NoNewline
    $p = Start-Process -FilePath $exe -PassThru
    try {
        $h = Get-Window $p
        if ($h.ToInt64() -eq 0) {
            if ($p.HasExited) { throw ("the program exited at once (code {0})" -f $p.ExitCode) }
            throw 'the window did not appear'
        }
        Start-Sleep -Milliseconds 1200
        [SkinShot]::Resize($h, $winW, $winH) | Out-Null
        Start-Sleep -Milliseconds 1300
        Save-Shot $h $file
        if ((Get-Item $file).Length -lt 4000) {
            Start-Sleep -Milliseconds 800
            Save-Shot $h $file
        }
        Write-Host 'ok' -ForegroundColor Green
    } catch {
        Write-Host ("FAILED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    } finally {
        if (-not $p.HasExited) {
            $p.CloseMainWindow() | Out-Null
            Start-Sleep -Milliseconds 400
            if (-not $p.HasExited) { $p.Kill() }
        }
        $p.Dispose()
        Remove-Item Env:NIKPAK_GUI_SKIN -ErrorAction SilentlyContinue
        Remove-Item Env:NIKPAK_GUI_NO_NAG -ErrorAction SilentlyContinue
        Remove-Item Env:NIKPAK_GUI_NO_UPDATE -ErrorAction SilentlyContinue
        Remove-Item Env:NIKPAK_GUI_CAPTURE -ErrorAction SilentlyContinue
    }
}

foreach ($s in $wanted) { Invoke-Shot $s[0] (Join-Path $stage ($s[0] + '.png')) }

$captured = @($wanted | Where-Object { Test-Path (Join-Path $stage ($_[0] + '.png')) })
if ($captured.Count -eq 0 -and -not $Inner -and (Test-Elevated)) {
    # the window went away at once: this build does not know NIKPAK_GUI_CAPTURE,
    # so let a normal-user copy try (tools\unelevated.ps1)
    Write-Host '  the window closed at once; retrying as the normal user...' -ForegroundColor Yellow
    $code = Invoke-Unelevated -Script $PSCommandPath -Root $Root -Log $script:UnelevatedLog `
        -Arguments @{ Inner = $true; NoBuild = $true; NoSite = $true }
    $captured = @($wanted | Where-Object { Test-Path (Join-Path $stage ($_[0] + '.png')) })
    if ($code -ne 0 -and $captured.Count -eq 0) {
        Write-Host ("  the normal-user capture failed too (exit {0})" -f $code) -ForegroundColor Red
        Write-Host ("  see {0}" -f $script:UnelevatedLog)
        exit 1
    }
}

if ($captured.Count -eq 0) { Write-Host 'No capture succeeded - nothing written.' -ForegroundColor Red; exit 1 }
Write-Host ("{0} capture(s) in {1}" -f $captured.Count, $stage) -ForegroundColor Cyan

# the normal-user copy only captures: this shell does the tiles, the list and the site
if ($Inner) { Stop-Transcript | Out-Null; return }

# --- 2. tiles (System.Drawing: part of Windows, nothing to install) ---------
Add-Type -AssemblyName System.Drawing

$manifest = @()
foreach ($s in $wanted) {
    if (-not (Test-Path (Join-Path $stage ($s[0] + '.png')))) { continue }
    $manifest += [pscustomobject]@{ id = $s[0]; name = $s[1]; dark = $s[2]; radius = $s[3] }
}
if ($manifest.Count -eq 0) { Write-Host 'No capture to turn into tiles.' -ForegroundColor Red; exit 1 }

# Rounded corners: the window has its own radius, and a screen copy shows the
# desktop in the corners. The alpha channel is punched out with a path.
function Save-Tile($source, $target, $w, $h, $radius) {
    $src = [System.Drawing.Image]::FromFile($source)
    try {
        $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        try {
            $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
            $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $g.Clear([System.Drawing.Color]::Transparent)
            $rect = New-Object System.Drawing.Rectangle 0, 0, $w, $h
            if ($radius -gt 0) {
                $path = New-Object System.Drawing.Drawing2D.GraphicsPath
                $d = $radius * 2
                $path.AddArc(0, 0, $d, $d, 180, 90)
                $path.AddArc($w - $d - 1, 0, $d, $d, 270, 90)
                $path.AddArc($w - $d - 1, $h - $d - 1, $d, $d, 0, 90)
                $path.AddArc(0, $h - $d - 1, $d, $d, 90, 90)
                $path.CloseFigure()
                $g.SetClip($path)
            }
            $g.DrawImage($src, $rect)
        } finally {
            $g.Dispose()
            $bmp.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
            $bmp.Dispose()
        }
    } finally {
        $src.Dispose()
    }
}

Write-Host '== skin tiles (png + thumbnails) ==' -ForegroundColor Cyan
$tiles = 0
foreach ($it in $manifest) {
    $srcFile = Join-Path $stage ($it.id + '.png')
    $big = Join-Path $imgDir ($it.id + '.png')
    $thumb = Join-Path $imgDir ($it.id + '-thumb.png')
    try {
        Save-Tile $srcFile $big 500 560 ([int]$it.radius)
        $tr = 0
        if ([int]$it.radius -gt 0) { $tr = [Math]::Max(1, [int][Math]::Round([int]$it.radius / 2)) }
        Save-Tile $srcFile $thumb 250 280 $tr
        $tiles++
        Write-Host ("  {0}" -f $it.id) -ForegroundColor Green
    } catch {
        Write-Host ("  {0}: {1}" -f $it.id, $_.Exception.Message) -ForegroundColor Red
    }
}
if ($tiles -eq 0) { Write-Host 'No tile was written.' -ForegroundColor Red; exit 1 }
Write-Host ("{0} tiles written to {1}" -f $tiles, $imgDir) -ForegroundColor Cyan

# --- 3. the gallery list ----------------------------------------------------
$doneIds = @{}
foreach ($s in $wanted) { $doneIds[$s[0]] = $true }

$body = @()
foreach ($line in (Get-Content $jsonPath)) {
    $t = $line.Trim()
    if ($t -eq ']' -or $t -eq '') { continue }
    if ($Force -and $t -match '"id"\s*:\s*"([^"]+)"' -and $doneIds.ContainsKey($Matches[1])) { continue }
    $body += $line
}
if ($body.Count -eq 0) { Write-Host 'skins.json looks empty - left alone.' -ForegroundColor Red; exit 1 }

$last = $body.Count - 1
$body[$last] = ($body[$last].TrimEnd() -replace ',\s*$', '') + ','

$add = @()
foreach ($s in $wanted) {
    # the pictures taken here are .png; the older ones are .webp, so the entry
    # says which file to use (website/build.py reads "img" and "thumb")
    if (-not (Test-Path (Join-Path $imgDir ($s[0] + '.png')))) { continue }
    $add += '  {"id": "' + $s[0] + '", "name": "' + $s[1] + '", "dark": ' + ($s[2].ToString().ToLower()) +
            ', "img": "' + $s[0] + '.png", "thumb": "' + $s[0] + '-thumb.png"},'
}
if ($add.Count -eq 0) { Write-Host 'No tile was written - skins.json left alone.' -ForegroundColor Red; exit 1 }
$add[$add.Count - 1] = $add[$add.Count - 1] -replace ',$', ''

[System.IO.File]::WriteAllLines($jsonPath, ($body + $add + @(']')), (New-Object System.Text.UTF8Encoding($false)))
Write-Host ("== skins.json: {0} entries added ==" -f $add.Count) -ForegroundColor Cyan

# --- 4. the numbers in the page text ---------------------------------------
# The heading says how many skins there are ("55 skins, one click"), and one
# bullet of "How it works" says how many came from Nikkomander. Both are brought
# up to date here, in the digits each language already uses: the Bengali page
# writes its numbers with Bengali digits (U+09E6 to U+09EF).
Write-Host '== the skin count in the page text ==' -ForegroundColor Cyan
$count = @(Read-JsonList $jsonPath).Count
$painted = 14

# "५५" -> 55 : the digits of a run, whatever script they are written in
function Get-Number([string]$run) {
    $n = 0
    foreach ($ch in $run.ToCharArray()) {
        $d = [int][char]$ch
        if ($d -ge 0x09E6 -and $d -le 0x09EF) { $d = $d - 0x09E6 }
        $n = $n * 10 + $d
    }
    return $n
}

# a number written in the same script as the run it replaces
function Format-Like([int]$value, [string]$run) {
    $bn = ([int][char]$run[0]) -ge 0x09E6
    if (-not $bn) { return $value.ToString() }
    return -join ($value.ToString().ToCharArray() | ForEach-Object { [char](0x09E6 + ([int][char]$_ - 48)) })
}

function Set-FirstNumber([string]$text, [int]$value, [int]$expect = -1) {
    $m = [regex]::Match($text, '[0-9]+|[\u09E6-\u09EF]+')
    if (-not $m.Success) { return $text }
    if ($expect -ge 0 -and (Get-Number $m.Value) -ne $expect) { return $text }
    return $text.Substring(0, $m.Index) + (Format-Like $value $m.Value) + $text.Substring($m.Index + $m.Length)
}

$touched = 0
foreach ($f in Get-ChildItem (Join-Path $siteDir 'i18n') -Filter *.json) {
    $text = [System.IO.File]::ReadAllText($f.FullName)
    $start = $text.IndexOf('"skins": {')            # not the "skins" of the menu tabs
    if ($start -lt 0) { continue }
    # the skins object ends with a top-level " }" (one space), with or without a comma
    $m = [regex]::Match($text.Substring($start), "`n \},?`n")
    $end = if ($m.Success) { $start + $m.Index } else { $text.Length }
    $block = $text.Substring($start, $end - $start)
    $was = $block

    # the heading
    $j = $block.IndexOf('"title"')
    if ($j -ge 0) {
        $k = $block.IndexOf("`n", $j)
        if ($k -lt 0) { $k = $block.Length }
        $block = $block.Substring(0, $j) + (Set-FirstNumber $block.Substring($j, $k - $j) $count) + $block.Substring($k)
    }

    # the bullet: "41 skins from Nikkomander ... and 14 new ones".
    # Both numbers show up as "41": the heading's "one click" has a 1 in it,
    # and 14 is in every page too, so the 41 is the one to look for.
    $b = [regex]::Match($block, '(?<![0-9\u09E6-\u09EF])(41|' + [char]0x09EA + [char]0x09E7 + ')(?![0-9\u09E6-\u09EF])')
    if ($b.Success) {
        $flat = $count - $painted
        $block = $block.Substring(0, $b.Index) + (Format-Like $flat $b.Value) + $block.Substring($b.Index + $b.Length)
    }

    if ($block -ne $was) {
        [System.IO.File]::WriteAllText($f.FullName, $text.Substring(0, $start) + $block + $text.Substring($end),
            (New-Object System.Text.UTF8Encoding($false)))
        Write-Host ('  ' + $f.Name)
        $touched++
    }
}
if ($touched -eq 0) { Write-Host '  every language already says the right number' -ForegroundColor Green }

# --- 5. the site ------------------------------------------------------------
if ($NoSite) {
    Write-Host '  -NoSite: the site was not rebuilt (run python website\build.py yourself)' -ForegroundColor Yellow
} else {
    $python = Find-Python
    if ($python) {
        Write-Host ("== {0} build.py ==" -f $python) -ForegroundColor Cyan
        Push-Location $siteDir
        try {
            & $python build.py
            if ($LASTEXITCODE -ne 0) { Write-Host 'The site build failed.' -ForegroundColor Red }
        } finally {
            Pop-Location
        }
    } else {
        # nothing found: the pictures and the list are ready all the same
        Write-Host '  Python could not be found, so website\build.py was not run.' -ForegroundColor Yellow
        Write-Host '  The pictures and skins.json are ready; the skins page shows them at the next site build.'
        Write-Host '  (Install it with:  winget install Python.Python.3.12)'
    }
}

Write-Host ''
Write-Host ("{0} skin tile(s) in {1}" -f $manifest.Count, $imgDir) -ForegroundColor Cyan
Write-Host ("raw captures kept in {0}" -f $stage)
