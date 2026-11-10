#Requires -Version 5.1
<#
.SYNOPSIS
  NikPak - commit everything, then push it to GitHub.

.DESCRIPTION
  One command for the end of a work session, so GitHub has exactly what is on this PC:

    1. commits what is not committed yet, with a message written from the files that
       changed when none is given (NikPak v1.13.1 - update buildwin.ps1, buildall.ps1);
    2. pushes to origin - setting the upstream the first time, and saying "up to date"
       instead of pretending to push when GitHub already has everything;
    3. **a clean tree is not "nothing to do"**: commits already made on this PC (by hand,
       or by Claude) are still pushed, which is what makes the macOS/Linux GitHub builds
       see the current code. Their version comes from the pushed Cargo.toml, so this is
       the step that gives buildmac.ps1 / buildunix.ps1 the right number;
    4. stops with exit code 1 when anything failed, so a chained build script stops too.

  Build output never reaches the commit: target\, dist\, binaries\, keys\ and the Python
  caches are unstaged again after they have been picked up (see -Force). keys\ holds the
  private licence key and must never be pushed. website\dist\ IS committed on purpose -
  it is the built site - so it is only reported, never dropped.

.PARAMETER Message
  The commit message. Default: "NikPak v<version> - update <folders that changed>".

.PARAMETER NoPush
  Commit locally only.

.PARAMETER Force
  Keep what the script would otherwise unstage (build output and caches), and skip the
  checkpoint of the version files below.

.EXAMPLE
  .\commit.ps1
  .\commit.ps1 -Message "Split volumes, multi-core compression, extract box"
  .\commit.ps1 -NoPush

.NOTES
  Windows PowerShell 5.1: no `??`, no ternary, and git is always run through a resolved
  full path (see Invoke-Git).

  A release usually goes:  .\buildwin.ps1  ->  .\commit.ps1  ->  .\buildmac.ps1  ->
  .\buildunix.ps1  ->  .\buildweb.ps1. The commit sits in the middle because the two
  GitHub builds read the version from what was pushed.
#>
[CmdletBinding()]
param(
  [string]$Message = '',
  [switch]$NoPush,
  [switch]$Force
)

$ErrorActionPreference = 'Stop'
$Root = $PSScriptRoot
Set-Location -LiteralPath $Root
$RemoteUrl = 'https://github.com/NicolasChoukroun/NikPak.git'

function Step([string]$Text) { Write-Host ''; Write-Host "== $Text" -ForegroundColor Cyan }
function Fail([string]$Text) { Write-Host ''; Write-Host $Text -ForegroundColor Red; exit 1 }

# Console output in UTF-8, or a path with é / 日本語 comes back as '?' and a `git reset`
# built from it matches nothing. Guarded: a host without a console (a build runner) has
# no encoding to set, and that must not stop the commit.
try {
  $utf8 = New-Object System.Text.UTF8Encoding($false)
  [Console]::OutputEncoding = $utf8
  $OutputEncoding = $utf8
} catch { }

# ---------------------------------------------------------------------------
Step 'Finding git'

# A git installed since this Explorer/terminal was opened is not on this process's PATH,
# and "git is not installed" would be a lie. Same probe as the other Nikkoid scripts.
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
  $dirs = @(
    [Environment]::GetEnvironmentVariable('Path', 'Machine'),
    [Environment]::GetEnvironmentVariable('Path', 'User')
  ) -join ';'
  $extra = @()
  if ($env:ProgramFiles) { $extra += Join-Path $env:ProgramFiles 'Git\cmd' }
  if ($env:LOCALAPPDATA) { $extra += Join-Path $env:LOCALAPPDATA 'Programs\Git\cmd' }
  foreach ($dir in (($dirs + ';' + ($extra -join ';')) -split ';' | Where-Object { $_ } | Select-Object -Unique)) {
    try { $candidate = Join-Path $dir 'git.exe' } catch { continue }
    if (Test-Path -LiteralPath $candidate) { $env:PATH = "$dir;$env:PATH"; break }
  }
}
if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
  Fail 'Git was not found. Install it from https://git-scm.com/download/win and open a new terminal.'
}
# The full path, resolved once. Everything below runs this, never the bare word `git`.
$GitExe = (Get-Command git).Source

# **Not named `Git`.** PowerShell looks for a function before an external program and
# ignores case, so a helper called `Git` that runs `& git ...` finds itself and recurses
# until the call stack gives out ("call depth overflow").
function Invoke-Git([string[]]$GitArgs) {
  # git writes progress to stderr; under 'Stop' that becomes a terminating error, so the
  # call is judged by its exit code instead.
  $prev = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  try { $out = & $GitExe @GitArgs 2>&1 } finally { $ErrorActionPreference = $prev }
  if ($LASTEXITCODE -ne 0) {
    throw ("git {0} failed:`n{1}" -f ($GitArgs -join ' '), ($out -join "`n"))
  }
  # Emit nothing when git said nothing: `return $null` puts one $null into the pipeline,
  # and `@(...)` around that is a one-element array - a clean tree would look like one
  # changed path and the script would commit nothing.
  if ($null -ne $out) { $out }
}

# ---------------------------------------------------------------------------
Step 'Checking the repository'

if (-not (Test-Path -LiteralPath (Join-Path $Root '.git'))) { Fail 'This folder is not a git repository. Run: git init -b main' }

# An operation left half-way (a rebase from another session, a merge) must be finished
# before anything is committed: `git commit` would make a commit of its middle state.
foreach ($marker in @('MERGE_HEAD', 'rebase-merge', 'rebase-apply', 'CHERRY_PICK_HEAD', 'REVERT_HEAD')) {
  if (Test-Path -LiteralPath (Join-Path $Root ".git\$marker")) {
    Fail "A merge / rebase / cherry-pick is in progress (.git\$marker). Finish it, then run this again."
  }
}

# A stale index.lock blocks every write, and the message git prints for it ("Another git
# process seems to be running") is wrong once the process is gone - a crashed git, or a
# tool that could not unlink it, leaves the file behind. It is removed only when no git
# process is actually running, so a real commit is never disturbed.
$lock = Join-Path $Root '.git\index.lock'
if (Test-Path -LiteralPath $lock) {
  if (Get-Process git -ErrorAction SilentlyContinue) {
    Fail '.git\index.lock exists and a git process is running: close it and run this again.'
  }
  Remove-Item -LiteralPath $lock -Force
  Write-Host '   removed a stale .git\index.lock' -ForegroundColor Yellow
}

$remoteUrl = ''
try { $remoteUrl = [string](Invoke-Git @('config', '--get', 'remote.origin.url')) } catch { $remoteUrl = '' }
if (-not $remoteUrl) {
  Write-Host "   no origin remote: adding $RemoteUrl" -ForegroundColor Yellow
  $null = Invoke-Git @('remote', 'add', 'origin', $RemoteUrl)
  $remoteUrl = $RemoteUrl
}
if ($remoteUrl -match '<[A-Za-z]+>') {
  Fail ("origin is still the placeholder {0} - run: git remote set-url origin {1}" -f $remoteUrl, $RemoteUrl)
}

# An identity of this repository's own, if neither it nor the global config has one:
# `git commit` would stop with "Author identity unknown" after everything was staged.
$identName = ''
try { $identName = [string](& $GitExe config user.name 2>$null) } catch { }
if (-not $identName) {
  $null = & $GitExe config user.name 'NicolasChoukroun'
  Write-Host '   set a local user.name for this repository' -ForegroundColor Yellow
}
$identMail = ''
try { $identMail = [string](& $GitExe config user.email 2>$null) } catch { }
if (-not $identMail) {
  $null = & $GitExe config user.email '40539492+NicolasChoukroun@users.noreply.github.com'
  Write-Host '   set a local user.email for this repository' -ForegroundColor Yellow
}

$branch = ([string](Invoke-Git @('rev-parse', '--abbrev-ref', 'HEAD'))).Trim()
if ($branch -eq 'HEAD') { Fail 'Detached HEAD: check out a branch (git checkout main) before committing.' }
Write-Host "   branch $branch  ->  $remoteUrl"

# ---------------------------------------------------------------------------
# The number this commit carries. Cargo.toml is what cargo compiles into `nikpak -V`
# and what GitHub checks out to build macOS / Linux, so a tree where it disagrees with
# the web site is pushed only with a warning, never silently.
$version = ''
$versionHelper = Join-Path $Root 'scripts\version.ps1'
if (Test-Path -LiteralPath $versionHelper) {
  try {
    . $versionHelper
    $version = Get-NikpakVersion $Root
  } catch { $version = '' }
}
if (-not $version) {
  $toml = Join-Path $Root 'Cargo.toml'
  if (Test-Path -LiteralPath $toml) {
    $m = [regex]::Match([System.IO.File]::ReadAllText($toml), '(?m)^\s*version\s*=\s*"([^"]+)"')
    if ($m.Success) { $version = $m.Groups[1].Value }
  }
}

# ---------------------------------------------------------------------------
Step 'Looking at the changes'

# A clean tree is not the end: commits made earlier on this PC may not be on GitHub yet,
# and those are exactly what the macOS / Linux build needs to see.
$status = @(Invoke-Git @('status', '--porcelain'))
$hasChanges = ($status.Count -gt 0)
if ($hasChanges) {
  Write-Host ("   {0} path(s) changed" -f $status.Count)
} else {
  Write-Host '   working tree is clean' -ForegroundColor DarkGray
}

$committed = $false
$head = ([string](Invoke-Git @('rev-parse', '--short', 'HEAD'))).Trim()
if ($hasChanges) {
  $null = Invoke-Git @('add', '-A')

  $staged = @(Invoke-Git @('diff', '--cached', '--name-only'))

  # Build output, caches and secrets are picked up by `git add -A` when an ignore rule is
  # missing; committing them is how a repository grows without bound - and keys\ holds the
  # private licence key, which must never leave this PC. They are taken out of the commit
  # again (the files stay on disk, and nothing is deleted).
  # website\dist\ is NOT in this list: the built site is committed on purpose.
  $junkPatterns = @(
    '^target/', '^dist/', '^binaries/', '^keys/',
    '(^|/)__pycache__/', '\.pyc$',
    '^\.ftp-state\.json$', '^website/shots-raw/.*\.tmp$'
  )
  $junkStaged = @($staged | Where-Object { $p = $_; $hit = $false; foreach ($re in $junkPatterns) { if ($p -match $re) { $hit = $true; break } }; $hit })
  if ($junkStaged.Count -and -not $Force) {
    $reset = @('reset', '-q', 'HEAD', '--') + $junkStaged
    $null = Invoke-Git $reset
    Write-Host ("   left out {0} build / cache / secret file(s):" -f $junkStaged.Count) -ForegroundColor Yellow
    $junkStaged | Select-Object -First 6 | ForEach-Object { Write-Host "      $_" -ForegroundColor DarkGray }
    if ($junkStaged.Count -gt 6) { Write-Host ("      ... and {0} more" -f ($junkStaged.Count - 6)) -ForegroundColor DarkGray }
    Write-Host '   (they stay on disk; add them to .gitignore, or run with -Force to commit them)' -ForegroundColor DarkGray
    $staged = @(Invoke-Git @('diff', '--cached', '--name-only'))
  }

  # Not dropped, only shown: the owner's own call. Big binary material in a commit is
  # normal here (the guide's pictures and PDF, the site's screenshots), but worth a line
  # so it is never a surprise afterwards - website\dist\downloads\ holds copies of the
  # installers, several MB each.
  $notable = @($staged | Where-Object { $_ -match '^(binaries/|documents/|screenshots/|_to_delete/|_backup-|website/dist/downloads/)' })
  if ($notable.Count) {
    $bytes = 0
    foreach ($p in $notable) {
      # git prints forward slashes; both Windows and PowerShell accept them in a path
      try { $bytes += (Get-Item -LiteralPath (Join-Path $Root $p)).Length } catch { }
    }
    Write-Host ("   including {0} large file(s) - documents\, screenshots\, website\dist\downloads\  ({1:n1} MB)" -f $notable.Count, ($bytes / 1MB)) -ForegroundColor Yellow
  }

  if (-not $staged.Count) {
    Write-Host '   nothing to commit after leaving the build output out' -ForegroundColor DarkGray
  } else {
    if (-not $Message) {
      $groups = @($staged | ForEach-Object { ($_ -split '/')[0] } | Select-Object -Unique | Sort-Object)
      $tail = ($groups | Select-Object -First 6) -join ', '
      if ($groups.Count -gt 6) { $tail = $tail + ', ...' }
      if ($version) { $Message = "NikPak v$version - update $tail" }
      else { $Message = "NikPak - update $tail" }
    }
    Write-Host ("   {0} file(s) staged" -f $staged.Count)
    Write-Host "   message: $Message"
    $null = Invoke-Git @('commit', '-m', $Message)
    $head = ([string](Invoke-Git @('rev-parse', '--short', 'HEAD'))).Trim()
    $committed = $true
    Write-Host "   commit $head" -ForegroundColor Green

    # The version files must agree with each other before they are pushed: Cargo.toml is
    # what the macOS / Linux builds check out, site.json and version.json drive the
    # download links and the update prompt. Reported, not stopped - a work-in-progress
    # commit is allowed; buildweb.ps1 refuses to publish in this state.
    if (-not $Force -and $version) {
      $others = @(
        @{ File = 'website\site.json';    Pattern = '(?m)^\s*"version"\s*:\s*"([^"]+)"' },
        @{ File = 'website\version.json'; Pattern = '(?m)^\s*"latest"\s*:\s*"([^"]+)"' }
      )
      foreach ($o in $others) {
        $p = Join-Path $Root $o.File
        if (-not (Test-Path -LiteralPath $p)) { continue }
        $m = [regex]::Match([System.IO.File]::ReadAllText($p), $o.Pattern)
        if ($m.Success -and $m.Groups[1].Value -ne $version) {
          Write-Host ("   NOTE  {0} says {1} but Cargo.toml says {2} - run buildwin.ps1 to sync them" -f $o.File, $m.Groups[1].Value, $version) -ForegroundColor Yellow
        }
      }
    }
  }
}

if ($NoPush) {
  Step 'Done (not pushed: -NoPush)'
  if ($committed) { Write-Host "   commit $head on $branch" }
  Write-Host "   push later with: git push origin $branch"
  exit 0
}

# ---------------------------------------------------------------------------
Step 'Pushing'

# git push, retried when GitHub fails on its side (500, a dropped connection), with the
# reason spelled out for the failures that have one - "git push failed" alone sends you
# looking for a login problem that is not there.
function Invoke-GitPush([string[]]$PushArgs = @()) {
  $prev = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'   # git writes progress to stderr: not an error
  try {
    for ($try = 1; $try -le 3; $try++) {
      $out = @(& $GitExe push @PushArgs 2>&1 | ForEach-Object { "$_" })
      $code = $LASTEXITCODE
      foreach ($line in $out) { if ($line.Trim()) { Write-Host ("   " + $line) -ForegroundColor DarkGray } }
      if ($code -eq 0) { return }
      $text = $out -join "`n"
      if ($text -match 'Authentication failed|could not read Username|Invalid username or password|Permission to .* denied|returned error: 403') {
        throw 'git push failed: GitHub refused the login (run: gh auth login, or check Windows Credential Manager)'
      }
      if ($text -match 'GH013|push protection|secret scanning') {
        throw 'git push failed: GitHub push protection found a secret in a commit (see the message above)'
      }
      if ($text -match 'non-fast-forward|fetch first|\[rejected\]') {
        throw 'git push failed: GitHub has commits this computer does not have - run git pull first'
      }
      if ($try -lt 3) {
        $wait = 10 * $try
        Write-Host ("   GitHub failed on its side, trying again in {0} s ({1}/3)..." -f $wait, $try) -ForegroundColor Yellow
        Start-Sleep -Seconds $wait
      }
    }
    throw "git push failed 3 times on GitHub's side (server error or network) - see https://www.githubstatus.com and run this again later"
  } finally { $ErrorActionPreference = $prev }
}

try {
  $upstream = ''
  try { $upstream = ([string](Invoke-Git @('rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{u}'))).Trim() } catch { $upstream = '' }
  $hasUpstream = ($upstream -and $upstream -ne '@{u}')

  if (-not $hasUpstream) {
    # First push of this branch (also the first push of a repository that exists on GitHub
    # with no commits yet): -u records the upstream so the next run needs nothing typed.
    Write-Host "   first push of ${branch}: setting its upstream" -ForegroundColor Yellow
    Invoke-GitPush @('-u', 'origin', $branch)
    Write-Host "   pushed $branch to origin, upstream set" -ForegroundColor Green
  } else {
    # Learn what GitHub has, so a push that would be refused is not attempted.
    $null = Invoke-Git @('fetch', '--quiet', 'origin')
    $ahead = [int]([string](Invoke-Git @('rev-list', '--count', '@{u}..HEAD'))).Trim()
    $behind = [int]([string](Invoke-Git @('rev-list', '--count', 'HEAD..@{u}'))).Trim()

    if ($behind -gt 0) {
      Write-Host ("   GitHub has {0} commit(s) this computer does not have - run: git pull" -f $behind) -ForegroundColor Yellow
      if ($ahead -gt 0) { Write-Host ("   ({0} local commit(s) still waiting to be pushed)" -f $ahead) -ForegroundColor Yellow }
      Write-Host '   nothing was lost: the commit above is safe on this PC' -ForegroundColor DarkGray
      exit 1
    }
    if ($ahead -eq 0) {
      Write-Host '   up to date with GitHub: nothing to push' -ForegroundColor Green
    } else {
      Invoke-GitPush
      Write-Host ("   pushed {0} commit(s) to origin" -f $ahead) -ForegroundColor Green
    }
  }
} catch {
  Write-Host ''
  Write-Host ("   ERROR: {0}" -f $_.Exception.Message) -ForegroundColor Red
  exit 1
}

Step 'Done'
Write-Host ("   NikPak {0}, commit {1}, branch {2}" -f $version, $head, $branch)
Write-Host '   GitHub builds macOS / Linux from what is now on origin:'
Write-Host '   .\buildmac.ps1   /   .\buildunix.ps1   then   .\buildweb.ps1 to publish'
