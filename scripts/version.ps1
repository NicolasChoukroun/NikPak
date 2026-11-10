<#
  version.ps1 - the single place that reads, bumps and writes the NikPak version.

  Dot-source it from a build script:

      . (Join-Path $PSScriptRoot 'scripts\version.ps1')

  The version lives in Cargo.toml (it is what cargo compiles into nikpak -V and the
  About box). Three other files must always carry the same number, or the website
  offers a download that does not exist and the in-app update check lies:

      Cargo.toml            version          the source of truth
      Cargo.lock            the nikpak block (cargo rewrites it, we keep it in step early)
      website/site.json     "version"        download URLs ({v}) and the "Version {v}" line
      website/version.json  "latest"         the update manifest the app asks for

  CHANGELOG.md and the CHANGELOG list in website/build.py are prose: they are checked
  and reported, never rewritten, because only a person can summarise a release.
#>

function Get-NikpakVersion {
    param([Parameter(Mandatory)][string]$Root)
    $toml = Join-Path $Root 'Cargo.toml'
    if (-not (Test-Path -LiteralPath $toml)) { throw "Cargo.toml not found in $Root" }
    $m = [regex]::Match([System.IO.File]::ReadAllText($toml), '(?m)^\s*version\s*=\s*"([^"]+)"')
    if (-not $m.Success) { throw 'Cannot read the version from Cargo.toml.' }
    return $m.Groups[1].Value
}

function Get-NextNikpakVersion {
    param([Parameter(Mandatory)][string]$Version)
    $p = $Version.Split('.')
    if ($p.Count -ne 3) { throw "Version '$Version' is not x.y.z" }
    return ('{0}.{1}.{2}' -f $p[0], $p[1], ([int]$p[2] + 1))
}

# Write UTF-8 WITHOUT a BOM: the JSON files are read by Python (json.load), which
# rejects a leading BOM, and Windows PowerShell's -Encoding UTF8 adds one.
function Write-TextNoBom {
    param([string]$Path, [string]$Text)
    [System.IO.File]::WriteAllText($Path, $Text, [System.Text.UTF8Encoding]::new($false))
}

# Replace the first match of a two-group pattern. Named groups are used in the
# replacement so a value such as "1.13.2" can never be read as "$11" + "3.2".
function Replace-FirstVersion {
    param([string]$Text, [string]$Pattern, [string]$Value)
    $rx = [regex]$Pattern
    return $rx.Replace($Text, ('${pre}' + $Value + '${post}'), 1)
}

function Set-NikpakVersion {
    param([Parameter(Mandatory)][string]$Root, [Parameter(Mandatory)][string]$Version)
    if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Version '$Version' is not x.y.z (for example 1.14.0)" }

    $targets = @(
        @{ File = 'Cargo.toml';           Pattern = '(?m)^(?<pre>\s*version\s*=\s*")[^"]*(?<post>")' },
        @{ File = 'Cargo.lock';           Pattern = '(?m)^(?<pre>\[\[package\]\]\r?\nname = "nikpak"\r?\nversion = ")[^"]*(?<post>")' },
        @{ File = 'website\site.json';    Pattern = '(?m)^(?<pre>\s*"version"\s*:\s*")[^"]*(?<post>")' },
        @{ File = 'website\version.json'; Pattern = '(?m)^(?<pre>\s*"latest"\s*:\s*")[^"]*(?<post>")' }
    )
    foreach ($t in $targets) {
        $path = Join-Path $Root $t.File
        if (-not (Test-Path -LiteralPath $path)) { continue }
        $old = [System.IO.File]::ReadAllText($path)
        $new = Replace-FirstVersion $old $t.Pattern $Version
        if ($new -ne $old) {
            Write-TextNoBom $path $new
            Write-Host ("   {0} -> {1}" -f $t.File, $Version) -ForegroundColor DarkGray
        }
    }
}

# Refuse to build a mixture: the four machine-readable files must agree.
# The prose changelogs only produce a warning.
function Assert-NikpakVersionSync {
    param([Parameter(Mandatory)][string]$Root, [Parameter(Mandatory)][string]$Version)
    $checks = @(
        @{ File = 'Cargo.toml';           Pattern = '(?m)^\s*version\s*=\s*"([^"]+)"' },
        @{ File = 'Cargo.lock';           Pattern = '(?m)^\[\[package\]\]\r?\nname = "nikpak"\r?\nversion = "([^"]+)"' },
        @{ File = 'website\site.json';    Pattern = '(?m)^\s*"version"\s*:\s*"([^"]+)"' },
        @{ File = 'website\version.json'; Pattern = '(?m)^\s*"latest"\s*:\s*"([^"]+)"' }
    )
    $bad = @()
    foreach ($c in $checks) {
        $path = Join-Path $Root $c.File
        if (-not (Test-Path -LiteralPath $path)) { continue }
        $m = [regex]::Match([System.IO.File]::ReadAllText($path), $c.Pattern)
        if (-not $m.Success) { $bad += ("{0}: version not found" -f $c.File) }
        elseif ($m.Groups[1].Value -ne $Version) { $bad += ("{0}: {1} (expected {2})" -f $c.File, $m.Groups[1].Value, $Version) }
    }
    if ($bad.Count) {
        foreach ($b in $bad) { Write-Host ("   MISMATCH  " + $b) -ForegroundColor Red }
        throw 'The version is not the same in every source file.'
    }

    # prose: report, do not rewrite
    $cl = Join-Path $Root 'CHANGELOG.md'
    if (Test-Path -LiteralPath $cl) {
        $m = [regex]::Match([System.IO.File]::ReadAllText($cl), '(?m)^##\s*(\d+\.\d+\.\d+)')
        if ($m.Success -and $m.Groups[1].Value -ne $Version) {
            Write-Host ("   NOTE  CHANGELOG.md starts at {0}: add the {1} entry." -f $m.Groups[1].Value, $Version) -ForegroundColor Yellow
        }
    }
    $bp = Join-Path $Root 'website\build.py'
    if (Test-Path -LiteralPath $bp) {
        $m = [regex]::Match([System.IO.File]::ReadAllText($bp), '(?s)CHANGELOG\s*=\s*\[\s*\("([^"]+)"')
        if ($m.Success -and $m.Groups[1].Value -ne $Version) {
            Write-Host ("   NOTE  website\build.py changelog starts at {0}: add the {1} entry." -f $m.Groups[1].Value, $Version) -ForegroundColor Yellow
        }
    }
}

# Delete every file whose name carries a version other than the current one.
# Files without a version (nikpak.exe, unNik.exe, SHA256SUMS.txt) are left alone:
# they are overwritten by the build.
function Remove-OldNikpakFiles {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string[]]$Dirs
    )
    foreach ($d in $Dirs) {
        $path = Join-Path $Root $d
        if (-not (Test-Path -LiteralPath $path)) { continue }
        Get-ChildItem -LiteralPath $path -File | ForEach-Object {
            $m = [regex]::Match($_.Name, '(?<!\d)(\d+\.\d+\.\d+)(?!\d)')
            if ($m.Success -and $m.Groups[1].Value -ne $Version) {
                Write-Host ("   removing old version: {0}" -f $_.Name) -ForegroundColor DarkYellow
                Remove-Item -LiteralPath $_.FullName -Force
            }
        }
    }
}
