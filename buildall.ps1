<#
  buildall.ps1 - build everything that is published, and put it in binaries\

    binaries\nikpak.exe                    NikPak (command line + window), shareware
    binaries\unNik.exe                     unNik, the free .nik archive browser
    binaries\nikpak-<version>-setup.exe    NikPak setup (installs NikPak and unNik)
    binaries\unnik-<version>-setup.exe     unNik setup (unNik alone, freeware)
    binaries\SHA256SUMS.txt                checksums (for download sites)

  It also rebuilds the website (python website\build.py, so website\dist is
  always current), copies the built files into website\dist\downloads\, and
  uploads the public website\dist\ to the web host by FTP (overwriting),
  unless -NoUpload. The admin part is NOT uploaded: it stays offline.

  Usage (from the nikpak folder, in PowerShell):
      .\buildall.ps1              # build everything and upload the website
      .\buildall.ps1 -NoUpload    # build everything, skip the FTP upload
      .\buildall.ps1 -Test        # run the unit tests first
      .\buildall.ps1 -Clean       # empty binaries\ first (old versions are removed)
      .\buildall.ps1 -Version 1.14.0   # use this number and write it into every source
  If PowerShell refuses to run scripts:
      powershell -ExecutionPolicy Bypass -File .\buildall.ps1

  The version is read from Cargo.toml (or taken from -Version), written into
  Cargo.lock, website\site.json and website\version.json, and every file in
  binaries\, dist\ and website\dist\downloads\ that carries another version is
  deleted - only the version being built stays.
  buildwin.ps1 adds the per-build bump; use it for a normal delivery.

  Needs: Rust 1.89+ with the Visual Studio C++ Build Tools, Inno Setup 6.3+ and
  Python 3.8+ (winget install Rustlang.Rustup / JRSoftware.InnoSetup /
  Python.Python.3.12).
#>
param([switch]$Test, [switch]$Clean, [switch]$NoUpload, [string]$Version)

$ErrorActionPreference = 'Stop'
Set-Location -Path $PSScriptRoot
. (Join-Path $PSScriptRoot 'scripts\version.ps1')
$started = Get-Date

function Fail($msg) {
    Write-Host ''
    Write-Host $msg -ForegroundColor Red
    exit 1
}

function Step($msg) {
    Write-Host ''
    Write-Host "=== $msg" -ForegroundColor Cyan
}

# --- 1. Tools ---------------------------------------------------------------------
Step 'Checking the tools'
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
    if (Test-Path (Join-Path $cargoBin 'cargo.exe')) { $env:Path = "$cargoBin;$env:Path" }
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Fail ("Rust (cargo) was not found. Install it, open a NEW PowerShell window and run this script again:`n" +
          "    winget install Rustlang.Rustup`n" +
          "    winget install Microsoft.VisualStudio.2022.BuildTools --override `"--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`"")
}
$rv = (rustc --version) -replace '^rustc (\d+)\.(\d+).*$', '$1.$2'
if ([version]$rv -lt [version]'1.89') {
    Write-Host "Rust $rv is too old (1.89 or newer is needed): rustup update stable" -ForegroundColor Yellow
    rustup update stable
    if ($LASTEXITCODE -ne 0) { Fail 'rustup update failed.' }
}
Write-Host "Rust:       $(cargo --version)"

$iscc = (Get-Command iscc -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $iscc = @(
        (Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'),
        (Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe')
    ) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
}
if (-not $iscc) {
    Fail ("Inno Setup 6 was not found (it makes the two setups). Install it once, then run .\buildall.ps1 again:`n" +
          "    winget install JRSoftware.InnoSetup")
}
Write-Host "Inno Setup: $iscc"

# Python 3, to rebuild the website. The "py" launcher is tried with -3; only the
# standard library is used, so any Python 3.8+ will do.
$pythonExe = $null
$pythonArgs = @()
foreach ($cand in @(@('python'), @('py', '-3'), @('python3'))) {
    $cmd = Get-Command $cand[0] -ErrorAction SilentlyContinue
    if (-not $cmd) { continue }
    $extra = @()
    if ($cand.Count -gt 1) { $extra = $cand[1..($cand.Count - 1)] }
    $pyVer = $null
    try { $pyVer = (& $cmd.Source @extra --version 2>&1 | Out-String).Trim() } catch { continue }
    if ($LASTEXITCODE -eq 0 -and $pyVer -match '^Python 3\.') {
        $pythonExe = $cmd.Source
        $pythonArgs = $extra
        Write-Host "Python:     $pyVer  ($pythonExe)"
        break
    }
}
if (-not $pythonExe) {
    Fail ("Python 3 was not found (it rebuilds the website). Install it once, then run this script again:`n" +
          "    winget install Python.Python.3.12`n" +
          "  A new PowerShell window may be needed so the 'python' command is found.")
}

# Internal name $ver (not $version): PowerShell variable names are case-insensitive, so
# $version would be the same variable as the -Version parameter and overwrite it.
$ver = Get-NikpakVersion $PSScriptRoot
# -Version is authoritative: write it into every source file before anything is built.
if ($Version) {
    if ($Version -notmatch '^\d+\.\d+\.\d+$') { Fail "-Version '$Version' is not x.y.z (for example 1.14.0)." }
    if ($Version -ne $ver) {
        Write-Host "Version:    $ver -> $Version (writing it into every source)" -ForegroundColor Cyan
        Set-NikpakVersion $PSScriptRoot $Version
        $ver = $Version
    }
}
# The four machine-readable files must agree (fails otherwise; prose changelogs only warn).
Assert-NikpakVersionSync $PSScriptRoot $ver
Write-Host "Version:    $ver"

# --- 2. Tests (optional) ------------------------------------------------------------
if ($Test) {
    Step 'Unit tests'
    cargo test --release
    if ($LASTEXITCODE -ne 0) { Fail 'Tests failed: nothing was built.' }
}

# --- 3. nikpak.exe and unNik.exe -------------------------------------------------------
Step 'Building nikpak.exe and unNik.exe (release)'
cargo build --release --bin nikpak --bin unNik
if ($LASTEXITCODE -ne 0) {
    Fail ("Build failed. Most common cause: the Visual Studio C++ Build Tools are missing:`n" +
          "    winget install Microsoft.VisualStudio.2022.BuildTools --override `"--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`"")
}
# the setups take the programs from dist\
New-Item -ItemType Directory -Force -Path dist | Out-Null
Copy-Item -Force 'target\release\nikpak.exe' 'dist\nikpak.exe'
Copy-Item -Force 'target\release\unNik.exe' 'dist\unNik.exe'

# --- 4. The two setups ---------------------------------------------------------------
foreach ($iss in @('nikpak', 'unnik')) {
    Step "Building the $iss setup (installer\$iss.iss)"
    $out = "dist\$iss-$ver-setup.exe"
    if (Test-Path $out) { Remove-Item -Force $out }
    & $iscc /Q "/DMyAppVersion=$ver" "installer\$iss.iss"
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path $out)) { Fail "The $iss setup could not be built (installer\$iss.iss)." }
}

# --- 5. Copy everything to binaries\ ------------------------------------------------------
Step 'Copying to binaries\'
if ($Clean -and (Test-Path binaries)) {
    Get-ChildItem binaries -File | Remove-Item -Force
}
New-Item -ItemType Directory -Force -Path binaries | Out-Null
# Only the current version is kept: installers of older versions are removed from
# binaries\, dist\ and the website downloads, so they are never copied or uploaded again.
Remove-OldNikpakFiles -Root $PSScriptRoot -Version $ver -Dirs @('binaries', 'dist')
$files = @('nikpak.exe', 'unNik.exe', "nikpak-$ver-setup.exe", "unnik-$ver-setup.exe")
foreach ($f in $files) {
    Copy-Item -Force (Join-Path 'dist' $f) (Join-Path 'binaries' $f)
}

# checksums of what is published
$sums = foreach ($f in $files) {
    $h = (Get-FileHash -Algorithm SHA256 (Join-Path 'binaries' $f)).Hash.ToLower()
    "$h  $f"
}
Set-Content -Path 'binaries\SHA256SUMS.txt' -Value $sums -Encoding ascii

# --- 6. The website (python website\build.py) -----------------------------------------
# Rebuilds website\dist from i18n\, static\ and php\. It wipes dist\ first, so it
# must run BEFORE the downloads are copied into dist\downloads\ below.
Step 'Building the website (website\build.py)'
# $pythonExe / $pythonArgs were found in step 1.
& $pythonExe @pythonArgs (Join-Path $PSScriptRoot 'website\build.py')
if ($LASTEXITCODE -ne 0) { Fail 'The website build failed (website\build.py).' }

# --- 7. Website downloads -------------------------------------------------------------
# Every published build goes into website\dist\downloads\ so the site's download
# buttons (build.py / site.json) always point at real files. The Windows files come
# from this run; the macOS / Linux / .deb packages are added by buildmac.ps1 and
# buildunix.ps1, which copy their downloads here too.
Step 'Adding the downloads to the website'
$downloads = Join-Path $PSScriptRoot 'website\dist\downloads'
New-Item -ItemType Directory -Force -Path $downloads | Out-Null
# drop older versions from the downloads folder, then copy everything in binaries\
Remove-OldNikpakFiles -Root $PSScriptRoot -Version $ver -Dirs @('website\dist\downloads')
Get-ChildItem 'binaries' -File | Copy-Item -Destination $downloads -Force
Write-Host "  copied to $downloads"

# --- 8. Upload the website (FTP) ------------------------------------------------------
# Automatic upload of the PUBLIC site (with its downloads), overwriting the files
# on the server.
#
# The admin part stays OFFLINE on purpose: it is never uploaded by this script.
# Only website\dist (the public pages) goes out. Do not add an admin folder to
# $FtpUploads below.
$FtpHost = '65.21.192.14'
$FtpUser = 'NikkoD'
$FtpPass = 'Akaklike666'
$FtpUploads = @(
    @{ Local = (Join-Path $PSScriptRoot 'website\dist'); Remote = '/http/NikPak.nikkoid.com' }
)

# upload this many files at once, each on its own FTP connection
$FtpWorkers = 10
# upload only what changed: skip the files the server already has (same size, same date)
. (Join-Path $PSScriptRoot 'scripts\ftp-changed.ps1')
$FtpStatePath = Join-Path $PSScriptRoot '.ftp-state.json'

function New-FtpRequest([string]$url, $creds, [string]$method) {
    $req = [System.Net.WebRequest]::Create($url)
    if ($req -isnot [System.Net.FtpWebRequest]) { throw "this PowerShell cannot make FTP requests (run it in Windows PowerShell 5.1)" }
    $req.Credentials = $creds
    $req.Method = $method
    $req.UseBinary = $true
    $req.UsePassive = $true
    $req.KeepAlive = $true   # keep the control connection open between files
    $req.EnableSsl = $false
    $req.Timeout = 60000
    $req.ReadWriteTimeout = 600000
    return $req
}

function New-FtpDir([string]$url, $creds) {
    try {
        $req = New-FtpRequest $url $creds ([System.Net.WebRequestMethods+Ftp]::MakeDirectory)
        $req.GetResponse().Close()
    } catch { }   # 550 "already exists" is the normal case
}

# directories already created this run (deduped), so a site with hundreds of files
# does not re-issue a MKD round-trip for the same parent folders
$script:MadeDirs = New-Object 'System.Collections.Generic.HashSet[string]'

function Ensure-FtpPath([string]$base, [string]$rel, $creds) {
    $cur = $base.TrimEnd('/')
    foreach ($seg in ($rel -split '/')) {
        if ($seg -eq '') { continue }
        $cur = "$cur/$seg"
        if ($script:MadeDirs.Contains($cur)) { continue }
        New-FtpDir $cur $creds
        [void]$script:MadeDirs.Add($cur)
    }
}

function Write-FtpProgress([int]$done, [int]$count, [long]$sent, [long]$total, [int]$active) {
    # one line, overwritten in place: files done / total, a byte bar, percent, MB, in-flight count
    $w = 26
    $filled = if ($total -gt 0) { [int][math]::Floor($w * $sent / $total) } else { $w }
    if ($filled -lt 0) { $filled = 0 }
    if ($filled -gt $w) { $filled = $w }
    $bar = '[' + ('#' * $filled) + (' ' * ($w - $filled)) + ']'
    $pct = if ($total -gt 0) { [int](100 * $sent / $total) } else { 100 }
    $mb = if ($total -gt 0) { ('{0:N1}/{1:N1} MB' -f ($sent / 1MB), ($total / 1MB)) } else { '' }
    Write-Host -NoNewline ("`r  {0,4}/{1,-4} {2} {3,3}%  {4}  ({5} uploading)    " -f $done, $count, $bar, $pct, $mb, $active)
}

if (-not $NoUpload) {
    Step 'Uploading the website (FTP)'
    Write-Host '  public site only - the admin part stays offline' -ForegroundColor DarkGray
    $creds = New-Object System.Net.NetworkCredential($FtpUser, $FtpPass)
    foreach ($u in $FtpUploads) {
        if (-not (Test-Path $u.Local)) {
            Write-Host "  missing: $($u.Local)  (run: python website\build.py)" -ForegroundColor Yellow
            continue
        }
        $localDir = (Resolve-Path $u.Local).Path
        $remoteRel = $u.Remote.Trim('/')
        Ensure-FtpPath "ftp://$FtpHost" $remoteRel $creds
        $ftpUrl = "ftp://$FtpHost/$remoteRel"
        Write-Host "  $localDir  ->  $ftpUrl"
        $upFiles = @(Get-ChildItem -LiteralPath $localDir -Recurse -File)
        $fileCount = $upFiles.Count
        $totalBytes = [long]0
        if ($fileCount -gt 0) { $totalBytes = [long](($upFiles | Measure-Object -Property Length -Sum).Sum) }

        # allow one connection per worker (the .NET default of 2 would silently cap the parallelism)
        [System.Net.ServicePointManager]::DefaultConnectionLimit = $FtpWorkers

        # flatten the file list (relative path, size, upload URL) and ensure each directory once
        $entries = @(foreach ($f in $upFiles) {
            $rel = $f.FullName.Substring($localDir.Length).TrimStart('\', '/').Replace('\', '/')
            $relDir = if ($rel.Contains('/')) { $rel.Substring(0, $rel.LastIndexOf('/')) } else { '' }
            if ($relDir) { Ensure-FtpPath $ftpUrl $relDir $creds }
            [pscustomobject]@{ FullName = $f.FullName; Rel = $rel; Size = $f.Length; Url = "$ftpUrl/$rel" }
        })

        $plan = Select-FtpChanged -Entries $entries -LocalProperty FullName -UrlProperty Url -Credential $creds -StatePath $FtpStatePath -Workers $FtpWorkers
        $entries = @($plan.Upload)
        $fileCount = $entries.Count
        $totalBytes = [long]0
        if ($fileCount -gt 0) { $totalBytes = [long](($entries | Measure-Object -Property Size -Sum).Sum) }
        $okRel = New-Object 'System.Collections.Generic.HashSet[string]'
        $sync = [hashtable]::Synchronized(@{ bytesSent = [long]0 })
        $chunk = 256 * 1024

        # one worker uploads one file; self-contained because it runs in a separate runspace
        $worker = {
            param($e, $creds, $chunk, $sync)
            $ok = $true; $err = ''
            try {
                $req = [System.Net.WebRequest]::Create($e.Url)
                if ($req -isnot [System.Net.FtpWebRequest]) { throw "this PowerShell cannot make FTP requests (run it in Windows PowerShell 5.1)" }
                $req.Credentials = $creds
                $req.Method = [System.Net.WebRequestMethods+Ftp]::UploadFile
                $req.UseBinary = $true
                $req.UsePassive = $true
                $req.KeepAlive = $true
                $req.EnableSsl = $false
                $req.Timeout = 60000
                $req.ReadWriteTimeout = 600000
                $req.ContentLength = $e.Size
                $fs = [System.IO.File]::OpenRead($e.FullName)
                try {
                    $s = $req.GetRequestStream()
                    try {
                        $buf = New-Object byte[] $chunk
                        while (($n = $fs.Read($buf, 0, $chunk)) -gt 0) {
                            $s.Write($buf, 0, $n)
                            $sync.bytesSent += $n
                        }
                    } finally { $s.Close() }
                } finally { $fs.Close() }
                $req.GetResponse().Close()
            } catch {
                $ok = $false
                $err = $_.Exception.Message
            }
            [pscustomobject]@{ rel = $e.Rel; ok = $ok; err = $err }
        }

        $pool = [runspacefactory]::CreateRunspacePool(1, $FtpWorkers)
        $pool.Open()
        $jobs = @(foreach ($e in $entries) {
            $ps = [powershell]::Create()
            $ps.RunspacePool = $pool
            [void]$ps.AddScript($worker).AddArgument($e).AddArgument($creds).AddArgument($chunk).AddArgument($sync)
            [pscustomobject]@{ ps = $ps; handle = $ps.BeginInvoke(); rel = $e.Rel }
        })

        # keep one line on screen until the last file is done
        while ($true) {
            $remaining = 0
            foreach ($j in $jobs) { if (-not $j.handle.IsCompleted) { $remaining++ } }
            if ($remaining -eq 0) { break }
            Write-FtpProgress ($fileCount - $remaining) $fileCount ([long]$sync.bytesSent) $totalBytes ([math]::Min($FtpWorkers, $remaining))
            Start-Sleep -Milliseconds 100
        }
        Write-Host ''   # move past the progress line

        $total = 0
        $errors = @()
        foreach ($j in $jobs) {
            try {
                $out = $j.ps.EndInvoke($j.handle)
                foreach ($r in $out) {
                    if ($r.ok) { $total++; [void]$okRel.Add($r.rel) } else { $errors += "$($r.rel) : $($r.err)" }
                }
            } catch {
                $errors += "$($j.rel) : $($_.Exception.Message)"
            }
            $j.ps.Dispose()
        }
        $pool.Close(); $pool.Dispose()
        Save-FtpState -Plan $plan -Uploaded @($entries | Where-Object { $okRel.Contains($_.Rel) })

        $msg = "{0} files uploaded" -f $total
        if ($errors.Count) { $msg += ", $($errors.Count) FAILED" }
        Write-Host "  $msg" -ForegroundColor Green
        foreach ($e in $errors) { Write-Host "    FAILED  $e" -ForegroundColor Red }
    }
} else {
    Write-Host ''
    Write-Host 'Upload skipped (-NoUpload).' -ForegroundColor Yellow
}

# --- 9. Summary ------------------------------------------------------------------------
Write-Host ''
Write-Host "Done in $([math]::Round(((Get-Date) - $started).TotalMinutes, 1)) min - NikPak $ver" -ForegroundColor Green
Write-Host ''
Write-Host 'Files generated:' -ForegroundColor Cyan
foreach ($f in $files + 'SHA256SUMS.txt') {
    $p = Join-Path $PSScriptRoot "binaries\$f"
    $mb = [math]::Round((Get-Item $p).Length / 1MB, 2)
    Write-Host ("  {0,8} MB  {1}" -f $mb, $p) -ForegroundColor Green
}
Write-Host ''
Write-Host 'Before publishing, sign the .exe files with your code-signing certificate (SmartScreen).'
