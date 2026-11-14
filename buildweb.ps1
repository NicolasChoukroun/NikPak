<#
.SYNOPSIS
  NikPak - rebuild the web site and upload it (with the downloads) to the server.

.DESCRIPTION
  No build, no Inno - just the web site. It:
    1. collects the packages: every platform's zip of this version from binaries\,
       packing one from its loose files when the zip is missing;
    2. writes the size of each download into website\site.json, which is where the
       page reads the figure it prints under the button;
    3. runs  python website\build.py  which rebuilds website (public pages only);
    4. copies the zips into website\downloads\ and regenerates SHA256SUMS.txt -
       **after** the build, which wipes dist\;
    5. refuses to publish while a download button has no file behind it - the state
       that made the site offer 404s after a Windows-only build;
    6. uploads website to the web host by FTP (overwriting), 10 files at a time,
       with one inline progress line;
    7. removes downloads of older versions from the server, so only the latest build
       is ever offered;
    8. commits and pushes the source (steps 2, 4 and 6 edit it), so the next
       buildmac.ps1 / buildunix.ps1 does not refuse to start.

  Run it after buildwin.ps1 + buildmac.ps1 + buildunix.ps1 when every platform is
  built: it publishes the current version on the web site without rebuilding anything.

  Before publishing it checks that Cargo.toml, Cargo.lock, website\site.json and
  website\version.json all carry the same version. A mismatch stops the run, so the
  site can never advertise a download that does not exist (or hide a newer release).

.PARAMETER SkipCommit
  Do not commit and push the source at the end (run .\commit.ps1 yourself).

.PARAMETER AllowMissingDownloads
  Publish even though a platform's zip is missing (an emergency re-upload: those
  buttons will lead to a 404 until the platform is built).

.EXAMPLE
  .\buildweb.ps1
  .\buildweb.ps1 -SkipCommit
  powershell -ExecutionPolicy Bypass -File buildweb.ps1      (if scripts are blocked)
#>
param(
    [switch]$SkipCommit,
    [switch]$AllowMissingDownloads
)

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot
. (Join-Path $PSScriptRoot 'scripts\version.ps1')
. (Join-Path $PSScriptRoot 'scripts\site-downloads.ps1')
. (Join-Path $PSScriptRoot 'scripts\release-commit.ps1')

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
  # The eleven manuals, before the pack: release-packaging copies every *.pdf
  # from documents\manuals into the zip's Documentation folder.
  & (Join-Path $PSScriptRoot 'scripts\build-docs.ps1')

# --- 2. The packages --------------------------------------------------------------
# The zips come from binaries\, where buildwin / buildmac / buildunix left them. A
# platform whose zip is missing is packed from its loose files when those are still
# there (a leg that ran before the zips scheme), and otherwise reported below.
Step 'Collecting the packages'
Remove-OldNikpakFiles -Root $PSScriptRoot -Version $version -Dirs @('binaries', 'dist', 'website\downloads')
. (Join-Path $PSScriptRoot 'scripts\release-packaging.ps1')
if (Test-Path 'binaries') {
    $binaries = Join-Path $PSScriptRoot 'binaries'
    $packages = @(Get-PlatformPackages -Product 'Nikpak' -Version $version -Dir $binaries)
    $inc = @{
        'Windows' = @(@{ Pattern = 'nikpak-{v}-setup.exe' }, @{ Pattern = 'unnik-{v}-setup.exe' },
                      @{ Pattern = 'nikpak.exe' }, @{ Pattern = 'unNik.exe' })
        'macOS'   = @(@{ Pattern = 'nikpak-{v}-macos.zip' })
        'Linux'   = @(@{ Pattern = 'nikpak-{v}-linux-*.tar.gz' })
    }
    foreach ($p in @('Windows', 'macOS', 'Linux')) {
        if ($packages.Name -contains (Get-SitePackageName 'Nikpak' $version $p)) { continue }
        $z = New-SitePackage -Product 'Nikpak' -Version $version -Platform $p -Binaries $binaries `
                             -Include $inc[$p] -Documentation (Join-Path $PSScriptRoot 'documents\manuals')
        if ($z) { $packages += (Get-Item -LiteralPath $z) }
    }
} else {
    Write-Host '  binaries\ is empty: there is nothing to publish' -ForegroundColor Yellow
}

# --- 2b. The sizes the page prints ------------------------------------------------
# Read from binaries\ (the file that will be copied in a moment), and written BEFORE
# the site is built: the page reads them from site.json, and the build wipes dist\ -
# so a size computed from the site's own copies would be computed from files the
# build is about to delete.
Step 'Download sizes'
Set-DownloadSizes -Root $PSScriptRoot | Out-Null

# --- 3. The website (python website\build.py) ------------------------------------
Step 'Building the website (website\build.py)'
& $pythonExe @pythonArgs (Join-Path $PSScriptRoot 'website\build.py')
if ($LASTEXITCODE -ne 0) { Fail 'The website build failed (website\build.py).' }

# --- 3b. Website downloads -------------------------------------------------------
# Only the current version is ever published: anything else in the folder - an older
# build, a loose installer from the old layout - is removed, so stale files are never
# uploaded. This runs AFTER build.py, which wipes dist\.
Step 'Adding the downloads to the website'
$downloads = Get-SiteDownloadsPath -Root $PSScriptRoot
$published = @(Publish-SiteDownloadsToSite -Root $PSScriptRoot -Version $version)
Write-Host ("  {0} download(s) in {1} (NikPak {2})" -f $published.Count, $downloads, $version)
foreach ($p in $published) { Write-Host "     $p" }

# Every download button of site.json must have its file, or the site offers a 404.
# This is a hard stop by default - publishing a page whose buttons lead nowhere is the
# exact failure this script is meant to prevent.
$complete = $true
try { Assert-SiteDownloadsComplete -Root $PSScriptRoot -Version $version }
catch {
    $complete = $false
    Write-Host ''
    Write-Host $_.Exception.Message -ForegroundColor Red
    if (-not $AllowMissingDownloads) {
        Fail ("Not publishing: the site on the server is unchanged.`n" +
              "Build the missing platform(s), then run .\buildweb.ps1 again.`n" +
              "(-AllowMissingDownloads publishes anyway - those buttons will be 404s.)")
    }
    Write-Host '  Publishing anyway (-AllowMissingDownloads).' -ForegroundColor Yellow
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
$localDir = (Resolve-Path 'website').Path
$remoteRel = $FtpBasePath.Trim('/')
Ensure-FtpPath "ftp://$FtpHost" $remoteRel $creds
$ftpUrl = "ftp://$FtpHost/$remoteRel"
Write-Host "  $localDir  ->  $ftpUrl"
# the published site, not its sources (i18n\, static\, tech\, build.py, site.json)
$files = @(Get-SiteFilesToPublish -Root $PSScriptRoot)
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

# --- 5b. Remove the old per-language folders on the server ------------------------
# The site used to be published as en/index.html, fr/index.html ...; it is one set of
# pages now, with the language chosen in the browser. The old folders would keep
# serving the previous release for ever, so they are deleted (best effort: a host may
# refuse, which is only reported).
Step 'Removing the old per-language folders on the server'
function Remove-FtpDir([string]$url, $creds) {
    try {
        $names = Get-FtpNames $url $creds
    } catch { return }
    foreach ($n in $names) {
        $name = ($n -split '/')[-1].Trim()
        if ($name -notmatch '^[A-Za-z0-9][A-Za-z0-9._-]*$') { continue }
        $child = "$url/$name"
        if ($n -match '^d' -or $n -match '/$') { Remove-FtpDir $child $creds; continue }
        # LIST detail lines start with a mode; a bare name is a file
        if ($n -match '^[d-][rwx-]{9}') {
            if ($n.StartsWith('d')) { Remove-FtpDir $child $creds }
            else {
                try { $r = New-FtpRequest $child $creds ([System.Net.WebRequestMethods+Ftp]::DeleteFile); $r.GetResponse().Close() } catch { }
            }
            continue
        }
        try { $r = New-FtpRequest $child $creds ([System.Net.WebRequestMethods+Ftp]::DeleteFile); $r.GetResponse().Close() } catch { }
    }
    try { $r = New-FtpRequest $url $creds ([System.Net.WebRequestMethods+Ftp]::RemoveDirectory); $r.GetResponse().Close() } catch { }
}
$removedDirs = 0
foreach ($c in @('en','zh','hi','es','ar','fr','bn','pt','ru','id','ja')) {
    $u = "ftp://$FtpHost/" + (($FtpBasePath.Trim('/') + '/' + $c).TrimStart('/'))
    try { $null = Get-FtpNames $u $creds; Remove-FtpDir $u $creds; $removedDirs++; Write-Host "  removed /$c/" -ForegroundColor DarkYellow } catch { }
}
Write-Host ("  {0} old language folder(s) removed (if the host allowed it)" -f $removedDirs)

# --- 6. Commit and push the source ----------------------------------------------
# Steps 2 and 3 rewrote website, website\site.json (the sizes) and the downloads
# checksums. A publish that leaves the tree dirty makes the next buildmac.ps1 /
# buildunix.ps1 refuse to start - the failure nicol hit on 2026-11-14.
Step 'Committing the release source'
$GitStatus = Complete-ReleaseSource -Root $PSScriptRoot -SkipCommit:$SkipCommit

Step 'Done'
Write-Host ("  NikPak {0} is published (site {1})" -f $version, $(if ($complete) { 'complete' } else { 'with missing downloads' })) -ForegroundColor Green
Write-Host ("  git: " + $GitStatus)
if ($GitStatus -like 'FAILED*' -or $GitStatus -like 'skipped*') {
    Write-Host '       run .\commit.ps1 before the next buildmac.ps1 / buildunix.ps1' -ForegroundColor Yellow
}
