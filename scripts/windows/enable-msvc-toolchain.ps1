# Activates the repository's single Windows MSVC environment in the current
# PowerShell. Dot-source this file once before running Cargo commands.
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The Windows MSVC environment can only be activated on Windows."
}

$ToolchainScript = Join-Path $PSScriptRoot "msvc-toolchain.ps1"
if (-not (Test-Path -LiteralPath $ToolchainScript -PathType Leaf)) {
    throw "Windows MSVC toolchain helper is missing: $ToolchainScript"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust with rustup before activating the Windows MSVC environment."
}
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw "rustup not found. Install Rust from https://rustup.rs before activating the Windows MSVC environment."
}

. $ToolchainScript

$Toolchain = Initialize-WindowsMsvcEnvironment
Write-Host "Windows MSVC environment is active for this PowerShell session."
Write-Host "  rust:   $($Toolchain.RustToolchain)"
Write-Host "  target: $($Toolchain.Target)"
Write-Host "  cl:     $($Toolchain.Cl)"
Write-Host "  cmake:  $($Toolchain.CMake)"
Write-Host "  nmake:  $($Toolchain.NMake)"
Write-Host "Use standard Cargo commands now, for example: cargo build or cargo run -p ramag-bin"
