<#
.SYNOPSIS
  Nikkoid - one implementation of "a download is a zip per platform".

.DESCRIPTION
  Every Nikkoid product ships the same way: one zip per platform, named
  <Product>-<version>-<Platform>.zip, holding that platform's installer(s), its plug-in
  bundles under their canonical names, the manual and a README. The site offers those
  zips and nothing else, so there is no flat folder in which an installer, a .dmg, a
  .deb and a bundle can overlap or be mistaken for one another.

  **This file is copied into each project's scripts\ folder** (release-packaging.ps1),
  the way ftp-changed.ps1 already is. Keep the copies identical: the point is that six
  products do not grow six slightly different packers, which is how NikVerb ended up
  publishing a Linux .clap under the name the Windows one used.

  Use it from a build script that has already decided its version:

      . (Join-Path $PSScriptRoot 'scripts\release-packaging.ps1')

      $zip = New-SitePackage -Product 'Nikkosynth' -Version $v -Platform 'Windows' `
          -Binaries (Join-Path $PSScriptRoot 'binaries') `
          -Include @(
              @{ Pattern = 'Nikkosynth_{v}_x64-setup.exe'; As = 'Nikkosynth-Setup-{v}.exe' },
              @{ Pattern = 'Nikkosynth.exe' },
              @{ Pattern = 'nikkosynth-cli.exe' }
          ) `
          -FromArchive @(
              @{ Archive = 'Nikkosynth_{v}_plugins_win.zip'; Path = 'Nikkosynth.vst3' },
              @{ Archive = 'Nikkosynth_{v}_plugins_win.zip'; Path = 'Nikkosynth.clap' }
          ) `
          -Documentation (Join-Path $PSScriptRoot 'documents') `
          -Readme $readmeLines

  `{v}` in a Pattern or an As is replaced by the version. A Pattern may be a wildcard
  (`*.dmg`): the first match is taken, so a build that produced one artifact needs no
  exact name.

  The zip always gets a Documentation\ folder (whatever PDF/DOCX/EPUB the folder holds)
  and a README.txt when -Readme lines are given.
#>

$ErrorActionPreference = 'Stop'

# **The zip assemblies are loaded here, at script scope, before anything that mentions them.**
# Windows PowerShell does not carry System.IO.Compression for free, and a type literal such as
# [IO.Compression.ZipArchiveMode] is resolved when the function holding it is compiled - which
# happens before that function's first line runs. An Add-Type *inside* the function is therefore
# too late to help it, and the build stops the first time a zip is written with
# "Unable to find type [IO.Compression.ZipArchiveMode]". Loading both assemblies once, up here,
# means every later [IO.Compression.*] resolves. System.IO.Compression carries ZipArchive,
# ZipArchiveMode and CompressionLevel; System.IO.Compression.FileSystem only adds ZipFile.
function Initialize-ZipAssembly {
  foreach ($assembly in @('System.IO.Compression', 'System.IO.Compression.FileSystem')) {
    $loaded = $false
    try { Add-Type -AssemblyName $assembly -ErrorAction Stop; $loaded = $true } catch { }
    if ($loaded) { continue }
    # A machine where Add-Type cannot answer the simple name still has the assembly next to
    # mscorlib; load that copy rather than fail a build that has already done its work.
    try {
      $dll = Join-Path ([IO.Path]::GetDirectoryName([string].Assembly.Location)) "$assembly.dll"
      if (Test-Path -LiteralPath $dll) { [void][System.Reflection.Assembly]::LoadFrom($dll) }
    } catch { }
  }
}
Initialize-ZipAssembly

function Get-SitePackageName([string]$Product, [string]$Version, [string]$Platform) {
  # The version sits in the name, so a one-version-on-disk purge recognises these.
  return ("{0}-{1}-{2}.zip" -f $Product, $Version, $Platform)
}

function Resolve-SitePackagePatterns {
  # **Every** file a Pattern means, newest first. `{v}` is the version.
  #
  # A wildcard can legitimately mean more than one file: the macOS build produces
  # `Nikkomander-macos-arm64.app.tar.gz` and `…-macos-x64.app.tar.gz`, one per architecture, and
  # both belong in the download. Taking only the newest (`$hits[0]`, which is what the single-file
  # helper below does) silently shipped the Intel .app and dropped the Apple Silicon one on
  # 2026-11-12 - and because the loose tarballs are deleted once the zip is written, the missing
  # one was gone from the disk too. A caller that wants just one file uses the helper, not this.
  param([string]$Dir, [string]$Pattern, [string]$Version)

  if (-not $Pattern) { return @() }
  $p = $Pattern.Replace('{v}', $Version)
  if (-not (Test-Path -LiteralPath $Dir)) { return @() }
  return @(Get-ChildItem -LiteralPath $Dir -Filter $p -ErrorAction SilentlyContinue |
           Sort-Object LastWriteTime -Descending)
}

function Resolve-SitePackagePattern {
  # The one file a Pattern means, or $null. `{v}` is the version; a wildcard matches the
  # newest, so a leftovers of an earlier build cannot be picked by accident. Use `All = $true`
  # in an -Include entry when every match is wanted (see Resolve-SitePackagePatterns).
  param([string]$Dir, [string]$Pattern, [string]$Version)

  $hits = @(Resolve-SitePackagePatterns -Dir $Dir -Pattern $Pattern -Version $Version)
  if (-not $hits.Count) { return $null }
  return $hits[0]
}

function Add-SitePackageItem {
  # Copy a file or a directory into the staging folder, whole. A VST3/CLAP bundle is a
  # folder, and half of one is worse than none.
  param([string]$Stage, [string]$Source, [string]$Leaf)

  if (-not (Test-Path -LiteralPath $Source)) { return $false }
  if (-not $Leaf) { $Leaf = Split-Path -Path $Source -Leaf }
  $dst = Join-Path $Stage $Leaf
  if (Test-Path -LiteralPath $dst) {
    Remove-Item -LiteralPath $dst -Recurse -Force -ErrorAction Stop
    # **Check, do not assume.** Copying a directory onto one that is still there puts it
    # *inside* instead of replacing it - the bundle then ships as
    # `NikVerb.vst3/NikVerb.vst3/Contents/...` and no host sees a plug-in. The remove
    # above fails silently when something holds the folder (a DAW with the plug-in
    # loaded, a scanner), so the failure is verified here and reported where it means
    # something.
    if (Test-Path -LiteralPath $dst) {
      throw "Cannot replace $dst - it is still there after deleting it. Close whatever has it open (a host with the plug-in loaded, or a virus scanner) and try again."
    }
  }
  Copy-Item -LiteralPath $Source -Destination $dst -Recurse -Force
  return $true
}

function Expand-SitePackageEntry {
  # Unpack a bundle that ships *inside* a platform archive, under its canonical name.
  #
  # **Entry by entry, with the separator normalised.** Windows PowerShell 5.1's
  # Compress-Archive writes entry names with backslashes, which is not the ZIP
  # specification: every tool except Explorer reads `Name.vst3\Contents\...` as one long
  # file name. Unpacking by hand costs ten lines and makes any archive readable.
  param([string]$Archive, [string]$RelPath, [string]$Stage, [string]$Leaf, [string]$Scratch)

  if (-not (Test-Path -LiteralPath $Archive)) { return $false }

  # **A fresh folder per archive per run, reused for every entry of that archive.**
  # A Windows package names its VST3 and its CLAP from the same plugins zip, and the
  # macOS one does the same - so the archive is unpacked once and both are copied out of
  # it. The folder is unique, not a fixed name that a previous run left behind: emptying
  # a folder can fail (a mounted or network path answers "Access is denied" to the
  # delete, and the failure is swallowed), and then the extractor tries to write over
  # files that are still there and gives up. The symptom is silent and partial - the
  # VST3 arrives, the CLAP does not.
  if (-not $script:NkUnpackCache) { $script:NkUnpackCache = @{} }
  if (-not $script:NkUnpackCache.ContainsKey($Archive)) {
    $tmp = Join-Path $Scratch ('_unpack-' + [IO.Path]::GetFileNameWithoutExtension($Archive) + '-' +
                               [guid]::NewGuid().ToString('N').Substring(0, 8))
    New-Item -ItemType Directory -Force -Path $tmp | Out-Null
    $tmpRoot = [IO.Path]::GetFullPath($tmp).TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar
    try {
      if ($Archive -like '*.zip') {
        Initialize-ZipAssembly
        $zip = [IO.Compression.ZipFile]::OpenRead($Archive)
        try {
          foreach ($entry in $zip.Entries) {
            $rel = $entry.FullName -replace '\\', '/'
            if ($rel.EndsWith('/')) { continue }
            $dstPath = [IO.Path]::GetFullPath((Join-Path $tmp ($rel -replace '/', [IO.Path]::DirectorySeparatorChar)))
            # **An entry may not land outside the folder it is unpacked into.** `..\..\x` is a
            # legal name inside a zip; written as-is it would put a file anywhere on the disk.
            if (-not $dstPath.StartsWith($tmpRoot, [StringComparison]::OrdinalIgnoreCase)) {
              Write-Host ("   skipped an entry pointing outside the archive: {0}" -f $entry.FullName) -ForegroundColor Yellow
              continue
            }
            $parent = Split-Path -Path $dstPath -Parent
            if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Force -Path $parent | Out-Null }
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $dstPath, $true)
          }
        } finally { $zip.Dispose() }
      } else {
        # tar.exe ships with Windows 10 and reads .tar.gz; `tar` is the same binary on Linux.
        $tar = Get-Command tar.exe -ErrorAction SilentlyContinue
        if (-not $tar) { $tar = Get-Command tar -ErrorAction SilentlyContinue }
        if (-not $tar) {
          Write-Host "   no tar available: cannot unpack $(Split-Path $Archive -Leaf)" -ForegroundColor Yellow
          Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
          return $false
        }
        & $tar.Source -xzf $Archive -C $tmp 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) {
          Write-Host "   tar failed on $(Split-Path $Archive -Leaf)" -ForegroundColor Yellow
          Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
          return $false
        }
      }
    } catch {
      Write-Host "   could not unpack $(Split-Path $Archive -Leaf): $($_.Exception.Message)" -ForegroundColor Yellow
      Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
      return $false
    }
    # **Only a complete unpack is remembered.** The cache used to be filled before the
    # extraction ran, so a failure halfway through (a truncated archive, a locked file) left
    # a half-filled folder that the *next* entry of the same archive read from - the VST3
    # arrived, the CLAP did not, and nothing said why. The marker is the proof that the
    # folder holds everything, and the filter below skips it when listing the contents.
    [IO.File]::WriteAllText((Join-Path $tmp '.nk-extracted'), ($Archive + "`r`n"), (New-Object Text.UTF8Encoding $false))
    $script:NkUnpackCache[$Archive] = $tmp
  } else {
    $tmp = $script:NkUnpackCache[$Archive]
  }

  # An empty RelPath means "everything this archive holds", each entry keeping its own
  # name. That is how a platform archive (a .zip or .tar.gz that is already the
  # package) is unpacked into the download instead of being nested inside it - nobody
  # wants Nikpak.zip\Nikpak.zip.
  if (-not $RelPath) {
    $any = $false
    foreach ($entry in @(Get-ChildItem -LiteralPath $tmp -Force | Where-Object { $_.Name -ne '.nk-extracted' })) {
      if (Add-SitePackageItem -Stage $Stage -Source $entry.FullName -Leaf $entry.Name) { $any = $true }
    }
    return $any
  }

  $src = Join-Path $tmp $RelPath
  if (-not (Test-Path -LiteralPath $src)) { return $false }
  return (Add-SitePackageItem -Stage $Stage -Source $src -Leaf $Leaf)
}

function Write-SitePackageZip {
<#
  Write the staging folder as a zip, with **forward slashes in every entry name**.

  **Not Compress-Archive.** On Windows PowerShell 5.1 (and Windows PowerShell in
  general) it writes entry names with backslashes: `NikVerb.vst3\Contents\...`. The ZIP
  specification says the separator is `/`, and only Explorer is forgiving about it - on
  macOS, on Linux, and in Python's zipfile the backslash is an ordinary character in a
  file name, so a Windows-made archive extracts as one long file called
  `NikVerb.vst3\Contents\x86_64-linux\NikVerb.so`. A Linux user unpacking that gets no
  plug-in at all. For a download that exists to be opened on macOS and Linux, that is
  the difference between working and not.

  Written entry by entry, which also lets the timestamp and the compression level be
  set once instead of relying on defaults.
#>
  param([Parameter(Mandatory)][string]$Stage, [Parameter(Mandatory)][string]$Path)

  Initialize-ZipAssembly
  $stageFull = (Resolve-Path -LiteralPath $Stage).Path
  $root = $stageFull.TrimEnd('\', '/') + [IO.Path]::DirectorySeparatorChar

  # Mode and level are given as strings: PowerShell converts them to the enum when it binds the
  # call, so no [IO.Compression.ZipArchiveMode] / CompressionLevel literal is needed here either.
  $zip = [IO.Compression.ZipFile]::Open($Path, 'Create')
  try {
    foreach ($file in @(Get-ChildItem -LiteralPath $stageFull -Recurse -File -Force |
                        Sort-Object FullName)) {
      $rel = $file.FullName.Substring($root.Length).Replace([IO.Path]::DirectorySeparatorChar, '/')
      if ($rel.StartsWith('/')) { $rel = $rel.Substring(1) }
      $entry = $zip.CreateEntry($rel, 'Optimal')
      # A zip stores a 1980..2107 date. A file outside that range (a restored archive, a
      # build tool writing epoch 0) makes this setter throw, and the pack - which has
      # already done all the copying - dies at its last step. Clamp instead.
      $stamp = $file.LastWriteTime
      if ($stamp.Year -lt 1980) { $stamp = [DateTime]::new(1980, 1, 1) }
      elseif ($stamp.Year -gt 2107) { $stamp = [DateTime]::new(2107, 12, 31) }
      $entry.LastWriteTime = [DateTimeOffset]::new($stamp)
      $in = $file.OpenRead()
      try {
        $out = $entry.Open()
        try { $in.CopyTo($out, 1048576) } finally { $out.Dispose() }
      } finally { $in.Dispose() }
    }
  } finally {
    $zip.Dispose()
  }
}

function New-SitePackage {
<#
  Build <Product>-<Version>-<Platform>.zip and return its path, or $null when the
  platform has nothing to pack.

  -Binaries   the folder the build stages into (one of these per project)
  -Include    @{ Pattern = '<file or wildcard>'; As = '<name in the zip>'; All = $true }
              `All = $true` packs every file the pattern matches, each under its own name - the
              macOS build needs it for the two architecture .app tarballs. Without it a wildcard
              means the newest match only.
  -FromArchive @{ Archive = '<archive in -Binaries>'; Path = '<path inside>'; As = '<name>' }
  -Documentation  folder holding the manual
  -Readme     the lines of README.txt
#>
  param(
    [Parameter(Mandatory)][string]$Product,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Platform,
    [Parameter(Mandatory)][string]$Binaries,
    [hashtable[]]$Include = @(),
    [hashtable[]]$FromArchive = @(),
    [string]$Documentation = '',
    [string]$OutDir = '',
    [string]$Scratch = '',
    [string[]]$Readme = @(),
    [switch]$Quiet
  )

  if (-not $OutDir) { $OutDir = $Binaries }
  if (-not $Scratch) {
    # **Never inside -OutDir.** A staging folder under binaries\ is a folder that gets
    # copied, checksummed and uploaded: NikSQL's buildweb mirrors binaries\ to the
    # server recursively, so `binaries\target\packages\stage-...` would publish the
    # half-built files as downloads. Beside it, under the project's own target\, instead.
    $Scratch = Join-Path (Split-Path -Parent $OutDir) 'target\packages'
  }
  # Both, not just the scratch: -OutDir may be a folder that does not exist yet (a fresh
  # build directory), and ZipFile.Open does not create one.
  New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
  New-Item -ItemType Directory -Force -Path $Scratch | Out-Null

  # And refuse the mistake outright, in case a caller passes one by hand. The comparison is
  # case-insensitive, which is how Windows itself treats these paths: binaries\Target and
  # binaries\target are one folder, and a check that disagrees would let the stage through.
  $outFull = [IO.Path]::GetFullPath($OutDir).TrimEnd('\', '/')
  $scrFull = [IO.Path]::GetFullPath($Scratch).TrimEnd('\', '/')
  if ($scrFull -eq $outFull -or
      $scrFull.StartsWith($outFull + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw "The staging folder ($Scratch) is inside the folder being published ($OutDir). Build it under target\ instead: anything left behind there would be uploaded."
  }

  # **A stage folder no other run has ever used.** The name used to be fixed
  # (stage-<product>-<platform>) and emptied before use with -ErrorAction SilentlyContinue:
  # when that delete fails - a scanner reading a freshly built .exe, Explorer holding one -
  # the previous build's files stay behind and are packed into the new zip under names this
  # build never produced. The same trap the unpack cache below is written to avoid. A unique
  # suffix cannot inherit anything; the sweep at the end of this function removes the older
  # ones once a run has succeeded.
  $stage = Join-Path $Scratch ("stage-{0}-{1}-{2}" -f $Product, $Platform,
                               [guid]::NewGuid().ToString('N').Substring(0, 8))
  New-Item -ItemType Directory -Force -Path $stage | Out-Null

  $got = @()
  $missing = @()
  foreach ($item in $Include) {
    $hits = @(Resolve-SitePackagePatterns -Dir $Binaries -Pattern $item.Pattern -Version $Version)
    if (-not $hits.Count) { $missing += $item.Pattern; continue }
    # `All = $true` packs every file the pattern matches (the two macOS .app tarballs). Without
    # it, only the newest, which is what a single-artifact pattern wants.
    $take = if ($item.All) { $hits } else { @($hits[0]) }
    foreach ($src in $take) {
      # A name given with -As only fits one file; several keep their own names, which is what
      # makes the two architecture tarballs distinguishable.
      $leaf = if ($item.As -and $take.Count -eq 1) { $item.As.Replace('{v}', $Version) } else { $src.Name }
      if (Add-SitePackageItem -Stage $stage -Source $src.FullName -Leaf $leaf) { $got += $leaf }
    }
  }

  # **A package that is missing a file is worse than no package.** If the Linux build
  # produced a .deb but not the .rpm, or the legs have not run at all, the zip would ship
  # whatever happened to be on disk and look complete: a user downloads "Linux", gets a
  # bare binary, and there is nothing on the page to say why. Say it here instead, in the
  # build log, naming the patterns that found nothing.
  if ($missing.Count) {
    Write-Host ("   warning: {0} of {1} expected file(s) for {2} were not found in {3}:" -f
                $missing.Count, $Include.Count, $Platform, $Binaries) -ForegroundColor Yellow
    foreach ($m in $missing) { Write-Host "     $m" -ForegroundColor Yellow }
    Write-Host ("   the $Platform download will be incomplete - build that platform first" ) -ForegroundColor Yellow
  }
  foreach ($a in $FromArchive) {
    $arch = Resolve-SitePackagePattern -Dir $Binaries -Pattern $a.Archive -Version $Version
    if (-not $arch) {
      Write-Host ("   {0}: no {1} to unpack" -f $a.Path, $a.Archive) -ForegroundColor DarkYellow
      continue
    }
    $leaf = if ($a.As) { $a.As } else { Split-Path -Path $a.Path -Leaf }
    if (Expand-SitePackageEntry -Archive $arch.FullName -RelPath ($a.Path.Replace('{v}', $Version)) -Stage $stage -Leaf $leaf -Scratch $Scratch) {
      $got += $leaf
    } else {
      Write-Host ("   {0}: not found in {1}" -f $a.Path, $arch.Name) -ForegroundColor DarkYellow
    }
  }

  if ($got.Count -eq 0) {
    if (-not $Quiet) { Write-Host "   nothing to pack for $Platform" -ForegroundColor DarkYellow }
    Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
    return $null
  }

  # The manual travels with the software: a download without its documentation is the
  # one people ask support questions about.
  if ($Documentation -and (Test-Path -LiteralPath $Documentation)) {
    $docs = Join-Path $stage 'Documentation'
    $n = 0
    # A manual as a document...
    foreach ($f in @(Get-ChildItem -LiteralPath $Documentation -File -ErrorAction SilentlyContinue |
                     Where-Object { $_.Extension -in @('.pdf', '.docx', '.epub') })) {
      New-Item -ItemType Directory -Force -Path $docs | Out-Null
      Copy-Item -LiteralPath $f.FullName -Destination (Join-Path $docs $f.Name) -Force
      $n++
    }
    # ...or, for a product whose manual is a set of web pages, those. A download with no
    # documentation in it is the one that produces support questions.
    if ($n -eq 0) {
      $pages = @(Get-ChildItem -LiteralPath $Documentation -File -Filter '*.html' -ErrorAction SilentlyContinue)
      if ($pages.Count) {
        New-Item -ItemType Directory -Force -Path $docs | Out-Null
        foreach ($f in $pages) { Copy-Item -LiteralPath $f.FullName -Destination (Join-Path $docs $f.Name) -Force }
        $n = $pages.Count
      }
    }
    if ($n) { $got += 'Documentation' }
  }

  if ($Readme.Count) {
    [IO.File]::WriteAllText((Join-Path $stage 'README.txt'), (($Readme -join "`r`n") + "`r`n"),
                            (New-Object Text.UTF8Encoding $false))
    $got += 'README.txt'
  }

  $name = Get-SitePackageName $Product $Version $Platform
  $zipPath = Join-Path $OutDir $name

  # **Written to a .part name and renamed at the end.** Compressing a few hundred
  # megabytes takes minutes; a build that is interrupted, or a full disk, used to leave
  # a truncated zip under the real name - which is then published as the download and
  # fails only in the user's hands. A .part file is never mistaken for the package, and
  # the rename is atomic on one volume.
  $part = "$zipPath.part"
  if (Test-Path -LiteralPath $part) { Remove-Item -LiteralPath $part -Force -ErrorAction SilentlyContinue }
  Write-SitePackageZip -Stage $stage -Path $part

  # A quick integrity check: a zip ends with its central directory, and a truncated one
  # cannot be opened. Cheaper than shipping a bad file, and it names the real cause.
  try {
    Initialize-ZipAssembly
    $check = [IO.Compression.ZipFile]::OpenRead($part)
    $entries = $check.Entries.Count
    $check.Dispose()
    if ($entries -lt 1) { throw 'the archive is empty' }
  } catch {
    Remove-Item -LiteralPath $part -Force -ErrorAction SilentlyContinue
    throw "The $Platform download could not be written completely ($($_.Exception.Message)). Is the disk full?"
  }

  if (Test-Path -LiteralPath $zipPath) {
    # A running program, or Explorer, can hold the old zip; renaming it out of the way
    # is enough, and beats failing a build that has already done all its work.
    try { Remove-Item -LiteralPath $zipPath -Force -ErrorAction Stop }
    catch {
      try { Move-Item -LiteralPath $zipPath -Destination "$zipPath.old-" -Force -ErrorAction Stop }
      catch { throw "Cannot write $name - it is open in another program." }
    }
  }
  Move-Item -LiteralPath $part -Destination $zipPath -Force

  Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
  Get-ChildItem -LiteralPath $Scratch -Directory -Filter '_unpack-*' -ErrorAction SilentlyContinue |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
  # Staging folders of earlier runs of this same product and platform - a crashed build, or a
  # run whose delete was refused. Scoped to this product/platform so a parallel leg's stage
  # (another platform, its own process) is never touched.
  Get-ChildItem -LiteralPath $Scratch -Directory -Filter ("stage-{0}-{1}*" -f $Product, $Platform) -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -ne $stage } |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue }
  # Scatter from an interrupted run: a `.old-` copy of a zip is never wanted.
  Get-ChildItem -LiteralPath $OutDir -File -Filter '*.zip.old-*' -ErrorAction SilentlyContinue |
    ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force -ErrorAction SilentlyContinue }

  if (-not $Quiet) {
    $mb = (Get-Item -LiteralPath $zipPath).Length / 1MB
    Write-Host ("   {0}  {1:N1} MB  ({2})" -f $name, $mb, ($got -join ', '))
  }
  return $zipPath
}

function Remove-SupersededFiles {
  # **Keep the folder clean and current.** Once a platform's download is a zip, the loose
  # files that went into it are dead weight in binaries\: an installer beside the zip that
  # contains it, a plug-in bundle three platforms used to share, an archive that is now
  # unpacked. They are deleted, not archived - the zip is the artifact, and a second copy
  # is how someone ships last week's file.
  #
  # Only files are considered, never directories that were not asked for, and `-Keep`
  # always wins.
  param(
    [Parameter(Mandatory)][string]$Dir,
    [string[]]$Patterns = @(),
    [string[]]$Keep = @(),
    # **`{v}` must be expanded here too.** It is the version placeholder used by -Include, and a
    # caller naturally writes the same patterns for the two calls. This function did not expand
    # it: on 2026-11-12 `Nikkomander_{v}_aarch64.dmg` matched nothing, so the two .dmg files
    # stayed in binaries\ beside the zip that already contained them - while the pattern without
    # {v} (the .app tarballs) did match and was deleted, which hid the problem.
    [string]$Version = '',
    [switch]$WhatIf
  )

  $removed = @()
  if (-not (Test-Path -LiteralPath $Dir)) { return , $removed }
  foreach ($p0 in $Patterns) {
    $p = if ($Version) { $p0.Replace('{v}', $Version) } else { $p0 }
    foreach ($f in @(Get-ChildItem -LiteralPath $Dir -File -Filter $p -ErrorAction SilentlyContinue)) {
      if ($Keep -contains $f.Name) { continue }
      if ($WhatIf) { $removed += $f.Name; continue }
      try { Remove-Item -LiteralPath $f.FullName -Force -ErrorAction Stop; $removed += $f.Name }
      catch { Write-Host "   could not delete $($f.Name): $($_.Exception.Message)" -ForegroundColor Yellow }
    }
  }
  return , $removed
}

function Remove-OtherDownloads {
  # **One version, one scheme, on the site.** Anything in the downloads folder that is not
  # one of the current packages is an older build or an older layout - a loose .dmg, a
  # bundle three platforms used to share - and must not sit beside them or be uploaded.
  param([string]$Downloads, [string[]]$Keep)

  $removed = @()
  if (-not (Test-Path -LiteralPath $Downloads)) { return , $removed }
  foreach ($item in @(Get-ChildItem -LiteralPath $Downloads -Force)) {
    if ($item.Name -in @('.gitkeep', 'SHA256SUMS.txt', '.htaccess', 'web.config')) { continue }
    if ($Keep -contains $item.Name) { continue }
    try {
      Remove-Item -LiteralPath $item.FullName -Recurse -Force -ErrorAction Stop
      $removed += $item.Name
    } catch {
      Write-Host "   note: could not remove $($item.Name) - $($_.Exception.Message)" -ForegroundColor Yellow
    }
  }
  return , $removed
}

function Get-PlatformPackages {
  # The <Product>-<Version>-<Platform>.zip files already in a folder, so a build that made
  # only one platform does not have to repack the others.
  param([string]$Product, [string]$Version, [string]$Dir)

  if (-not (Test-Path -LiteralPath $Dir)) { return @() }
  return @(Get-ChildItem -LiteralPath $Dir -File -ErrorAction SilentlyContinue |
           Where-Object { $_.Name -like "$Product-$Version-*.zip" })
}

function Publish-SiteDownloads {
<#
  Pack this platform's zip and make the site's downloads folder hold exactly the
  packages of this version - this platform's plus any other platform's already built -
  and nothing else. One call per build leg; the legs then cannot disagree with each
  other about what the site offers.

  Returns the zip path (or $null when nothing matched), so a caller can say what it did.
#>
  param(
    [Parameter(Mandatory)][string]$Product,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Platform,
    [Parameter(Mandatory)][string]$Binaries,
    [Parameter(Mandatory)][string]$Downloads,
    [hashtable[]]$Include = @(),
    [hashtable[]]$FromArchive = @(),
    [string]$Documentation = '',
    [string[]]$Readme = @(),
    [switch]$Quiet
  )

  $zip = New-SitePackage -Product $Product -Version $Version -Platform $Platform -Binaries $Binaries `
                         -Include $Include -FromArchive $FromArchive -Documentation $Documentation `
                         -Readme $Readme -Quiet:$Quiet
  if (-not $zip) { return $null }

  $keep = @((Split-Path -Path $zip -Leaf), 'SHA256SUMS.txt')
  foreach ($z in @(Get-PlatformPackages -Product $Product -Version $Version -Dir $Binaries)) {
    if ($keep -notcontains $z.Name) { $keep += $z.Name }
  }

  foreach ($d in @(Remove-OtherDownloads -Downloads $Downloads -Keep $keep)) {
    Write-Host "   removed an older download: $d" -ForegroundColor DarkYellow
  }
  $n = Copy-Downloads -From $Binaries -To $Downloads -Names $keep
  if (-not $Quiet) { Write-Host "   $n download(s) in the site's folder" }
  return $zip
}

function Copy-Downloads {
  # Copy the packages (and the checksums) into the site's downloads folder. Never throws:
  # one file held open must not fail a build that has already compiled everything.
  param([string]$From, [string]$To, [string[]]$Names)

  New-Item -ItemType Directory -Force -Path $To | Out-Null
  $n = 0
  foreach ($name in $Names) {
    $src = Join-Path $From $name
    if (-not (Test-Path -LiteralPath $src)) { continue }
    $dst = Join-Path $To $name
    $same = $false
    if (Test-Path -LiteralPath $dst) {
      $a = Get-Item -LiteralPath $src -Force
      $b = Get-Item -LiteralPath $dst -Force
      # Two seconds, because not every filesystem keeps sub-second times (FAT rounds to
      # 2 s, a Windows share stores whole seconds).
      $same = (-not $a.PSIsContainer) -and ($a.Length -eq $b.Length) -and
              ([Math]::Abs(($a.LastWriteTimeUtc - $b.LastWriteTimeUtc).TotalSeconds) -le 2)
    }
    if ($same) { $n++; continue }
    try { Copy-Item -LiteralPath $src -Destination $dst -Recurse -Force -ErrorAction Stop; $n++ }
    catch { Write-Host "   could not copy $name : $($_.Exception.Message)" -ForegroundColor Yellow }
  }
  return $n
}
