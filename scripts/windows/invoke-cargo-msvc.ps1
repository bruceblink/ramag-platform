# Run one Cargo command in the repository's shared Windows MSVC environment.
param(
    [switch]$DenyWarnings,
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$CargoCommand,
    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [string[]]$CargoOptions
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ToolchainScript = Join-Path $PSScriptRoot "msvc-toolchain.ps1"
if (-not (Test-Path -LiteralPath $ToolchainScript -PathType Leaf)) {
    throw "Windows MSVC toolchain helper is missing: $ToolchainScript"
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw "cargo not found. Install Rust with rustup before running Cargo."
}

$RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $RepoRoot
. $ToolchainScript
$CargoArguments = @($CargoCommand) + @($CargoOptions)
$EnvironmentSnapshot = Save-WindowsMsvcEnvironment
try {
    $Toolchain = Initialize-WindowsMsvcEnvironment
    Write-Host "Running Cargo with $($Toolchain.RustToolchain) for target $($Toolchain.Target)."
    Write-Host "Target directory: $(Get-WindowsCargoTargetDirectory)"
    if ($DenyWarnings) {
        # PowerShell treats a literal `--` after -File as its own argument
        # terminator, so expose the Clippy lint suffix as an explicit switch.
        $CargoArguments += @("--", "-D", "warnings")
    }
    & cargo @CargoArguments
    exit $LASTEXITCODE
}
finally {
    Restore-WindowsMsvcEnvironment -Snapshot $EnvironmentSnapshot
}
