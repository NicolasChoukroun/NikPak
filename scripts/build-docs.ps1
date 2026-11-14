<#
.SYNOPSIS
  Build the eleven NikPak manuals in documents\manuals\ (one per language).

.DESCRIPTION
  Each manual is one book holding the user manual and the technical guide (the text lives in documents\text\<code>.py).

  **The zips carry them all.** release-packaging.ps1 copies every *.pdf from the
  -Documentation folder, so buildall.ps1 points it at documents\manuals rather than
  at the old English-only documents\ root (which still holds the PDF the in-app
  manual is generated from).

  Called from buildall.ps1 before the packing step. Skipped when the PDFs are newer
  than the text, so a rebuild that changed no documentation does not repaginate ten
  books. Pass -Force to rebuild anyway.

.EXAMPLE
  .\scripts\build-docs.ps1
  .\scripts\build-docs.ps1 -Force -Languages fr,ar
#>
param(
  [switch]$Force,
  [string[]]$Languages = @()
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$docs = Join-Path $root 'documents'
$script = Join-Path $docs 'build_docs.py'
$outDir = Join-Path $docs 'manuals'

if (-not (Test-Path -LiteralPath $script)) {
  Write-Host "   no documents\build_docs.py - nothing to build" -ForegroundColor DarkYellow
  return
}

$all = @('en', 'fr', 'de', 'es', 'pt', 'ru', 'ja', 'zh', 'hi', 'ar', 'th')
if (-not $Languages.Count) { $Languages = $all }

if (-not $Force) {
  # The newest of everything that goes into a book: the script, the shared engine
  # and the per-language texts. A book older than any of them is stale.
  $inputs = @(Get-Item -LiteralPath $script)
  $engine = Join-Path (Split-Path -Parent $root) 'nkdocs\nkdocs.py'
  if (Test-Path -LiteralPath $engine) { $inputs += Get-Item -LiteralPath $engine }
  $textDir = Join-Path $docs 'text'
  if (Test-Path -LiteralPath $textDir) { $inputs += Get-ChildItem -LiteralPath $textDir -Filter '*.py' }
  $newest = ($inputs | Measure-Object -Property LastWriteTime -Maximum).Maximum

  $stale = @()
  foreach ($l in $Languages) {
    $pdf = Join-Path $outDir ("NikPak-Manual-$l.pdf")
    if (-not (Test-Path -LiteralPath $pdf)) { $stale += $l; continue }
    if ((Get-Item -LiteralPath $pdf).LastWriteTime -lt $newest) { $stale += $l }
  }
  if (-not $stale.Count) {
    Write-Host "   eleven manuals already in documents\manuals (up to date)" -ForegroundColor DarkGray
    return
  }
  $Languages = $stale
}

$py = $null
foreach ($name in @('python', 'python3', 'py')) {
  $cmd = Get-Command $name -ErrorAction SilentlyContinue
  if ($cmd) { $py = $cmd.Source; break }
}
if (-not $py) {
  Write-Warning "python was not found: the manuals will be the ones already in documents\manuals. Install Python 3 with 'pip install python-docx pillow', then run documents\build_docs.py."
  return
}

$pyArgs = @()
if ($Languages.Count -lt $all.Count) { $pyArgs += @('-l') + $Languages }
Write-Host "   $py documents\build_docs.py $($pyArgs -join ' ')" -ForegroundColor DarkGray
& $py $script @pyArgs
if ($LASTEXITCODE -ne 0) {
  Write-Warning "the manuals could not all be built (exit $LASTEXITCODE). The zips will carry whatever PDFs exist."
  return
}

$got = @(Get-ChildItem -LiteralPath $outDir -Filter 'NikPak-Manual-*.pdf' -ErrorAction SilentlyContinue)
$missing = @($all | Where-Object { $l = $_; -not ($got.Name -contains "NikPak-Manual-$l.pdf") })
Write-Host ("   {0} manual(s) in documents\manuals" -f $got.Count)
if ($missing.Count) { Write-Warning ("missing: {0}" -f ($missing -join ', ')) }
