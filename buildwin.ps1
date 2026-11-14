<#
.SYNOPSIS
  NikPak - build Windows (nikpak.exe + unNik.exe + the two setups), no upload.

.DESCRIPTION
  Windows-only convenience script: gives this build its own version number, writes that
  number into every source file, then runs buildall.ps1 -NoUpload, so it builds the
  Windows binaries and installers, packs Nikpak-<ver>-Windows.zip, puts it where the
  site's download button reads it, rebuilds the website and commits + pushes the
  release source - without the FTP upload. Use  .\buildwin.ps1 -Upload  to also upload.

  **The download.** The zip is published into website\downloads\ AFTER the site is
  rebuilt (a rebuild wipes dist\), and its real size is written into website\site.json,
  so the button on the download page always points at a file that exists. That is what
  was missing on 2026-11-14, when buildwin packed the zip into binaries\ and the site's
  download folder stayed empty.

  **The commit.** The build edits Cargo.toml, Cargo.lock, site.json, version.json and
  dist\. It ends by committing and pushing them (commit.ps1), because buildmac.ps1 and
  buildunix.ps1 build the PUSHED commit and refuse to start on a dirty tree. -SkipCommit
  leaves that to you.

  VERSION POLICY (Cargo.toml: "every delivered build gets a new number")
    By default the patch number is incremented before building: 1.13.1 -> 1.13.2.
    The new number is written to Cargo.toml, Cargo.lock, website\site.json and
    website\version.json before anything is compiled, so the exe you build and the
    download links on the site always agree. If the build fails, the old number is
    restored, so a failed run does not burn a version.

    -Version 1.14.0   build that exact number (for a minor or major release)
    -NoBump           rebuild the current number (fix a failed build, no new number)

  buildmac.ps1 and buildunix.ps1 read the same Cargo.toml and refuse to run when the
  pushed source has a different version, so all three platforms carry one number.

.EXAMPLE
  .\buildwin.ps1                  # 1.13.1 -> 1.13.2, build locally and push
  .\buildwin.ps1 -Version 1.14.0  # release 1.14.0
  .\buildwin.ps1 -NoBump          # rebuild 1.13.1 as it is
  .\buildwin.ps1 -Test            # run the unit tests first
  .\buildwin.ps1 -SkipCommit      # build, but leave the commit to me
  .\buildwin.ps1 -Upload          # build 1.13.2 and upload the website
#>
param(
    [switch]$Test,
    [switch]$Clean,
    [switch]$Upload,
    [switch]$NoBump,
    [switch]$SkipCommit,
    [string]$Version
)

$ErrorActionPreference = 'Stop'
$root = $PSScriptRoot
. (Join-Path $root 'scripts\version.ps1')

$current = Get-NikpakVersion $root
if ($Version) {
    if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "-Version '$Version' is not x.y.z (for example 1.14.0)." }
    $new = $Version
} elseif ($NoBump) {
    $new = $current
} else {
    $new = Get-NextNikpakVersion $current
}

if ($new -ne $current) {
    Write-Host ''
    Write-Host "=== Version $current -> $new" -ForegroundColor Cyan
    Set-NikpakVersion $root $new
} else {
    Write-Host ''
    Write-Host "=== Version $new (unchanged)" -ForegroundColor Cyan
}

# every source file must carry the number this build will carry
Assert-NikpakVersionSync $root $new

# Forward switches via a hashtable (array splatting of '-Switch' strings would pass them as
# positional values, which buildall.ps1 would mis-bind). $args is a reserved automatic variable,
# so a real variable is used.
$params = @{}
if ($Test)         { $params['Test']       = $true }
if ($Clean)        { $params['Clean']      = $true }
if ($SkipCommit)   { $params['SkipCommit'] = $true }
if (-not $Upload)  { $params['NoUpload']   = $true }
$params['Version'] = $new

# try/catch: buildall.ps1 can both exit (sets $LASTEXITCODE) and throw; either way the
# rollback below must run when the build did not succeed.
$code = 0
try {
    & (Join-Path $root 'buildall.ps1') @params
    $code = $LASTEXITCODE
} catch {
    Write-Host ''
    Write-Host "Build error: $($_.Exception.Message)" -ForegroundColor Red
    $code = 1
}

# A failed build must not consume a version number.
if ($code -ne 0 -and $new -ne $current) {
    Write-Host ''
    Write-Host "=== Build failed - restoring version $current" -ForegroundColor Yellow
    Set-NikpakVersion $root $current
    # and remove whatever this failed run already produced under the abandoned number
    Remove-OldNikpakFiles -Root $root -Version $current -Dirs @('binaries', 'dist', 'website\downloads')
}

exit $code
