<#
  build-shots.ps1 - build nikpak and capture the skins as PNG files.

  This makes the screenshots of a release without opening every skin by hand:

    1. runs cargo build --release (skip with -NoBuild),
    2. dumps the painted background of every skin through the
       dump_backgrounds test hook (PPM files),
    3. starts target\release\nikpak.exe once per new skin with the
       NIKPAK_GUI_SKIN test hook, waits for the window, and captures it,
    4. captures the skins menu (NIKPAK_GUI_SKINMENU) over the Moon skin.

  Usage, from the nikpak folder in PowerShell:

      .\tools\build-shots.ps1              # build, then 15 screenshots
      .\tools\build-shots.ps1 -NoBuild     # use the exe already built
      .\tools\build-shots.ps1 -Test        # run the unit tests first

  The PNGs go to screenshots\<version>\ (00-skins-menu.png, 01-moon.png, ...)
  and the backgrounds to screenshots\<version>\backgrounds\.
  An elevated PowerShell works too: the capture part is then run by a
  normal-user task (tools\unelevated.ps1), because NikPak never keeps running
  elevated.
  Keep the window in front and the desktop unlocked while it runs: the shots
  are taken with CopyFromScreen, which captures whatever is on screen.
#>
param(
    [switch]$NoBuild,
    [switch]$Test,
    [string]$OutDir,
    # set by tools\unelevated.ps1 on the copy that runs as the normal user
    [switch]$Inner,
    [string]$Root
)

$ErrorActionPreference = 'Stop'
$root = if ($Root) { $Root } else { Split-Path $PSScriptRoot -Parent }
Set-Location $root
. (Join-Path $root 'tools\unelevated.ps1')

# the copy that runs as the normal user writes what it does to this file
if ($Inner) { Start-Transcript -Path $script:UnelevatedLog -Force | Out-Null }

# Elevation is not a problem: NIKPAK_GUI_CAPTURE tells NikPak / unNik to stay in
# the process that was started instead of restarting itself through Explorer, so
# the shell that started them - elevated or not - owns the window and can copy
# it off the screen.
$env:NIKPAK_GUI_CAPTURE = '1'

# --- 1. cargo (in this shell, before the capture is handed over) -------------
if (-not $NoBuild -and -not $Inner) {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
        if (Test-Path (Join-Path $cargoBin 'cargo.exe')) { $env:Path = "$cargoBin;$env:Path" }
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host 'Rust (cargo) was not found. Install it with:  winget install Rustlang.Rustup' -ForegroundColor Red
        exit 1
    }
    if ($Test) {
        Write-Host '== cargo test --release ==' -ForegroundColor Cyan
        cargo test --release
        if ($LASTEXITCODE -ne 0) { Write-Host 'Tests failed - no screenshots.' -ForegroundColor Red; exit 1 }
    }
    Write-Host '== cargo build --release ==' -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Write-Host 'Build failed - no screenshots.' -ForegroundColor Red; exit 1 }
}

$exe = Join-Path $root 'target\release\nikpak.exe'
if (-not (Test-Path $exe)) { Write-Host "Not found: $exe" -ForegroundColor Red; exit 1 }

# --- 2. window capture ------------------------------------------------------
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Shot32 {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int cx, int cy, uint f);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern IntPtr GetParent(IntPtr h);
    delegate bool EnumProc(IntPtr h, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }

    /// Raise the window above the others without needing the focus, and put
    /// it back afterwards (the capture is a plain screen copy).
    public static void Raise(IntPtr h) { SetWindowPos(h, new IntPtr(-1), 0, 0, 0, 0, 0x0003); }
    public static void Unraise(IntPtr h) { SetWindowPos(h, new IntPtr(-2), 0, 0, 0, 0, 0x0003); }

    /// The biggest visible top-level window of the process: Process.MainWindowHandle
    /// can pick a small helper window, which is what broke the first screenshots.
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
[Shot32]::SetProcessDPIAware() | Out-Null

function Get-Window($proc) {
    # the window only counts once it is really laid out (a window that is not
    # shown yet can report a 22 x 22 rect)
    for ($i = 0; $i -lt 200; $i++) {
        $proc.Refresh()
        if ($proc.HasExited) { return [IntPtr]::Zero }
        $h = [Shot32]::BiggestWindow($proc.Id)
        if ($h.ToInt64() -ne 0) {
            $r = [Shot32+RECT]::new()
            [Shot32]::GetWindowRect($h, [ref]$r) | Out-Null
            if (($r.Right - $r.Left) -ge 400 -and ($r.Bottom - $r.Top) -ge 400) { return $h }
        }
        Start-Sleep -Milliseconds 100
    }
    return [IntPtr]::Zero
}

function Save-Shot($h, $file) {
    [Shot32]::ShowWindow($h, 5) | Out-Null
    [Shot32]::SetForegroundWindow($h) | Out-Null
    [Shot32]::Raise($h)
    Start-Sleep -Milliseconds 700
    $r = [Shot32+RECT]::new()
    [Shot32]::GetWindowRect($h, [ref]$r) | Out-Null
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
        [Shot32]::Unraise($h)
    }
}

function Invoke-Shot($name, $skin, $menu, $file) {
    $env:NIKPAK_GUI_NO_NAG = '1'
    if ($skin) { $env:NIKPAK_GUI_SKIN = $skin } else { Remove-Item Env:NIKPAK_GUI_SKIN -ErrorAction SilentlyContinue }
    if ($menu) { $env:NIKPAK_GUI_SKINMENU = '1' } else { Remove-Item Env:NIKPAK_GUI_SKINMENU -ErrorAction SilentlyContinue }
    Write-Host ("  {0}" -f $name) -NoNewline
    $p = Start-Process -FilePath $exe -PassThru
    try {
        $h = Get-Window $p
        if ($h.ToInt64() -eq 0) {
            if ($p.HasExited) { throw ("the program exited at once (code {0})" -f $p.ExitCode) }
            throw 'the window did not appear'
        }
        # the painted background of a skin is built on the first frames, and
        # the window widens itself when the skins box opens
        Start-Sleep -Milliseconds 2500
        Save-Shot $h $file
        # a captured window that the desktop covered is one flat colour: retry
        if ((Get-Item $file).Length -lt 4000) {
            Start-Sleep -Milliseconds 800
            Save-Shot $h $file
        }
        Write-Host '  ok' -ForegroundColor Green
    } catch {
        Write-Host ("  FAILED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    } finally {
        if (-not $p.HasExited) {
            $p.CloseMainWindow() | Out-Null
            Start-Sleep -Milliseconds 500
            if (-not $p.HasExited) { $p.Kill() }
        }
        $p.Dispose()
        Remove-Item Env:NIKPAK_GUI_SKIN -ErrorAction SilentlyContinue
        Remove-Item Env:NIKPAK_GUI_SKINMENU -ErrorAction SilentlyContinue
    }
}

# --- 3. the shots -----------------------------------------------------------
$version = (Select-String -Path 'Cargo.toml' -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
if (-not $OutDir) { $OutDir = "screenshots\$version" }
$dir = Join-Path $root $OutDir
New-Item -ItemType Directory -Force -Path $dir | Out-Null

# the painted backgrounds alone, through the dump_backgrounds test hook: they
# are proof the artwork renders even if a GPU window cannot be captured
$bg = Join-Path $dir 'backgrounds'
New-Item -ItemType Directory -Force -Path $bg | Out-Null
Write-Host '== painted backgrounds (PPM) ==' -ForegroundColor Cyan
$env:NIKPAK_SKIN_DUMP = $bg
cargo test --release --lib dump_backgrounds -- --ignored | Out-Host
Remove-Item Env:NIKPAK_SKIN_DUMP -ErrorAction SilentlyContinue

Write-Host "== screenshots -> $dir ==" -ForegroundColor Cyan
$skins = @(
    @('moon', 'Moon'), @('america', 'America'), @('pattaya', 'Pattaya Neons'),
    @('burgers', 'Burgers Tokyo'), @('sushi', 'Sushi'), @('amiga', 'Amiga Computer'),
    @('msx', 'MSX Computer'), @('colecovision', 'Colecovision'), @('zx-spectrum', 'ZX Spectrum'),
    @('catlife', 'Cat Life'), @('doglife', 'Dog Life'), @('mario', 'Mario Bros'),
    @('cs', 'Counter Strike'), @('chess', 'Chess')
)

Invoke-Shot 'skins menu (over Moon)' 'moon' $true (Join-Path $dir '00-skins-menu.png')
$n = 1
foreach ($s in $skins) {
    $file = Join-Path $dir ('{0:d2}-{1}.png' -f $n, $s[0])
    Invoke-Shot $s[1] $s[0] $false $file
    $n++
}

Write-Host ''
Write-Host ("{0} PNG files in {1}" -f (($n)), $dir) -ForegroundColor Cyan
Write-Host 'Tell Claude the shots are ready: it reads them straight from that folder.'
