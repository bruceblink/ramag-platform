param(
    [string]$SourceRoot = 'F:\project\system-pulse',
    [string]$WorkspaceRoot = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = 'Stop'
$sourcePath = [IO.Path]::GetFullPath($SourceRoot)
$workspacePath = [IO.Path]::GetFullPath($WorkspaceRoot)
$toolPath = Join-Path $workspacePath 'crates\ramag-tool-system'
$modelPath = Join-Path $workspacePath 'crates\ramag-system-model'
$encoding = [Text.UTF8Encoding]::new($false)

# Import application code without its standalone executable, tray or workspace.
# Existing monitoring code is retained in a local backup until replacement passes.
$backupPath = Join-Path $workspacePath 'target\system-pulse-import\legacy-tool'
if (-not (Test-Path -LiteralPath $backupPath)) {
    New-Item -ItemType Directory -Path $backupPath -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $toolPath 'src') -Destination $backupPath -Recurse
}

function Copy-SourceTree([string]$From, [string]$To, [string[]]$Exclude) {
    foreach ($file in Get-ChildItem -LiteralPath $From -File -Recurse) {
        $relative = [IO.Path]::GetRelativePath($From, $file.FullName)
        if ($relative -in $Exclude) { continue }
        $destination = Join-Path $To $relative
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        if ($file.Extension -in @('.rs', '.toml', '.md', '.txt')) {
            $contents = [IO.File]::ReadAllText($file.FullName).Replace("`r`n", "`n")
            $contents = $contents.Replace('system_pulse_collectors', 'ramag_infra_system')
            [IO.File]::WriteAllText($destination, $contents, $encoding)
        } else {
            Copy-Item -LiteralPath $file.FullName -Destination $destination
        }
    }
}

Copy-SourceTree (Join-Path $sourcePath 'src') (Join-Path $toolPath 'src') @(
    'main.rs', 'lib.rs', 'application.rs', 'tray.rs', 'tray\icon.rs'
)
Copy-SourceTree (Join-Path $sourcePath 'crates\model\src') (Join-Path $modelPath 'src') @()
Copy-SourceTree (Join-Path $sourcePath 'assets\fonts') (Join-Path $toolPath 'assets\fonts') @()
$noticePath = Join-Path $toolPath 'upstream-notices'
New-Item -ItemType Directory -Path $noticePath -Force | Out-Null
foreach ($license in @('LICENSE', 'LICENSE-APACHE')) {
    $licenseSource = Join-Path $sourcePath $license
    if (Test-Path -LiteralPath $licenseSource) {
        $contents = [IO.File]::ReadAllText($licenseSource).Replace("`r`n", "`n")
        [IO.File]::WriteAllText((Join-Path $noticePath $license), $contents, $encoding)
    }
}
