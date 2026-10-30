<#
.SYNOPSIS
  NikPak - rebuild the web site and upload it (with the downloads) to the server.

.DESCRIPTION
  No build, no Inno - just the web site. It:
    1. runs  python website\build.py  which rebuilds website\dist (public pages only);
    2. copies whatever is in binaries\ into website\dist\downloads\;
    3. uploads website\dist to the web host by FTP (overwriting), 10 files at a time,
       with one inline progress line.

  Run it after buildall.ps1 / buildwin.ps1 when you only need to refresh the web
  site (or the download list) without rebuilding anything.

.EXAMPLE
  .\buildweb.ps1
  powershell -ExecutionPolicy Bypass -File buildweb.ps1      (if scripts are blocked)
#>

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot

function Step([string]$msg) { Write-Host ''; Write-Host "== $msg" -ForegroundColor Cyan }
function Fail([string]$msg) { Write-Host ''; Write-Host $msg -ForegroundColor Red; exit 1 }

# --- 1. Python 3 -----------------------------------------------------------------
Step 'Checking Python 3'
$pythonExe = $null
$pythonArgs = @()
foreach ($cand in @(@('python'), @('py', '-3'), @('python3'))) {
    $cmd = Get-Command $cand[0] -ErrorAction SilentlyContinue
    if (-not $cmd) { continue }
    $extra = @()
    if ($cand.Count -gt 1) { $extra = $cand[1..($cand.Count - 1)] }
    $ver = $null
    try { $ver = (& $cmd.Source @extra --version 2>&1 | Out-String).Trim() } catch { continue }
    if ($LASTEXITCODE -eq 0 -and $ver -match '^Python 3\.') {
        $pythonExe = $cmd.Source
        $pythonArgs = $extra
        Write-Host "Python:     $ver  ($pythonExe)"
        break
    }
}
if (-not $pythonExe) { Fail ("Python 3 was not found (it rebuilds the website). Install it once:`n    winget install Python.Python.3.12") }

# --- 2. The website (python website\build.py) ------------------------------------
Step 'Building the website (website\build.py)'
& $pythonExe @pythonArgs (Join-Path $PSScriptRoot 'website\build.py')
if ($LASTEXITCODE -ne 0) { Fail 'The website build failed (website\build.py).' }

# --- 3. Website downloads --------------------------------------------------------
Step 'Adding the downloads to the website'
$downloads = Join-Path $PSScriptRoot 'website\dist\downloads'
New-Item -ItemType Directory -Force -Path $downloads | Out-Null
if (Test-Path 'binaries') {
    Get-ChildItem 'binaries' -File | Copy-Item -Destination $downloads -Force
    Write-Host "  copied $(@(Get-ChildItem 'binaries' -File).Count) file(s) to $downloads"
} else {
    Write-Host '  binaries\ is empty: the download page will have no files' -ForegroundColor Yellow
}

# --- 4. Upload the website (FTP) -------------------------------------------------
$FtpHost = '65.21.192.14'
$FtpUser = 'NikkoD'
$FtpPass = 'Akaklike666'
$FtpBasePath = '/http/NikPak.nikkoid.com'

# upload this many files at once, each on its own FTP connection
$FtpWorkers = 10

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
            if ($r.ok) { $total++ } else { $errors += "$($r.rel) : $($r.err)" }
        }
    } catch {
        $errors += "$($j.rel) : $($_.Exception.Message)"
    }
    $j.ps.Dispose()
}
$pool.Close(); $pool.Dispose()

$msg = "{0} files uploaded" -f $total
if ($errors.Count) { $msg += ", $($errors.Count) FAILED" }
Write-Host "  $msg" -ForegroundColor $(if ($errors.Count) { 'Red' } else { 'Green' })
foreach ($e in $errors) { Write-Host "    FAILED  $e" -ForegroundColor Red }
if ($errors.Count) { exit 1 }
