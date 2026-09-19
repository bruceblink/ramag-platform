# Checks Rust source files under crates and matches scripts/check-source-size.sh.
[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$RepoRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$MaxLines = 600
if ([string]::IsNullOrWhiteSpace($RepoRoot)) {
    $RepoRoot = Join-Path $PSScriptRoot "..\.."
}
$RepoRoot = [System.IO.Path]::GetFullPath($RepoRoot).TrimEnd('\', '/')
$CratesRoot = Join-Path $RepoRoot "crates"
$BaselineFile = Join-Path $RepoRoot "scripts\source-size-baseline.txt"

if (-not (Test-Path -LiteralPath $CratesRoot -PathType Container)) {
    [Console]::Error.WriteLine("Rust source root is missing: $CratesRoot")
    exit 1
}
if (-not (Test-Path -LiteralPath $BaselineFile -PathType Leaf)) {
    [Console]::Error.WriteLine("Source-size baseline is missing: $BaselineFile")
    exit 1
}

$BaselinePaths = [System.Collections.Generic.HashSet[string]]::new(
    [System.StringComparer]::OrdinalIgnoreCase
)
foreach ($BaselineLine in @(Get-Content -LiteralPath $BaselineFile)) {
    $BaselinePath = $BaselineLine.Trim()
    if (-not [string]::IsNullOrWhiteSpace($BaselinePath) -and -not $BaselinePath.StartsWith("#")) {
        [void]$BaselinePaths.Add($BaselinePath.Replace('/', '\'))
    }
}

$Failed = $false
$SourceFiles = @(Get-ChildItem -LiteralPath $CratesRoot -Recurse -File -Filter "*.rs" | Sort-Object -Property FullName)
foreach ($SourceFile in $SourceFiles) {
    $RelativePath = $SourceFile.FullName.Substring($RepoRoot.Length).TrimStart('\', '/')
    $RelativePath = $RelativePath.Replace('/', '\')
    if ($BaselinePaths.Contains($RelativePath)) {
        continue
    }

    $LineCount = 0
    $Reader = [System.IO.StreamReader]::new($SourceFile.FullName)
    try {
        while ($null -ne $Reader.ReadLine()) {
            $LineCount++
        }
    }
    finally {
        $Reader.Dispose()
    }

    if ($LineCount -gt $MaxLines) {
        $DiagnosticPath = $RelativePath.Replace('\', '/')
        $Diagnostic = "{0}: {1} lines (max {2})" -f @($DiagnosticPath, $LineCount, $MaxLines)
        [Console]::Error.WriteLine($Diagnostic)
        $Failed = $true
    }
}

if ($Failed) {
    exit 1
}
exit 0
