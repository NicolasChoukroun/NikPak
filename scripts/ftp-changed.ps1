<#
.SYNOPSIS
  Upload only what changed: pick, from a list of files about to be sent by FTP, the ones
  the server does not already have, and remember what was sent.

.DESCRIPTION
  Dot-sourced by the project's FTP upload scripts (buildweb.ps1, buildall.ps1, ...):

      . (Join-Path $PSScriptRoot 'scripts\ftp-changed.ps1')
      $plan    = Select-FtpChanged -Entries $entries -LocalProperty FullName -UrlProperty Url `
                                   -Credential $creds -StatePath $FtpStatePath
      $entries = @($plan.Upload)          # upload only these
      ...upload...
      Save-FtpState -Plan $plan -Uploaded $entriesThatWereUploaded

  How a file is judged, using its size and its date:
    1. The folders on the server are listed once each (LIST, 10 at a time), which gives the
       size and the date of every file already there.
    2. A file missing on the server, or with another size there, is uploaded.
    3. Same size: the file is compared with what was uploaded last time (kept in
       .ftp-state.json beside the build scripts, git-ignored: size, date and SHA-256 of each
       file sent). Same date as then -> skipped. New date but the same content (a page
       rebuilt identically) -> skipped. Otherwise -> uploaded.
    4. Same size but never sent from this computer (first run, or a new clone): the server's
       date decides. If the server copy was written clearly after the local file was last
       changed (by more than 14 hours, which covers any server time zone), it is skipped;
       otherwise it is uploaded.
  A listing that fails (a server that refuses LIST, a folder that does not exist yet) only
  means those files are uploaded, so nothing is ever left out.

  Set the environment variable FTP_UPLOAD_ALL=1 to upload everything regardless:
      $env:FTP_UPLOAD_ALL = '1'; .\buildweb.ps1

  Works in Windows PowerShell 5.1 (ASCII only, no PowerShell 7 features).
#>

function Get-FtpListingUrl([string]$url) {
  # the folder of a file URL, with a trailing slash (what LIST needs)
  return $url.Substring(0, $url.LastIndexOf('/') + 1)
}

function ConvertTo-FtpFileUrl([string]$server, [string]$remotePath) {
  $p = $remotePath.Replace('\', '/').Trim('/')
  $segs = $p.Split('/') | ForEach-Object { [Uri]::EscapeDataString([Uri]::UnescapeDataString($_)) }
  return 'ftp://' + $server.Trim('/') + '/' + ($segs -join '/')
}

# One LIST line -> @{ Name; Size; Time (DateTime or $null); Dir (bool) }, or $null.
# Understands the Unix style (ProFTPD, vsftpd, Pure-FTPd, IIS in Unix mode) and the MS-DOS
# style (IIS default).
$script:FtpMonths = @{ jan = 1; feb = 2; mar = 3; apr = 4; may = 5; jun = 6; jul = 7; aug = 8; sep = 9; oct = 10; nov = 11; dec = 12 }
function ConvertFrom-FtpListLine([string]$line) {
  $line = $line.TrimEnd()
  if (-not $line) { return $null }
  $m = [regex]::Match($line, '^(?<perm>[\-dlbcps][rwxsStTl\-]{9}\S*)\s+.*?\s(?<size>\d+)\s+(?<mon>[A-Za-z]{3})\s+(?<day>\d{1,2})\s+(?<ty>\d{1,2}:\d{2}|\d{4})\s(?<name>.+)$')
  if ($m.Success) {
    $isDir = $m.Groups['perm'].Value.StartsWith('d')
    $name = $m.Groups['name'].Value
    if ($m.Groups['perm'].Value.StartsWith('l') -and $name.Contains(' -> ')) { $name = $name.Substring(0, $name.IndexOf(' -> ')) }
    $time = $null
    $mon = $script:FtpMonths[$m.Groups['mon'].Value.ToLowerInvariant()]
    if ($mon) {
      $day = [int]$m.Groups['day'].Value
      $ty = $m.Groups['ty'].Value
      try {
        if ($ty.Contains(':')) {
          # "Nov  4 09:25": this year, or last year when that would be in the future
          $hm = $ty.Split(':')
          $now = [DateTime]::UtcNow
          $time = New-Object DateTime ($now.Year, $mon, $day, [int]$hm[0], [int]$hm[1], 0, [DateTimeKind]::Utc)
          if ($time -gt $now.AddDays(2)) { $time = $time.AddYears(-1) }
        } else {
          # "Nov  4  2025": a day only; take its start, so a file changed later that day is newer
          $time = New-Object DateTime ([int]$ty, $mon, $day, 0, 0, 0, [DateTimeKind]::Utc)
        }
      } catch { $time = $null }
    }
    return [pscustomobject]@{ Name = $name; Size = [long]$m.Groups['size'].Value; Time = $time; Dir = $isDir }
  }
  $m = [regex]::Match($line, '^(?<d>\d{2}-\d{2}-\d{2,4})\s+(?<t>\d{1,2}:\d{2}\s?(?:[AaPp][Mm])?)\s+(?<sz><DIR>|\d+)\s+(?<name>.+)$')
  if ($m.Success) {
    $isDir = $m.Groups['sz'].Value -eq '<DIR>'
    $time = $null
    $text = ($m.Groups['d'].Value + ' ' + ($m.Groups['t'].Value -replace '\s', '')).ToUpperInvariant()
    $formats = [string[]]@('MM-dd-yy hh:mmtt', 'MM-dd-yyyy hh:mmtt', 'MM-dd-yy HH:mm', 'MM-dd-yyyy HH:mm', 'MM-dd-yy h:mmtt', 'MM-dd-yyyy h:mmtt', 'MM-dd-yy H:mm', 'MM-dd-yyyy H:mm')
    $parsed = [DateTime]::MinValue
    if ([DateTime]::TryParseExact($text, $formats, [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::AssumeUniversal -bor [Globalization.DateTimeStyles]::AdjustToUniversal, [ref]$parsed)) {
      $time = $parsed
    }
    $size = if ($isDir) { [long]0 } else { [long]$m.Groups['sz'].Value }
    return [pscustomobject]@{ Name = $m.Groups['name'].Value; Size = $size; Time = $time; Dir = $isDir }
  }
  return $null
}

# List several server folders at once (10 connections). Returns @{ folderUrl = @{ name = entry } };
# a folder that cannot be listed maps to $null.
function Get-FtpFolderListings([string[]]$folderUrls, [System.Net.NetworkCredential]$credential, [int]$workers = 10) {
  $result = @{}
  if (-not $folderUrls -or $folderUrls.Count -eq 0) { return $result }
  if ([System.Net.ServicePointManager]::DefaultConnectionLimit -lt $workers) {
    [System.Net.ServicePointManager]::DefaultConnectionLimit = $workers
  }
  $lister = {
    param($url, $cred)
    try {
      $req = [System.Net.FtpWebRequest]::Create($url)
      $req.Method = [System.Net.WebRequestMethods+Ftp]::ListDirectoryDetails
      $req.Credentials = $cred
      $req.UsePassive = $true
      $req.UseBinary = $false
      $req.KeepAlive = $true
      $req.Timeout = 30000
      $req.ReadWriteTimeout = 60000
      $resp = $req.GetResponse()
      try {
        $reader = New-Object System.IO.StreamReader($resp.GetResponseStream())
        $text = $reader.ReadToEnd()
        $reader.Close()
      } finally { $resp.Close() }
      [pscustomobject]@{ url = $url; ok = $true; text = $text }
    } catch {
      [pscustomobject]@{ url = $url; ok = $false; text = '' }
    }
  }
  $pool = [runspacefactory]::CreateRunspacePool(1, $workers)
  $pool.Open()
  try {
    $jobs = @(foreach ($u in $folderUrls) {
      $ps = [powershell]::Create()
      $ps.RunspacePool = $pool
      [void]$ps.AddScript($lister).AddArgument($u).AddArgument($credential)
      [pscustomobject]@{ ps = $ps; handle = $ps.BeginInvoke(); url = $u }
    })
    foreach ($j in $jobs) {
      $entries = $null
      try {
        foreach ($r in $j.ps.EndInvoke($j.handle)) {
          if ($r.ok) {
            $entries = @{}
            foreach ($line in ($r.text -split "`r?`n")) {
              $e = ConvertFrom-FtpListLine $line
              if ($e -and -not $e.Dir) { $entries[$e.Name] = $e }
            }
          }
        }
      } catch { $entries = $null }
      $result[$j.url] = $entries
      $j.ps.Dispose()
    }
  } finally {
    $pool.Close(); $pool.Dispose()
  }
  return $result
}

function Read-FtpState([string]$path) {
  $state = @{}
  if ($path -and (Test-Path -LiteralPath $path)) {
    try {
      $obj = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
      foreach ($p in $obj.PSObject.Properties) { $state[$p.Name] = $p.Value }
    } catch {
      Write-Host ("   ({0} could not be read: every file is checked against the server)" -f (Split-Path $path -Leaf)) -ForegroundColor Yellow
    }
  }
  return $state
}

function Get-FtpFileHash([string]$path) {
  return (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash
}

function New-FtpStateEntry([string]$localPath) {
  $fi = New-Object System.IO.FileInfo $localPath
  return [pscustomobject]@{
    size  = [string]$fi.Length
    mtime = [string]$fi.LastWriteTimeUtc.Ticks
    hash  = (Get-FtpFileHash $localPath)
  }
}

<#
  Select-FtpChanged: which of -Entries need uploading.
  -LocalProperty   the property holding the local file path (FullName, Local, ...)
  -UrlProperty     the property holding the full ftp:// URL of the remote file
  -RemotePathProperty with -Server, instead of -UrlProperty: the remote path (/http/site/a.html)
  Returns an object: Upload (the entries to send), Skipped (count), plus what Save-FtpState needs.
#>
function Select-FtpChanged {
  param(
    [object[]]$Entries,
    [string]$LocalProperty = 'FullName',
    [string]$UrlProperty = 'Url',
    [string]$RemotePathProperty = '',
    [string]$Server = '',
    [System.Net.NetworkCredential]$Credential,
    [string]$StatePath,
    [int]$Workers = 10
  )
  $Entries = @($Entries | Where-Object { $_ })
  $urlOf = @{}
  foreach ($e in $Entries) {
    $local = [string]$e.$LocalProperty
    $url = if ($RemotePathProperty) { ConvertTo-FtpFileUrl $Server ([string]$e.$RemotePathProperty) } else { [string]$e.$UrlProperty }
    $urlOf[$local] = $url
  }
  $plan = [pscustomobject]@{
    Upload = @(); Skipped = 0; State = (Read-FtpState $StatePath); StatePath = $StatePath
    LocalProperty = $LocalProperty; UrlOf = $urlOf; Dirty = $false
  }
  if ($env:FTP_UPLOAD_ALL -eq '1') {
    Write-Host '   FTP_UPLOAD_ALL=1: every file is uploaded' -ForegroundColor Yellow
    $plan.Upload = $Entries
    return $plan
  }
  if ($Entries.Count -eq 0) { return $plan }

  $folders = @($urlOf.Values | ForEach-Object { Get-FtpListingUrl $_ } | Sort-Object -Unique)
  $listings = Get-FtpFolderListings $folders $Credential $Workers

  $upload = New-Object System.Collections.ArrayList
  $new = 0; $changed = 0
  foreach ($e in $Entries) {
    $local = [string]$e.$LocalProperty
    $url = $urlOf[$local]
    $listing = $listings[(Get-FtpListingUrl $url)]
    $name = [Uri]::UnescapeDataString($url.Substring($url.LastIndexOf('/') + 1))
    $remote = $null
    if ($listing) { $remote = $listing[$name] }
    $fi = New-Object System.IO.FileInfo $local
    if (-not $remote) { [void]$upload.Add($e); $new++; continue }
    if ($remote.Size -ne $fi.Length) { [void]$upload.Add($e); $changed++; continue }

    $mtime = [string]$fi.LastWriteTimeUtc.Ticks
    $last = $plan.State[$url]
    if ($last -and [string]$last.size -eq [string]$fi.Length) {
      if ([string]$last.mtime -eq $mtime) { $plan.Skipped++; continue }
      # rebuilt with a new date: the same bytes as last time need no upload
      if ([string]$last.hash -eq (Get-FtpFileHash $local)) {
        $last.mtime = $mtime
        $plan.Dirty = $true
        $plan.Skipped++
        continue
      }
      [void]$upload.Add($e); $changed++; continue
    }
    # never sent from this computer: trust the server copy only when it is clearly newer
    if ($remote.Time -and $remote.Time.AddHours(-14) -gt $fi.LastWriteTimeUtc) {
      $plan.State[$url] = New-FtpStateEntry $local
      $plan.Dirty = $true
      $plan.Skipped++
      continue
    }
    [void]$upload.Add($e); $changed++
  }
  $plan.Upload = @($upload)
  $unlisted = @($listings.Keys | Where-Object { $null -eq $listings[$_] }).Count
  $note = if ($unlisted) { ("; {0} folder(s) not listed on the server, their files are sent" -f $unlisted) } else { '' }
  Write-Host ("   {0} file(s) to upload ({1} new, {2} changed), {3} already on the server{4}" -f $plan.Upload.Count, $new, $changed, $plan.Skipped, $note) -ForegroundColor DarkGray
  if ($plan.Dirty -and -not $plan.Upload.Count) { Save-FtpState -Plan $plan -Uploaded @() }
  return $plan
}

# Remember what was uploaded (size, date, SHA-256), for the next run.
function Save-FtpState {
  param($Plan, [object[]]$Uploaded = @())
  if (-not $Plan -or -not $Plan.StatePath) { return }
  # merge with the file as it is now, so two uploads of one run (or two scripts) both count
  $state = Read-FtpState $Plan.StatePath
  foreach ($k in @($Plan.State.Keys)) { $state[$k] = $Plan.State[$k] }
  foreach ($e in @($Uploaded | Where-Object { $_ })) {
    $local = [string]$e.($Plan.LocalProperty)
    $url = $Plan.UrlOf[$local]
    if (-not $url -or -not (Test-Path -LiteralPath $local)) { continue }
    $state[$url] = New-FtpStateEntry $local
  }
  $sorted = [ordered]@{}
  foreach ($k in ($state.Keys | Sort-Object)) { $sorted[$k] = $state[$k] }
  try {
    $json = [pscustomobject]$sorted | ConvertTo-Json -Depth 4
    [System.IO.File]::WriteAllText($Plan.StatePath, $json, (New-Object System.Text.UTF8Encoding $false))
    $Plan.State = $state
    $Plan.Dirty = $false
  } catch {
    Write-Host ("   could not write {0}: {1}" -f $Plan.StatePath, $_.Exception.Message) -ForegroundColor Yellow
  }
}
