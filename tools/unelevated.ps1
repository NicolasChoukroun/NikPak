<#
  unelevated.ps1 - run one of these scripts as the normal user, from an
  administrator PowerShell.

  The capture scripts start nikpak.exe and photograph its window. NikPak
  refuses to keep running elevated: on Windows it starts itself again through
  Explorer (which runs as the logged-in user) so that files can be dropped on
  it, and the elevated copy exits. The window then belongs to another process,
  and the variables that pick the skin are lost.

  So the capture has to run in a normal-user process. Two ways are tried, in
  this order:

    1. a small .cmd wrapper started through Explorer - the same route NikPak
       itself uses, and the most reliable one on a locked-down machine;
    2. a one-shot scheduled task with the lowest rights (if the first fails).

  Both write their output to a log and an exit code to a marker file, so this
  shell can follow the run and report what happened.

  Usage, from a script in the project root:

      . (Join-Path $PSScriptRoot 'tools\unelevated.ps1')
      if (Test-Elevated -and -not $Inner) {
          exit (Invoke-Unelevated -Script $PSCommandPath -Root $PSScriptRoot -Arguments @{ Inner = $true })
      }

  The script must accept -Inner and -Root, and must do its real work when
  -Inner is given.
#>

# where the copy writes its output, so this shell can follow it
$script:UnelevatedLog = Join-Path ([System.IO.Path]::GetTempPath()) 'nikpak-unelevated.log'

function Test-Elevated {
    # the Windows principal types do not exist on macOS / Linux, where the
    # question does not arise: nothing to hand over
    try {
        $id = [Security.Principal.WindowsIdentity]::GetCurrent()
        if (-not $id) { return $false }
        $principal = New-Object Security.Principal.WindowsPrincipal($id)
        return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    } catch {
        if ($env:OS -eq 'Windows_NT') { throw }
        return $false
    }
}

# full paths: on a machine with a stripped PATH, "powershell" alone may not resolve
function Get-FullPath([string]$name, [string]$fallback) {
    $c = Get-Command $name -ErrorAction SilentlyContinue
    if ($c -and $c.Source) { return $c.Source }
    return $fallback
}

<#
  The command line that runs the copy as the normal user: powershell, the
  script, the arguments, all output to the log, and the exit code to a marker.
#>
function New-UnelevatedWrapper {
    param([string]$Copy, [string]$Root, [hashtable]$Arguments, [string]$Log, [string]$Marker)

    $ps = Get-FullPath 'powershell.exe' (Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe')
    $inner = '"{0}" -NoProfile -ExecutionPolicy Bypass -File "{1}" -Inner -Root "{2}"' -f $ps, $Copy, $Root
    foreach ($k in $Arguments.Keys) {
        $v = $Arguments[$k]
        if ($v -is [switch] -or $v -is [bool]) {
            if ($v) { $inner += (' -' + $k) }
        } else {
            $inner += (' -{0} "{1}"' -f $k, ([string]$v))
        }
    }
    $cmd = @(
        '@echo off',
        ('{0} > "{1}" 2>&1' -f $inner, $Log),
        ('echo %ERRORLEVEL% > "{0}"' -f $Marker)
    ) -join "`r`n"
    return $cmd
}

<#
  Wait for the marker file, echoing the log as it grows. Returns the exit code
  the copy left, or $null when it never finished.
#>
function Wait-Unelevated {
    param([string]$Log, [string]$Marker, [int]$TimeoutSeconds = 1800, [int]$StartAfterSeconds = 25)

    $seen = 0
    $start = Get-Date
    while (((Get-Date) - $start).TotalSeconds -lt $TimeoutSeconds) {
        if (Test-Path $Log) {
            $lines = @(Get-Content $Log -ErrorAction SilentlyContinue)
            if ($lines.Count -gt $seen) {
                $lines[$seen..($lines.Count - 1)] | ForEach-Object { Write-Host $_ }
                $seen = $lines.Count
            }
        }
        if (Test-Path $Marker) {
            $text = (Get-Content $Marker -ErrorAction SilentlyContinue | Select-Object -First 1)
            if ($text -match '^\s*-?\d+\s*$') { return [int]$text.Trim() }
        }
        if (((Get-Date) - $start).TotalSeconds -gt $StartAfterSeconds -and $seen -eq 0) {
            return $null   # nothing has started: let the caller try the other route
        }
        Start-Sleep -Milliseconds 700
    }
    return $null
}

<#
  Run the script as the normal user. Returns its exit code, or 1 when no route
  worked (the caller says so).
#>
function Invoke-Unelevated {
    param(
        [Parameter(Mandatory)][string]$Script,
        [string]$Root = (Split-Path -Parent $Script),
        [hashtable]$Arguments = @{},
        [string]$Log,
        [int]$TimeoutSeconds = 1800
    )

    if (-not $Log) { $Log = $script:UnelevatedLog }
    $temp = [System.IO.Path]::GetTempPath()
    $copy = Join-Path $temp ('unelevated-' + [System.IO.Path]::GetFileName($Script))
    $wrapper = Join-Path $temp 'nikpak-unelevated.cmd'
    $marker = Join-Path $temp 'nikpak-unelevated.exit'

    # the copy runs from %TEMP%, so the folder it must work in is passed on
    Copy-Item -LiteralPath $Script -Destination $copy -Force
    Remove-Item $marker -Force -ErrorAction SilentlyContinue
    Remove-Item $Log -Force -ErrorAction SilentlyContinue
    New-UnelevatedWrapper -Copy $copy -Root $Root -Arguments $Arguments -Log $Log -Marker $marker |
        Set-Content -Path $wrapper -Encoding ASCII

    Write-Host '  this shell is elevated: the capture is handed to a normal-user process' -ForegroundColor Yellow

    # --- route 1: through Explorer, like NikPak restarts itself --------------
    $explorer = Get-FullPath 'explorer.exe' (Join-Path $env:SystemRoot 'explorer.exe')
    if (Test-Path $explorer) {
        Write-Host '  starting it through Explorer...' -ForegroundColor DarkGray
        Start-Process -FilePath $explorer -ArgumentList $wrapper
        $code = Wait-Unelevated -Log $Log -Marker $marker -TimeoutSeconds $TimeoutSeconds
        if ($null -ne $code) {
            Remove-Item $copy, $wrapper, $marker -Force -ErrorAction SilentlyContinue
            return $code
        }
        Write-Host '  Explorer did not start it; trying a scheduled task...' -ForegroundColor DarkGray
        Remove-Item $marker -Force -ErrorAction SilentlyContinue
    }

    # --- route 2: a one-shot scheduled task with the lowest rights ----------
    $schtasks = Join-Path $env:SystemRoot 'System32\schtasks.exe'
    if (-not (Test-Path $schtasks)) { $schtasks = Get-FullPath 'schtasks.exe' 'schtasks.exe' }
    $taskName = 'NikPakSkinShots'
    $start = (Get-Date).AddMinutes(1).ToString('HH:mm')
    try {
        & $schtasks /Create /TN $taskName /TR ('"{0}"' -f $wrapper) /SC ONCE /ST $start /IT /F *> $null
        & $schtasks /Run /TN $taskName *> $null
        $code = Wait-Unelevated -Log $Log -Marker $marker -TimeoutSeconds $TimeoutSeconds
        & $schtasks /Delete /TN $taskName /F *> $null
        if ($null -ne $code) {
            Remove-Item $copy, $wrapper, $marker -Force -ErrorAction SilentlyContinue
            return $code
        }
    } catch {
        Write-Host ("  the scheduled task route failed: {0}" -f $_.Exception.Message) -ForegroundColor Red
    }

    Write-Host ''
    Write-Host '  The capture could not be started as the normal user on this machine.' -ForegroundColor Red
    Write-Host '  Run it by hand instead, in a PowerShell opened WITHOUT "Run as administrator":' -ForegroundColor Yellow
    Write-Host ('      cd "{0}"' -f $Root)
    Write-Host ('      {0} -Inner -Root "{1}"' -f $Script, $Root)
    Remove-Item $copy, $wrapper, $marker -Force -ErrorAction SilentlyContinue
    return 1
}
