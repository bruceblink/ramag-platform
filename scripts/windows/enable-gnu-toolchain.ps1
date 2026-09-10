# Activates the repository's Windows GNU-preferred environment in the current
# PowerShell, with automatic MSVC fallback.
# Dot-source this file once, then use the same Cargo commands as Linux/macOS.
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

# Prefer the repository's GNU host, but keep ordinary Windows development
# usable when Rust GNU or the MinGW/CMake/Ninja tool set is unavailable.
$Toolchain = Select-WindowsToolchain -PreferGnu
Write-Host "Windows $($Toolchain.Flavor) environment is active for this PowerShell session."
Write-Host "  rust:   $($Toolchain.RustToolchain)"
Write-Host "  target: $($Toolchain.Target)"
if ($Toolchain.Flavor -eq "GNU") {
    Write-Host "  gcc:    $($Toolchain.Gcc)"
}
else {
    Write-Host "  msvc:   Windows default linker and SDK"
}
Write-Host "Use standard Cargo commands now, for example: cargo build or cargo run -p ramag-bin"
