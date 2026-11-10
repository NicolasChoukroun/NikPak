<#
.SYNOPSIS
  NikPak - rebuild the web site and upload it (with the downloads) to the server.

.DESCRIPTION
  No build, no Inno - just the web site. It:
    1. runs  python website\build.py  which rebuilds website\dist (public pages only);
    2. removes installers of older versions from binaries\ and copies only the current
       version's files into website\dist\downloads\;
    3. uploads website\dist to the web host by FTP (overwriting), 10 files at a time,
       with one inline progress line;
    4. removes downloads of older versions from the server, so only the latest build
       is ever offered.

  Run it after buildwin.ps1 + buildmac.ps1 + buildunix.ps1 when every platform is
  built: it publishes the current version on the web site without rebuilding anything.

  Before publishing it checks that Cargo.toml, Cargo.lock, website\site.json and
  website\version.json all carry the same version. A mismatch stops the run, so the
  site can never advertise a download that does not exist (or hide a newer release).

.EXAMPLE
  .\buildweb.ps1
  powershell -ExecutionPolicy Bypass -File buildweb.ps1      (if scripts are blocked)
#>

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot
. (Join-Path $PSScriptRoot 'scripts\version.ps1')

function Step([string]$msg) { Write-Host ''; Write-Host "== $msg" -ForegroundColor Cyan }
function Fail([string]$msg) { Write-Host ''; Write-Host $msg -ForegroundColor Red; exit 1 }

# The last gate before publishing: the four source files must agree, or the download
# links ({v} in site.json) and the update manifest would point at another release.
$version = Get-NikpakVersion $PSScriptRoot
Step "Checking the version ($version)"
try {
    Assert-NikpakVersionSync $PSScriptRoot $version
} catch {
    Fail ("Not publishing: $($_.Exception.Message)`n" +
          "Fix the version (or run buildwin.ps1, which writes it everywhere), then run this script again.")
}
Write-Host "  every source file says $version"

# --- 1. Python 3 -----------------------------------------------------------------
Step 'Checking Python 3'
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
if (-not $pythonExe) { Fail ("Python 3 was not found (it rebuilds the website). Install it once:`n    winget install Python.Python.3.12") }

# --- 2. The website (python website\build.py) ------------------------------------
Step 'Building the website (website\build.py)'
& $pythonExe @pythonArgs (Join-Path $PSScriptRoot 'website\build.py')
if ($LASTEXITCODE -ne 0) { Fail 'The website build failed (website\build.py).' }

# --- 3. Website downloads --------------------------------------------------------
# Only the current version is ever published: installers of older versions are removed
# from binaries\ and from the downloads folder first, so stale builds are never uploaded.
Step 'Adding the downloads to the website'
$downloads = Join-Path $PSScriptRoot 'website\dist\downloads'
New-Item -ItemType Directory -Force -Path $downloads | Out-Null
Remove-OldNikpakFiles -Root $PSScriptRoot -Version $version -Dirs @('binaries', 'dist', 'website\dist\downloads')
if (Test-Path 'binaries') {
    Get-ChildItem 'binaries' -File | Copy-Item -Destination $downloads -Force
    Write-Host "  copied $(@(Get-ChildItem 'binaries' -File).Count) file(s) to $downloads (NikPak $version)"
} else {
    Write-Host '  binaries\ is empty: the download page will have no files' -ForegroundColor Yellow
}

# Every download button of site.json must have its file, or the site would offer a
# 404. The usual cause is publishing before buildmac.ps1 / buildunix.ps1 have run.
$expected = @(
    "nikpak-$version-setup.exe", "unnik-$version-setup.exe",
    "nikpak-$version-macos.zip", "nikpak-$version-linux-x86_64.tar.gz",
    "nikpak_${version}_amd64.deb", "unnik_${version}_amd64.deb"
)
$missing = @($expected | Where-Object { -not (Test-Path -LiteralPath (Join-Path $downloads $_)) })
if ($missing.Count) {
    Write-Host ''
    Write-Host '  WARNING: these downloads of the current version are missing,' -ForegroundColor Yellow
    Write-Host '  so their buttons will lead to a 404 on the site:' -ForegroundColor Yellow
    foreach ($m in $missing) { Write-Host "     $m" -ForegroundColor Yellow }
    Write-Host '  Build them first: buildmac.ps1 / buildunix.ps1 (or buildwin.ps1 for the setups).' -ForegroundColor Yellow
    Write-Host '  Continuing in 5 s - press Ctrl+C to stop.' -ForegroundColor DarkYellow
    Start-Sleep -Seconds 5
}

# --- 4. Upload the website (FTP) -------------------------------------------------
# The credentials live in ftp-config.ps1 (git-ignored). If it is missing, copy
# ftp-config.example.ps1 to ftp-config.ps1 and fill in the host, user, password and base path.
$FtpConfig = Join-Path $PSScriptRoot 'ftp-config.ps1'
if (-not (Test-Path -LiteralPath $FtpConfig)) {
    throw "Missing $FtpConfig - copy ftp-config.example.ps1 to ftp-config.ps1 and fill in the FTP host, user, password and base path."
}
. $FtpConfig

# upload this many files at once, each on its own FTP connection
$FtpWorkers = 10
# upload only what changed: skip the files the server already has (same size, same date)
. (Join-Path $PSScriptRoot 'scripts\ftp-changed.ps1')
$FtpStatePath = Join-Path $PSScriptRoot '.ftp-state.json'

function Get-FtpUrl([string]$remotePath) {
    $encoded = (($remotePath.Trim('/').Split('/') | ForEach-Object { [System.Uri]::EscapeDataString($_) }) -join '/')
    return "ftp://$FtpHost/$encoded"
}

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
    try { $req = New-FtpRequest $url $creds ([System.Net.WebRequestMethods+Ftp]::MakeDirectory); $req.GetResponse().Close() } catch { }
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
    $w = 26
    $filled = if ($total -gt 0) { [int][math]::Floor($w * $sent / $total) } else { $w }
    if ($filled -lt 0) { $filled = 0 }
    if ($filled -gt $w) { $filled = $w }
    $bar = '[' + ('#' * $filled) + (' ' * ($w - $filled)) + ']'
    $pct = if ($total -gt 0) { [int](100 * $sent / $total) } else { 100 }
    $mb = if ($total -gt 0) { ('{0:N1}/{1:N1} MB' -f ($sent / 1MB), ($total / 1MB)) } else { '' }
    Write-Host -NoNewline ("`r  {0,4}/{1,-4} {2} {3,3}%  {4}  ({5} uploading)    " -f $done, $count, $bar, $pct, $mb, $active)
}

Step 'Uploading the website (FTP)'
Write-Host '  public site only - the admin part stays offline' -ForegroundColor DarkGray
$creds = New-Object System.Net.NetworkCredential($FtpUser, $FtpPass)
$localDir = (Resolve-Path 'website\dist').Path
$remoteRel = $FtpBasePath.Trim('/')
Ensure-FtpPath "ftp://$FtpHost" $remoteRel $creds
$ftpUrl = "ftp://$FtpHost/$remoteRel"
Write-Host "  $localDir  ->  $ftpUrl"
$files = @(Get-ChildItem -LiteralPath $localDir -Recurse -File)
$fileCount = $files.Count
$totalBytes = [long]0
if ($fileCount -gt 0) { $totalBytes = [long](($files | Measure-Object -Property Length -Sum).Sum) }

# allow one connection per worker (the .NET default of 2 would silently cap the parallelism)
[System.Net.ServicePointManager]::DefaultConnectionLimit = $FtpWorkers

$entries = @(foreach ($f in $files) {
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
Write-Host "  $msg" -ForegroundColor $(if ($errors.Count) { 'Red' } else { 'Green' })
foreach ($e in $errors) { Write-Host "    FAILED  $e" -ForegroundColor Red }
if ($errors.Count) { exit 1 }

# --- 5. Remove old downloads on the server --------------------------------------
# The upload above only adds/overwrites files; it never deletes. So installers of
# older versions that earlier builds put on the server would stay there forever.
# Delete any remote download whose version number differs from the current one, so
# the server only ever offers the latest build.
Step 'Removing old downloads on the server'
$remoteDownloads = $FtpBasePath.TrimEnd('/') + '/downloads'
$dlUrl = "ftp://$FtpHost/$($remoteDownloads.TrimStart('/'))"

function Get-FtpNames([string]$dirUrl, $creds) {
    $req = New-FtpRequest $dirUrl $creds ([System.Net.WebRequestMethods+Ftp]::ListDirectory)
    $req.UseBinary = $false   # a listing is text, not binary
    $resp = $req.GetResponse()
    $reader = New-Object System.IO.StreamReader($resp.GetResponseStream())
    try { $text = $reader.ReadToEnd() } finally { $reader.Close(); $resp.Close() }
    return @($text -split "`r?`n" | Where-Object { $_.Trim() -ne '' })
}

try {
    $names = Get-FtpNames $dlUrl $creds
    $removed = 0
    foreach ($n in $names) {
        $name = ($n -split '/')[-1].Trim()
        # reject LIST detail lines and anything that is not a plain file name
        if ($name -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]*$') { continue }
        $m = [regex]::Match($name, '(?<!\d)(\d+\.\d+\.\d+)(?!\d)')
        if ($m.Success -and $m.Groups[1].Value -ne $version) {
            $req = New-FtpRequest "$dlUrl/$name" $creds ([System.Net.WebRequestMethods+Ftp]::DeleteFile)
            $req.GetResponse().Close()
            Write-Host "  removed old download: $name" -ForegroundColor DarkYellow
            $removed++
        }
    }
    $c = if ($removed) { 'Yellow' } else { 'DarkGray' }
    Write-Host "  $removed old download(s) removed from the server" -ForegroundColor $c
} catch {
    Write-Host "  could not list/clean $remoteDownloads : $($_.Exception.Message)" -ForegroundColor Yellow
}
