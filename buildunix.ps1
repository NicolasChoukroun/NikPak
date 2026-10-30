<#
.SYNOPSIS
  NikPak - build the LINUX version on GitHub Actions and download it into binaries\.

.DESCRIPTION
  NikPak is built on Linux, so the build runs on GitHub Actions (.github\workflows\build.yml).
  This script starts the "linux" leg, waits for it and downloads the result into binaries\:
      binaries\nikpak-<version>-linux-x86_64.tar.gz

  What you need once:
    1. The project on GitHub (NicolasChoukroun/nikpak) with your latest changes pushed.
    2. GitHub CLI: https://cli.github.com, then once:  gh auth login

  The script keeps only the latest Linux package: it clears the Linux files from binaries\
  first, and leaves the Windows / macOS files untouched.

.PARAMETER Ref
  Branch or tag to build on GitHub (default: the current branch).

.EXAMPLE
  .\buildunix.ps1
  .\buildunix.ps1 -Ref v1.6.2
  powershell -ExecutionPolicy Bypass -File buildunix.ps1      (if scripts are blocked)
#>
param([string]$Ref = '')

$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $PSScriptRoot
$Dist = Join-Path $PSScriptRoot 'binaries'
$Workflow = '.github\workflows\build.yml'
$Platforms = 'linux'
$Artifacts = @('nikpak-linux')

function Step([string]$msg) { Write-Host ''; Write-Host "== $msg" -ForegroundColor Cyan }

function Ensure-Gh {
  # gh.exe is not always on PATH on Windows (it lives in "Program Files\GitHub CLI").
  if (Get-Command gh -ErrorAction SilentlyContinue) { return }
  $candidates = @()
  if ($env:ProgramFiles)          { $candidates += Join-Path $env:ProgramFiles 'GitHub CLI\gh.exe' }
  if (${env:ProgramFiles(x86)})   { $candidates += Join-Path ${env:ProgramFiles(x86)} 'GitHub CLI\gh.exe' }
  if ($env:LOCALAPPDATA)          { $candidates += Join-Path $env:LOCALAPPDATA 'Programs\GitHub CLI\gh.exe' }
  if ($env:USERPROFILE) {
    $candidates += Join-Path $env:USERPROFILE 'scoop\apps\gh\current\bin\gh.exe'
    $candidates += Join-Path $env:USERPROFILE 'scoop\shims\gh.exe'
  }
  foreach ($p in $candidates) {
    if (Test-Path -LiteralPath $p) {
      $env:PATH = (Split-Path $p) + ';' + $env:PATH
      Write-Host "   found GitHub CLI at $p (it was not on PATH)" -ForegroundColor Yellow
      return
    }
  }
  throw 'The GitHub CLI (gh) could not be found. Install it from https://cli.github.com, then run "gh auth login".'
}

# Ask GitHub whether the workflow is actually pushed (and accepts the `platforms`
# input): a local-only edit makes `gh workflow run` fail with an HTTP 404 / 422.
function Confirm-RemoteWorkflow {
  $ownerRepo = (& gh repo view --json nameWithOwner --jq .nameWithOwner 2>$null | Out-String).Trim()
  if (-not $ownerRepo) { throw 'Could not read the GitHub repository name (gh repo view).' }
  $raw = (& gh api "repos/$ownerRepo/contents/.github/workflows/build.yml" -H 'Accept: application/vnd.github.raw' 2>$null | Out-String)
  if (-not $raw -or $raw -notmatch 'platforms') {
    throw "$Workflow is not pushed to GitHub yet (or is an older copy without the 'platforms' input). Run:  git add -A; git commit -m 'build workflow'; git push   then run this script again."
  }
}

function Check-Remote {
  Step 'Linux: checking GitHub'
  Ensure-Gh
  & gh auth status *> $null
  if ($LASTEXITCODE) { throw 'GitHub CLI is not logged in: run  gh auth login' }
  & git rev-parse --is-inside-work-tree *> $null
  if ($LASTEXITCODE) { throw 'This folder is not a git repository. Put it on GitHub first.' }
  if (-not (Test-Path $Workflow)) { throw "$Workflow is missing." }
  if (-not (Select-String -Path $Workflow -Pattern 'platforms' -SimpleMatch -Quiet)) {
    throw "$Workflow does not accept the 'platforms' input - it needs the multi-platform workflow with a prepare job."
  }
  Confirm-RemoteWorkflow
}

function Start-RemoteBuild {
  Step 'Linux: starting the build on GitHub Actions'
  $branch = $Ref
  if (-not $branch) { $branch = (& git rev-parse --abbrev-ref HEAD).Trim() }
  if (& git status --porcelain) { Write-Warning 'You have uncommitted changes: GitHub builds what is pushed, not the files on this PC.' }
  $null = & git fetch --quiet origin $branch 2>$null
  if (-not $LASTEXITCODE) {
    $ahead = (& git rev-list --count "origin/$branch..HEAD" 2>$null)
    if ($ahead -and [int]$ahead -gt 0) { Write-Warning "$ahead commit(s) are not pushed yet (git push): GitHub will build the older version." }
  }
  $since = (Get-Date).ToUniversalTime().AddSeconds(-15)
  $null = & gh workflow run build.yml --ref $branch -f platforms=$Platforms
  if ($LASTEXITCODE) { throw 'Could not start the GitHub workflow (is .github\workflows\build.yml pushed?)' }
  for ($i = 0; $i -lt 40; $i++) {
    Start-Sleep -Seconds 3
    $runs = & gh run list --workflow build.yml --branch $branch --event workflow_dispatch --limit 1 --json 'databaseId,createdAt,url' | ConvertFrom-Json
    if ($runs -and ([datetime]$runs[0].createdAt).ToUniversalTime() -ge $since) {
      $script:RunId  = [long]$runs[0].databaseId
      $script:RunUrl = [string]$runs[0].url
      Write-Host "   started: $($script:RunUrl)"
      return
    }
  }
  throw 'The GitHub run did not show up; look at the Actions tab of the repository.'
}

function Get-RemoteBuild($runId) {
  Step "Linux: waiting for the GitHub build (run $runId, usually 10-20 minutes)"
  & gh run watch $runId --exit-status --interval 30
  if ($LASTEXITCODE) { Write-Warning "The Linux build failed. Details: gh run view $runId --log-failed" }
  Step 'Linux: downloading into binaries\'
  New-Item -ItemType Directory -Force -Path $Dist | Out-Null
  $tmp = Join-Path ([IO.Path]::GetTempPath()) "nikpak-unix-$runId"
  if (Test-Path $tmp) { Remove-Item $tmp -Recurse -Force }
  $ok = $false
  foreach ($name in $Artifacts) {
    $to = Join-Path $tmp $name
    & gh run download $runId --name $name --dir $to
    if ($LASTEXITCODE) { Write-Warning "$name failed to download (no files)."; continue }
    Copy-Item (Join-Path $to '*') $Dist -Recurse -Force
    $ok = $true
  }
  Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
  if (-not $ok) { throw 'No Linux files were downloaded.' }
}

# keep only the latest Linux build: clear the Linux packages, leave Windows / macOS files alone
Step 'Cleaning Linux files from binaries\'
if (Test-Path $Dist) {
  Get-ChildItem $Dist -File |
    Where-Object { $_.Name -like 'nikpak-*-linux-*.tar.gz' } |
    Remove-Item -Force
}
else { New-Item -ItemType Directory -Force -Path $Dist | Out-Null }

$ok = $false
try {
  Check-Remote
  Start-RemoteBuild          # sets $script:RunId / $script:RunUrl
  Get-RemoteBuild $script:RunId
  $ok = $true
}
catch { Write-Host $_.Exception.Message -ForegroundColor Red }

Step 'Summary'
if (-not $ok) { Write-Host '   Linux build FAILED' -ForegroundColor Red; exit 1 }
Write-Host '   Linux build ok:' -ForegroundColor Green
$files = Get-ChildItem $Dist -File |
  Where-Object { $_.Name -like 'nikpak-*-linux-*.tar.gz' } | Sort-Object Name
if (-not $files) { Write-Host '   (nothing in binaries\ yet)' }
foreach ($f in $files) { Write-Host ('   {0,-72} {1,8:N1} MB' -f $f.FullName, ($f.Length / 1MB)) }
