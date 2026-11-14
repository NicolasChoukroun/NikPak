<#
  release-commit.ps1 - commit and push the release source, from inside a build script.

  Every NikPak build edits the source: the version bump (version.ps1) and the site
  rewrites (download sizes, checksums). buildmac.ps1 / buildunix.ps1 build what is
  PUSHED and refuse to start on a tree that has not been committed, so a release
  whose last step leaves the tree dirty makes the next script fail. This is the
  same fix NikEdit got on 2026-11-14.

  It does not re-implement any of it: it runs commit.ps1, which already leaves
  build output and secrets out, refuses a file near GitHub's limit, sets the
  upstream on the first push and retries a push GitHub drops.

      . (Join-Path $PSScriptRoot 'scripts\release-commit.ps1')
      $GitStatus = Complete-ReleaseSource -Root $PSScriptRoot -SkipCommit:$SkipCommit

  Returns one line for the build summary:
      'committed and pushed'   'pushed (nothing new to commit)'
      'FAILED (see the messages above)'                       'skipped (-SkipCommit)'
#>

$ErrorActionPreference = 'Stop'

function Assert-PushedReleaseSource {
<#
  GitHub builds the PUSHED commit, never the files on this PC. buildmac.ps1 and
  buildunix.ps1 therefore refuse to start unless the version they are about to build
  is the one on origin - otherwise a Windows release 1.14.3 and a macOS package 1.14.2
  end up on the same download page.

  NikEdit gained the same guard on 2026-11-14; here it replaces two warnings that the
  build happily ignored.
#>
  param(
    [Parameter(Mandatory)][string]$Root,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Branch,
    [Parameter(Mandatory)][string]$Platform
  )

  $scriptName = 'buildunix.ps1'
  if ($Platform -eq 'macOS') { $scriptName = 'buildmac.ps1' }

  Push-Location -LiteralPath $Root
  try {
    $dirty = @(& git status --porcelain 2>$null | Where-Object { "$_".Trim() })
    if ($dirty.Count) {
      throw ("This folder has $($dirty.Count) uncommitted change(s), and GitHub builds what is PUSHED, not what is here.`n" +
             "  Run  .\commit.ps1  first (or re-run buildwin.ps1 without -SkipCommit), then .\$scriptName.`n" +
             "  Changed: " + (($dirty | Select-Object -First 5) -join ', '))
    }
    $null = & git fetch --quiet origin $Branch 2>$null
    $ahead = (& git rev-list --count "origin/$Branch..HEAD" 2>$null)
    if ($ahead -and [int]$ahead -gt 0) {
      throw ("$ahead commit(s) of this PC are not pushed, so GitHub would build an older $Platform package.`n" +
             "  Run  .\commit.ps1  then this script again.")
    }
    $remoteToml = & git show ('origin/{0}:Cargo.toml' -f $Branch) 2>$null
    if ($LASTEXITCODE -eq 0 -and $remoteToml) {
      $m = [regex]::Match(($remoteToml -join "`n"), '(?m)^\s*version\s*=\s*"([^"]+)"')
      if ($m.Success -and $m.Groups[1].Value -ne $Version) {
        throw ("GitHub has version $($m.Groups[1].Value) on '$Branch', but this PC has $Version.`n" +
               "  Commit and push Cargo.toml first:  .\commit.ps1  (or git add -A; git commit -m 'NikPak $Version'; git push)")
      }
    }
    Write-Host "   origin/$Branch carries $Version, and nothing is left uncommitted" -ForegroundColor Green
  } finally {
    Pop-Location
  }
}

function Complete-ReleaseSource {
  param(
    [Parameter(Mandatory)][string]$Root,
    [switch]$SkipCommit,
    [string]$Message = ''
  )

  if ($SkipCommit) {
    Write-Host '   skipped (-SkipCommit): run .\commit.ps1 before buildmac.ps1 / buildunix.ps1' -ForegroundColor Yellow
    return 'skipped (-SkipCommit)'
  }
  # not $script: that is the scope qualifier, and a variable of that name is a trap
  $commitScript = Join-Path $Root 'commit.ps1'
  if (-not (Test-Path -LiteralPath $commitScript)) {
    Write-Host '   commit.ps1 is missing: the tree stays uncommitted' -ForegroundColor Yellow
    return 'FAILED (commit.ps1 missing)'
  }

  # Run it with the PowerShell that is running this build (pwsh 7 or Windows
  # PowerShell 5), so it works even when 'powershell.exe' is not on PATH.
  $psExe = (Get-Process -Id $PID).Path
  $argList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $commitScript)
  if ($Message) { $argList += @('-Message', $Message) }

  # Its output is kept in memory, not in a temp file: Tee-Object writes UTF-16 on
  # Windows PowerShell 5.1, so reading the log back would turn every ASCII phrase the
  # checks below look for into text with NUL bytes in it.
  $prev = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'   # git writes progress to stderr; judge by exit code
  try {
    $out = @(& $psExe @argList *>&1 | ForEach-Object { "$_" })
    $code = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $prev
  }
  foreach ($line in $out) { Write-Host $line }
  $text = $out -join "`n"

  if ($code -ne 0) {
    Write-Host ''
    if ($text -match 'GitHub has commits this computer does not have') {
      Write-Host '   the commit is on this PC but not on GitHub: run  git pull  then  .\commit.ps1' -ForegroundColor Red
    }
    Write-Host '   the release source was NOT committed and pushed - see the messages above' -ForegroundColor Red
    return 'FAILED (see the messages above)'
  }
  if ($text -match 'up to date with GitHub: nothing to push' -and $text -match 'working tree is clean') {
    return 'pushed (nothing new to commit)'
  }
  return 'committed and pushed'
}
