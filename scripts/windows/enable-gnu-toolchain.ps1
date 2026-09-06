# Activates the repository's Windows GNU environment in the current PowerShell.
# Dot-source this file once, then use the same Cargo commands as Linux and macOS.
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "The Windows GNU environment can only be activated on Windows."
}

$ToolchainScript = Join-Path $PSScriptRoot "gnu-toolchain.ps1"
if (-not (Test-Path -LiteralPath $ToolchainScript -PathType Leaf)) {
    throw "Windows GNU toolchain helper is missing: $ToolchainScript"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust with rustup before activating the Windows GNU environment."
}
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw "rustup not found. Install Rust from https://rustup.rs before activating the Windows GNU environment."
}

. $ToolchainScript

# Resolve the complete native tool set before changing PATH so a partial MSYS2
# installation fails immediately instead of producing a less useful Cargo error.
$RustToolchain = Ensure-WindowsGnuRustToolchain
$Toolchain = Get-WindowsGnuToolchain

Set-WindowsGnuEnvironment -Toolchain $Toolchain
Write-Host "Windows GNU environment is active for this PowerShell session."
Write-Host "  rust:   $RustToolchain"
Write-Host "  target: $($Toolchain.Target)"
Write-Host "  gcc:    $($Toolchain.Gcc)"
Write-Host "  cmake:  $($Toolchain.Cmake)"
Write-Host "  ninja:  $($Toolchain.Ninja)"
Write-Host "Use standard Cargo commands now, for example: cargo build or cargo run -p ramag-bin"
