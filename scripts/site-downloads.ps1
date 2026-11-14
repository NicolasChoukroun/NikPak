<#
  site-downloads.ps1 - the files behind the site's download buttons.

  The download page (website\build.py, DL_GROUPS) links to
      downloads/Nikpak-<version>-Windows.zip
      downloads/Nikpak-<version>-macOS.zip
      downloads/Nikpak-<version>-Linux.zip
  Those files must exist in website\downloads\ when buildweb.ps1 uploads the
  site, or every button is a 404 - which is what a buildwin run left behind on
  2026-11-14: the zip was packed into binaries\, the site was rebuilt (which wipes
  dist\), and nothing ever copied the zip into the folder the page links to.

  Dot-source it from a build script:

      . (Join-Path $PSScriptRoot 'scripts\site-downloads.ps1')

  Functions
      Get-SiteDownloadsPath              website\downloads
      Publish-SiteDownloadsToSite        copy this product's zips of this version
      Assert-SiteDownloadsComplete       fail when a button has no file
      Write-DownloadChecksums            downloads\SHA256SUMS.txt
      Set-DownloadSizes                  the sizes printed next to each button
#>

$ErrorActionPreference = 'Stop'

# Keys of site.json "downloads" and the platform each one is served by. The order
# is the order of the download page.
$script:NikpakDownloadPlatforms = [ordered]@{
  'windows' = 'Windows'
  'macos'   = 'macOS'
  'linux'   = 'Linux'
}

function Get-SiteDownloadsPath {
  param([Parameter(Mandatory)][string]$Root)
  return (Join-Path $Root 'website\downloads')
}

function Get-NikpakSiteVersion {
  param([Parameter(Mandatory)][string]$Root)
  $site = Join-Path $Root 'website\site.json'
  if (-not (Test-Path -LiteralPath $site)) { throw "website\site.json is missing." }
  $json = Get-Content -LiteralPath $site -Raw | ConvertFrom-Json
  return [string]$json.version
}

function Get-NikpakDownloadFile {
  # The file name a download button points at, {v} filled in.
  param([Parameter(Mandatory)][string]$Root, [Parameter(Mandatory)][string]$Key)
  $site = Join-Path $Root 'website\site.json'
  $json = Get-Content -LiteralPath $site -Raw | ConvertFrom-Json
  $rel = [string]$json.downloads.$Key
  if (-not $rel) { return $null }
  return ($rel.Replace('{v}', [string]$json.version))
}

function Publish-SiteDownloadsToSite {
<#
  Copy this version's zips from binaries\ into website\downloads\, leaving
  alone the other platforms' zips of the same version (buildmac / buildunix put
  theirs there). Any other file in the folder - an older version, a loose
  installer from the old layout - is removed: the site offers the zips and
  nothing else. Returns the names that are now in the folder.
#>
  param(
    [Parameter(Mandatory)][string]$Root,
    [Parameter(Mandatory)][string]$Version,
    [string]$Product = 'Nikpak'
  )

  $binaries = Join-Path $Root 'binaries'
  $downloads = Get-SiteDownloadsPath -Root $Root
  New-Item -ItemType Directory -Force -Path $downloads | Out-Null
  if (-not (Test-Path -LiteralPath $binaries)) {
    Write-Host "   binaries\ does not exist: nothing to publish" -ForegroundColor Yellow
    return @()
  }

  $keep = @()
  foreach ($f in @(Get-ChildItem -LiteralPath $binaries -File -Filter "$Product-$Version-*.zip" -ErrorAction SilentlyContinue)) {
    $keep += $f.Name
  }
  # checksums travel with the downloads, like the other Nikkoid sites
  $keep += 'SHA256SUMS.txt'

  foreach ($item in @(Get-ChildItem -LiteralPath $downloads -Force -ErrorAction SilentlyContinue)) {
    if ($keep -contains $item.Name) { continue }
    try {
      Remove-Item -LiteralPath $item.FullName -Recurse -Force -ErrorAction Stop
      Write-Host "   removed from the site's downloads: $($item.Name)" -ForegroundColor DarkYellow
    } catch {
      Write-Host "   could not remove $($item.Name) (still open?)" -ForegroundColor Yellow
    }
  }

  $copied = @()
  foreach ($name in $keep) {
    if ($name -eq 'SHA256SUMS.txt') { continue }
    $src = Join-Path $binaries $name
    if (-not (Test-Path -LiteralPath $src)) { continue }
    $dst = Join-Path $downloads $name
    $same = $false
    if (Test-Path -LiteralPath $dst) {
      $a = Get-Item -LiteralPath $src -Force
      $b = Get-Item -LiteralPath $dst -Force
      $same = ($a.Length -eq $b.Length) -and
              ([Math]::Abs(($a.LastWriteTimeUtc - $b.LastWriteTimeUtc).TotalSeconds) -le 2)
    }
    if (-not $same) { Copy-Item -LiteralPath $src -Destination $dst -Force }
    $copied += $name
  }
  if ($copied.Count) {
    Write-DownloadChecksums -Root $Root | Out-Null
  } else {
    # no zip of this version: a checksum file left from an older release would
    # describe files that are no longer offered
    $sums = Join-Path $downloads 'SHA256SUMS.txt'
    if (Test-Path -LiteralPath $sums) { Remove-Item -LiteralPath $sums -Force -ErrorAction SilentlyContinue }
  }
  return $copied
}

function Write-DownloadChecksums {
  # SHA256SUMS.txt over every zip in the downloads folder, the file the page
  # tells visitors to check.
  param([Parameter(Mandatory)][string]$Root)

  $downloads = Get-SiteDownloadsPath -Root $Root
  if (-not (Test-Path -LiteralPath $downloads)) { return }
  $lines = @()
  foreach ($f in @(Get-ChildItem -LiteralPath $downloads -File -Filter '*.zip' -ErrorAction SilentlyContinue | Sort-Object Name)) {
    $h = (Get-FileHash -Algorithm SHA256 -LiteralPath $f.FullName).Hash.ToLower()
    $lines += "$h  $($f.Name)"
  }
  if (-not $lines.Count) { return }
  Set-Content -LiteralPath (Join-Path $downloads 'SHA256SUMS.txt') -Value $lines -Encoding ascii
  Write-Host ("   SHA256SUMS.txt: {0} file(s)" -f $lines.Count)
}

function Set-DownloadSizes {
<#
  Write the display size of every download into website\site.json
  ("download_sizes"), which the page prints under each button. The size comes
  from the file that is about to be published, so it can never describe a build
  that is not there.

  **-From is binaries\, not the site's downloads folder.** The sizes must be in
  site.json BEFORE website\build.py runs, because the build wipes dist\ (and with
  it the copies of the zips the sizes are read from). Reading them from binaries\
  is what lets the order be: sizes -> build the page -> copy the files in.

  Only that one object is rewritten, keeping the rest of the file (its comments
  and its hand-written formatting) exactly as it was: ConvertTo-Json would
  reformat the whole file and escape every < and > in it.
#>
  param(
    [Parameter(Mandatory)][string]$Root,
    [string]$From = '',
    [switch]$WhatIf
  )

  if (-not $From) { $From = Join-Path $Root 'binaries' }
  $downloads = $From
  $site = Join-Path $Root 'website\site.json'
  if (-not (Test-Path -LiteralPath $site)) { return 0 }

  $raw = [System.IO.File]::ReadAllText($site)
  $mDl = [regex]::Match($raw, '(?s)("downloads"\s*:\s*\{)(.*?)(\r?\n\s*\})')
  $m = [regex]::Match($raw, '(?s)("download_sizes"\s*:\s*\{)(.*?)(\r?\n\s*\})')
  if (-not $m.Success) { throw 'website\site.json has no "download_sizes" object.' }
  if (-not $mDl.Success) { throw 'website\site.json has no "downloads" object.' }
  $body = $m.Groups[2].Value

  $changed = 0
  foreach ($key in @($script:NikpakDownloadPlatforms.Keys)) {
    $mRel = [regex]::Match($mDl.Groups[2].Value, '(?m)^\s*"' + [regex]::Escape($key) + '"\s*:\s*"([^"]*)"')
    if (-not $mRel.Success) { continue }
    $name = Split-Path -Path ($mRel.Groups[1].Value.Replace('{v}', (Get-NikpakSiteVersion -Root $Root))) -Leaf
    $file = Join-Path $downloads $name
    $size = ''
    if (Test-Path -LiteralPath $file) {
      $mb = (Get-Item -LiteralPath $file).Length / 1MB
      $size = if ($mb -ge 1) { '{0:N1} MB' -f $mb } else { '{0:N0} KB' -f ($mb * 1024) }
    }
    $mSlot = [regex]::Match($body, '(?m)^(\s*"' + [regex]::Escape($key) + '"\s*:\s*")[^"]*(")')
    if (-not $mSlot.Success) { continue }
    if ($mSlot.Groups[0].Value -ne ($mSlot.Groups[1].Value + $size + $mSlot.Groups[2].Value)) {
      $body = $body.Remove($mSlot.Index, $mSlot.Length).Insert($mSlot.Index, ($mSlot.Groups[1].Value + $size + $mSlot.Groups[2].Value))
      $changed++
      Write-Host ("   size {0,-8} {1}" -f $key, $(if ($size) { $size } else { '(file missing - button shows no size)' })) `
        -ForegroundColor $(if ($size) { 'DarkGray' } else { 'Yellow' })
    }
  }
  if ($changed -and -not $WhatIf) {
    $new = $raw.Substring(0, $m.Groups[2].Index) + $body + $raw.Substring($m.Groups[2].Index + $m.Groups[2].Length)
    # UTF-8 without a BOM: Python's json.load rejects one
    [System.IO.File]::WriteAllText($site, $new, [System.Text.UTF8Encoding]::new($false))
  }
  return $changed
}

function Assert-SiteDownloadsComplete {
<#
  Every download button of the site must have its file, or the page offers a 404
  - the failure this whole file exists to prevent. Names the missing ones and
  says which script produces each.
#>
  param([Parameter(Mandatory)][string]$Root, [Parameter(Mandatory)][string]$Version)

  $downloads = Get-SiteDownloadsPath -Root $Root
  $missing = @()
  foreach ($key in @($script:NikpakDownloadPlatforms.Keys)) {
    $rel = Get-NikpakDownloadFile -Root $Root -Key $key
    if (-not $rel) { continue }
    $name = Split-Path -Path $rel -Leaf
    if (-not (Test-Path -LiteralPath (Join-Path $downloads $name))) {
      $missing += ("{0}  ({1} download)" -f $name, $script:NikpakDownloadPlatforms[$key])
    }
  }
  if ($missing.Count) {
    $scripts = @{
      'windows' = 'buildwin.ps1'
      'macos'   = 'buildmac.ps1'
      'linux'   = 'buildunix.ps1'
    }
    $hint = @()
    foreach ($key in @($script:NikpakDownloadPlatforms.Keys)) {
      $rel = Get-NikpakDownloadFile -Root $Root -Key $key
      if (-not $rel) { continue }
      if (-not (Test-Path -LiteralPath (Join-Path $downloads (Split-Path -Path $rel -Leaf)))) {
        $hint += ("  {0,-8} -> run .\{1} for {2}" -f $script:NikpakDownloadPlatforms[$key], $scripts[$key], $Version)
      }
    }
    throw ("The site would publish download buttons with no file behind them:`n    " +
           ($missing -join "`n    ") + "`n" +
           "  Build them first:`n" + ($hint -join "`n") + "`n" +
           "  Nothing was uploaded, so the site on the server is unchanged.")
  }
  Write-Host ("   all {0} download(s) of {1} are in place" -f $script:NikpakDownloadPlatforms.Count, $Version) -ForegroundColor Green
}

function Get-SiteFilesToPublish {
<#
  The files of website\ that go to the server. **The site is the folder** - there is
  no dist\ (NikEdit works the same way): the pages sit next to their sources, so the
  upload is this list and not "everything under website\". i18n\, static\, tech\,
  site.json and build.py are sources and must never be published (site.json holds the
  payment settings; site-public.json is the public copy).
#>
  param([Parameter(Mandatory)][string]$Root)

  $site = Join-Path $Root 'website'
  $files = @()
  foreach ($pat in @('*.html', '*.php')) {
    $files += @(Get-ChildItem -LiteralPath $site -File -Filter $pat -ErrorAction SilentlyContinue)
  }
  foreach ($name in @('robots.txt', 'sitemap.xml', 'version.json', 'site-public.json', 'LICENSE.txt', 'web.config')) {
    $f = Join-Path $site $name
    if (Test-Path -LiteralPath $f) { $files += (Get-Item -LiteralPath $f) }
  }
  foreach ($dir in @('assets', 'downloads', 'inc', 'shop')) {
    $d = Join-Path $site $dir
    if (Test-Path -LiteralPath $d) { $files += @(Get-ChildItem -LiteralPath $d -Recurse -File -ErrorAction SilentlyContinue) }
  }
  return @($files | Where-Object { $_.Name -ne 'build.py' })
}
