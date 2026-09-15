# Run the repository's Windows Rust format and Clippy checks in the shared MSVC environment.
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The Windows Rust quality checks can only run on Windows."
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust with rustup before running the quality checks."
}

$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$ToolchainScript = Join-Path $PSScriptRoot "msvc-toolchain.ps1"
if (-not (Test-Path -LiteralPath $ToolchainScript -PathType Leaf)) {
    throw "Windows MSVC toolchain helper is missing: $ToolchainScript"
}

Set-Location $RepoRoot
. $ToolchainScript
$EnvironmentSnapshot = Save-WindowsMsvcEnvironment
try {
    $Toolchain = Initialize-WindowsMsvcEnvironment
    Write-Host "Running Windows Rust quality checks with $($Toolchain.RustToolchain)."
    Write-Host "  target:    $($Toolchain.Target)"
    Write-Host "  target-dir: $(Get-WindowsCargoTargetDirectory)"
    Write-Host "  generator: $(Get-WindowsMsvcCMakeGenerator)"

    & cargo fmt-check
    if ($LASTEXITCODE -ne 0) {
        throw "cargo fmt-check failed."
    }

    & cargo clippy-all
    if ($LASTEXITCODE -ne 0) {
        throw "cargo clippy-all failed."
    }
}
finally {
    Restore-WindowsMsvcEnvironment -Snapshot $EnvironmentSnapshot
}
