<#
  site-shots.ps1 - recapture the pictures of the web site from the real program.

  The pictures in website\static\img\ are screenshots of NikPak and unNik. This
  script starts the current build with the test hooks that put it in each state
  and saves the window of each state as a PNG in website\shots-raw\:

      nikpak-drop        the window, waiting for a drop
      nikpak-compress    compressing a folder (progress ring, percentage)
      nikpak-choose      the "choose files / choose a folder" buttons
      nikpak-about       the About window
      nikpak-reminder    the shareware reminder
      nikpak-register    the registration form
      skins-menu         the skins box beside the window (Nikkoid Blue)
      skins-menu-bright  the same in Nikkoid Bright
      unnik-idle         unNik, waiting for an archive
      unnik-browser      unNik showing an archive
      unnik-skins        unNik with the skins box
      unnik-password     unNik asking for the password of an encrypted archive
      unnik-extract      unNik with its Extract box (--extract)
      lang-en-menu       English, language menu open
      lang-zh            Chinese
      lang-hi-nag        Hindi, shareware reminder
      lang-ar            Arabic (right to left)

  Usage, from the nikpak folder in PowerShell:

      .\tools\site-shots.ps1              # build, then capture
      .\tools\site-shots.ps1 -NoBuild     # use the exe already built

  Then tell Claude: the PNGs are converted to .webp, and the two composite
  pictures of the site (lang-quad, unnik-pair) are assembled from them.

  Notes: keep the desktop unlocked and the window in front. The compressing
  shot writes a demonstration archive into NikPak's usual destination folder
  (shown in the window, normally the Desktop) - delete it afterwards.
#>
param([switch]$NoBuild)

$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
$root = Get-Location

# The pictures must show the program as users normally see it. NikPak / unNik
# refuse to run their window as administrator (they relaunch it through
# Explorer), which both breaks the capture and puts a "Running as
# administrator" warning in the window. If this script was started elevated,
# relaunch it as a normal user by asking Explorer (which always runs in the
# normal user's context) to run a one-line launcher, then exit. NIKPAK_DEELEVATE
# stops a relaunch loop when Windows cannot drop the rights (UAC off).
$principal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    if ($env:NIKPAK_DEELEVATE -eq '1') {
        Write-Host 'Could not drop administrator rights (is UAC disabled?).' -ForegroundColor Red
        Write-Host 'Run this script from a normal (non-administrator) PowerShell instead.'
        exit 1
    }
    Write-Host 'Running elevated - relaunching this script as a normal user via Explorer...' -ForegroundColor Yellow
    $parts = @()
    foreach ($k in $PSBoundParameters.Keys) {
        $v = $PSBoundParameters[$k]
        if ($v -is [switch]) {
            if ($v) { $parts += "-$k" }
        } else {
            $parts += "-$k"
            $parts += ('"' + $v + '"')
        }
    }
    $argLine = $parts -join ' '
    $launcher = Join-Path $env:TEMP ('nikpak-deelevate-' + [guid]::NewGuid().ToString('N') + '.cmd')
    $body = "@echo off`r`nset NIKPAK_DEELEVATE=1`r`npowershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$($MyInvocation.MyCommand.Path)`" $argLine`r`ndel `"%~f0`" >nul 2>&1`r`n"
    Set-Content -Path $launcher -Value $body -Encoding Ascii
    Start-Process explorer.exe -ArgumentList ('"' + $launcher + '"')
    exit 0
}

# Close any NikPak / unNik windows left over from an earlier (interrupted) run,
# so they cannot be captured by mistake or hold the screen.
Get-Process -Name nikpak, unNik -ErrorAction SilentlyContinue | Stop-Process -Force

$out = Join-Path $root 'website\shots-raw'
New-Item -ItemType Directory -Force -Path $out | Out-Null

# --- build -----------------------------------------------------------------
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
    if ($LASTEXITCODE -ne 0) { Write-Host 'Build failed.' -ForegroundColor Red; exit 1 }
}

$nikpak = Join-Path $root 'target\release\nikpak.exe'
$unnik = Join-Path $root 'target\release\unNik.exe'
foreach ($e in @($nikpak, $unnik)) {
    if (-not (Test-Path $e)) { Write-Host "Not found: $e" -ForegroundColor Red; exit 1 }
}

# --- a demo folder to compress, and an archive for unNik --------------------
$demo = Join-Path $env:TEMP 'nikpak-site-demo'
if (Test-Path $demo) { Remove-Item -Recurse -Force $demo }
New-Item -ItemType Directory -Force -Path $demo | Out-Null
# a few megabytes of real files, so the progress ring is still moving
foreach ($d in @('src', 'assets\fonts')) {
    $src = Join-Path $root $d
    if (Test-Path $src) { Copy-Item -Recurse -Force $src $demo }
}
1..2 | ForEach-Object {
    $c = Join-Path $demo "photos-$_"
    New-Item -ItemType Directory -Force -Path $c | Out-Null
    Copy-Item (Join-Path $root 'website\static\img\*.webp') $c -ErrorAction SilentlyContinue
}
$archive = Join-Path $out 'demo.nik'
if (Test-Path $archive) { Remove-Item -Force $archive }
Write-Host '== making the demonstration archive for unNik =='
& $nikpak a $archive (Join-Path $demo 'src') | Out-Null

# an encrypted copy, so unNik shows its password box for the dialogs picture
$encrypted = Join-Path $out 'encrypted-demo.nik'
if (Test-Path $encrypted) { Remove-Item -Force $encrypted }
& $nikpak a "-pNikPakDemo1" $encrypted (Join-Path $demo 'src') | Out-Null

# --- window capture --------------------------------------------------------
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class SiteShot {
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
    public static IntPtr Biggest(int pid) {
        IntPtr best = IntPtr.Zero; long area = 0;
        EnumWindows(delegate(IntPtr h, IntPtr l) {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != (uint)pid || !IsWindowVisible(h) || GetParent(h) != IntPtr.Zero) return true;
            RECT r; if (!GetWindowRect(h, out r)) return true;
            long w = r.Right - r.Left, hh = r.Bottom - r.Top;
            if (w * hh > area) { area = w * hh; best = h; }
            return true;
        }, IntPtr.Zero);
        return best;
    }
    public static void Raise(IntPtr h) { SetWindowPos(h, new IntPtr(-1), 0, 0, 0, 0, 0x0003); }
    public static void Unraise(IntPtr h) { SetWindowPos(h, new IntPtr(-2), 0, 0, 0, 0, 0x0003); }
}
"@
[SiteShot]::SetProcessDPIAware() | Out-Null

function Wait-Window($proc) {
    for ($i = 0; $i -lt 200; $i++) {
        $proc.Refresh()
        if ($proc.HasExited) { return [IntPtr]::Zero }
        $h = [SiteShot]::Biggest($proc.Id)
        if ($h.ToInt64() -ne 0) {
            $r = [SiteShot+RECT]::new()
            [SiteShot]::GetWindowRect($h, [ref]$r) | Out-Null
            if (($r.Right - $r.Left) -ge 400 -and ($r.Bottom - $r.Top) -ge 400) { return $h }
        }
        Start-Sleep -Milliseconds 100
    }
    return [IntPtr]::Zero
}

function Save-Shot($h, $file) {
    [SiteShot]::ShowWindow($h, 5) | Out-Null
    [SiteShot]::SetForegroundWindow($h) | Out-Null
    [SiteShot]::Raise($h)
    Start-Sleep -Milliseconds 600
    $r = [SiteShot+RECT]::new()
    [SiteShot]::GetWindowRect($h, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left
    $hh = $r.Bottom - $r.Top
    if ($w -le 0 -or $hh -le 0) { throw 'the window has no size' }
    $bmp = New-Object System.Drawing.Bitmap $w, $hh
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try {
        $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $hh))
        $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $g.Dispose(); $bmp.Dispose(); [SiteShot]::Unraise($h) }
}

# --- one state -------------------------------------------------------------
function Shot($name, [string[]]$argv, [hashtable]$vars, $waitMs = 1800) {
    Remove-Item Env:NIKPAK_GUI_SKIN, Env:NIKPAK_GUI_SKINMENU, Env:NIKPAK_GUI_CHOOSER, Env:NIKPAK_GUI_MODAL, `
                Env:NIKPAK_GUI_LANG, Env:NIKPAK_GUI_LANGMENU, Env:NIKPAK_GUI_DEMO, Env:NIKPAK_GUI_HOVER -ErrorAction SilentlyContinue
    $env:NIKPAK_GUI_NO_NAG = '1'          # the reminder has its own shot
    if ($vars) { foreach ($k in $vars.Keys) { Set-Item "Env:$k" $vars[$k] } }
    if ($vars -and $vars.ContainsKey('NIKPAK_GUI_MODAL')) { Remove-Item Env:NIKPAK_GUI_NO_NAG -ErrorAction SilentlyContinue }
    Write-Host ("  {0}" -f $name) -NoNewline
    $exe = if ($argv.Count -gt 0 -and $argv[0] -eq 'unnik') { $unnik } else { $nikpak }
    $args2 = @()
    if ($argv.Count -gt 1) { $args2 = $argv[1..($argv.Count - 1)] | ForEach-Object { '"' + $_ + '"' } }
    if ($args2.Count -gt 0) {
        $p = Start-Process -FilePath $exe -ArgumentList $args2 -PassThru
    } else {
        $p = Start-Process -FilePath $exe -PassThru
    }
    try {
        $h = Wait-Window $p
        if ($h.ToInt64() -eq 0) {
            if ($p.HasExited) { throw ("exited at once (code {0})" -f $p.ExitCode) }
            throw 'the window did not appear'
        }
        Start-Sleep -Milliseconds $waitMs
        Save-Shot $h (Join-Path $out "$name.png")
        Write-Host '  ok' -ForegroundColor Green
    } catch {
        Write-Host ("  FAILED: {0}" -f $_.Exception.Message) -ForegroundColor Red
    } finally {
        if (-not $p.HasExited) { $p.CloseMainWindow() | Out-Null; Start-Sleep -Milliseconds 400; if (-not $p.HasExited) { $p.Kill() } }
        $p.Dispose()
        Remove-Item Env:NIKPAK_GUI_SKIN, Env:NIKPAK_GUI_SKINMENU, Env:NIKPAK_GUI_CHOOSER, Env:NIKPAK_GUI_MODAL, `
                    Env:NIKPAK_GUI_LANG, Env:NIKPAK_GUI_LANGMENU, Env:NIKPAK_GUI_DEMO -ErrorAction SilentlyContinue
    }
}

Write-Host "== pictures -> $out ==" -ForegroundColor Cyan
Shot 'nikpak-drop'     @()                                   @{}
Shot 'nikpak-compress' @()                                   @{ 'NIKPAK_GUI_DEMO' = $demo } 2600
Shot 'nikpak-choose'   @()                                   @{ 'NIKPAK_GUI_CHOOSER' = '1' }
Shot 'nikpak-about'    @()                                   @{ 'NIKPAK_GUI_MODAL' = 'about' }
Shot 'nikpak-reminder' @()                                   @{ 'NIKPAK_GUI_MODAL' = 'nag-left' }
Shot 'nikpak-register' @()                                   @{ 'NIKPAK_GUI_MODAL' = 'register' }
Shot 'skins-menu'      @()                                   @{ 'NIKPAK_GUI_SKIN' = 'moon'; 'NIKPAK_GUI_SKINMENU' = '1' } 2600
Shot 'skins-menu-bright' @()                                 @{ 'NIKPAK_GUI_SKIN' = 'nikkoid-light'; 'NIKPAK_GUI_SKINMENU' = '1' } 2600
Shot 'unnik-idle'      @('unnik')                            @{}
Shot 'unnik-browser'   @('unnik', $archive)                  @{} 2600
Shot 'unnik-skins'     @('unnik', $archive)                  @{ 'NIKPAK_GUI_SKINMENU' = '1' } 2600
Shot 'unnik-password'  @('unnik', $encrypted)                @{} 2600
Shot 'unnik-extract'   @('unnik', '--extract', $archive)     @{} 3000
Shot 'lang-en-menu'    @()                                   @{ 'NIKPAK_GUI_LANGMENU' = '1' }
Shot 'lang-zh'         @()                                   @{ 'NIKPAK_GUI_LANG' = 'zh' }
Shot 'lang-hi-nag'     @()                                   @{ 'NIKPAK_GUI_LANG' = 'hi'; 'NIKPAK_GUI_MODAL' = 'nag-left' }
Shot 'lang-ar'         @()                                   @{ 'NIKPAK_GUI_LANG' = 'ar' }

Write-Host ''
Write-Host "Pictures are in $out" -ForegroundColor Cyan

# --- turn them into the .webp files of the site ----------------------------
$py = Get-Command python -ErrorAction SilentlyContinue
if ($py) {
    Write-Host '== webp conversions ==' -ForegroundColor Cyan
    & $py.Source (Join-Path $root 'tools\site-shots-to-webp.py')
    Write-Host 'Rebuild the site with:  python website\build.py' -ForegroundColor Cyan
} else {
    Write-Host 'Python was not found: tell Claude, it converts the pictures.'
}
