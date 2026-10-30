<#
.SYNOPSIS
  NikPak - build Windows (nikpak.exe + unNik.exe + the two setups), no upload.

.DESCRIPTION
  Windows-only convenience script: runs buildall.ps1 with -NoUpload, so it builds the Windows
  binaries and installers, copies them to binaries\ and rebuilds the website, but skips the FTP
  upload. Use  .\buildwin.ps1 -Upload  to also upload.

  (buildunix.ps1 / buildmac.ps1 do not apply here: NikPak is a command-line tool that builds
  natively on macOS / Linux with ./build.sh, not through GitHub Actions.)

.EXAMPLE
  .\buildwin.ps1
  .\buildwin.ps1 -Test
  .\buildwin.ps1 -Upload
#>
param([switch]$Test, [switch]$Clean, [switch]$Upload)

# $args is a reserved automatic variable; use a real variable so the switches reach buildall.ps1
$buildArgs = @()
if ($Test)   { $buildArgs += '-Test' }
if ($Clean)  { $buildArgs += '-Clean' }
if (-not $Upload) { $buildArgs += '-NoUpload' }

& (Join-Path $PSScriptRoot 'buildall.ps1') @buildArgs
exit $LASTEXITCODE
