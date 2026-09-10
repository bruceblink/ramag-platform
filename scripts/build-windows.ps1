# Windows 原生 x64 构建；Release 由 Windows SDK fxc.exe 预编译 GPUI 着色器。
# `-Release -Fast` 使用 release-fast profile，供本地日常迭代构建。
param(
    [switch]$Release,
    [switch]$Fast
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
if ($Fast -and -not $Release) {
    throw "-Fast requires -Release."
}
$BuildProfile = if ($Fast) { "release-fast" } elseif ($Release) { "release" } else { "debug" }
$RepoDir = Split-Path -Parent $PSScriptRoot
$DependencyHelper = Join-Path $PSScriptRoot "windows\pe-dependencies.ps1"
$ToolchainHelper = Join-Path $PSScriptRoot "windows\gnu-toolchain.ps1"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "This script must run on Windows. Use the native Cargo command on Linux or macOS."
}
if (-not (Test-Path -LiteralPath $DependencyHelper -PathType Leaf)) {
    throw "PE dependency helper is missing: $DependencyHelper"
}
if (-not (Test-Path -LiteralPath $ToolchainHelper -PathType Leaf)) {
    throw "Windows GNU toolchain helper is missing: $ToolchainHelper"
}
. $DependencyHelper
. $ToolchainHelper

Set-Location $RepoDir
$EnvironmentSnapshot = Save-WindowsGnuEnvironment
$Toolchain = Select-WindowsToolchain -PreferGnu
$Target = $Toolchain.Target
Write-Host "Using Rust host toolchain: $($Toolchain.RustToolchain)"
Write-Host "Using Windows $($Toolchain.Flavor) toolchain: $($Toolchain.Target)"
if ($Toolchain.Flavor -eq "GNU") {
    Write-Host "  gcc: $($Toolchain.Gcc)"
}

function Find-Fxc {
    $Command = Get-Command fxc.exe -ErrorAction SilentlyContinue
    if ($null -ne $Command) {
        return $Command.Source
    }

    $RegistryPath = "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots"
    $InstalledRoots = Get-ItemProperty -LiteralPath $RegistryPath -ErrorAction SilentlyContinue
    if ($null -eq $InstalledRoots) {
        return $null
    }
    $KitsRootProperty = $InstalledRoots.PSObject.Properties["KitsRoot10"]
    if ($null -eq $KitsRootProperty) {
        return $null
    }
    $KitsRoot = $KitsRootProperty.Value
    if ([string]::IsNullOrWhiteSpace($KitsRoot)) {
        return $null
    }

    $BinDir = Join-Path $KitsRoot "bin"
    if (-not (Test-Path -LiteralPath $BinDir -PathType Container)) {
        return $null
    }
    return Get-ChildItem -LiteralPath $BinDir -Directory |
        Where-Object { $_.Name -match '^\d+(\.\d+){1,3}$' } |
        Sort-Object { [version]$_.Name } -Descending |
        ForEach-Object { Join-Path $_.FullName "x64\fxc.exe" } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
}

function Assert-PeTarget {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [bool]$Gui
    )

    $Stream = [System.IO.File]::OpenRead($Path)
    $Reader = [System.IO.BinaryReader]::new($Stream)
    try {
        if ($Reader.ReadUInt16() -ne 0x5A4D) {
            throw "Output is not a PE executable: $Path"
        }
        $Stream.Seek(0x3C, [System.IO.SeekOrigin]::Begin) | Out-Null
        $PeOffset = $Reader.ReadInt32()
        $Stream.Seek($PeOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
        if ($Reader.ReadUInt32() -ne 0x00004550) {
            throw "Output has an invalid PE signature: $Path"
        }
        if ($Reader.ReadUInt16() -ne 0x8664) {
            throw "Output is not an x64 executable: $Path"
        }
        $OptionalHeader = $PeOffset + 24
        $Stream.Seek($OptionalHeader, [System.IO.SeekOrigin]::Begin) | Out-Null
        if ($Reader.ReadUInt16() -ne 0x020B) {
            throw "Output is not a PE32+ executable: $Path"
        }
        $Stream.Seek($OptionalHeader + 68, [System.IO.SeekOrigin]::Begin) | Out-Null
        $Subsystem = $Reader.ReadUInt16()
        $ExpectedSubsystem = if ($Gui) { 2 } else { 3 }
        if ($Subsystem -ne $ExpectedSubsystem) {
            throw "Unexpected PE subsystem $Subsystem (expected $ExpectedSubsystem): $Path"
        }
    }
    finally {
        $Reader.Dispose()
        $Stream.Dispose()
    }
}

try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "cargo not found. Install Rust with rustup before building."
    }

    if ($Release) {
        $Fxc = Find-Fxc
        if ([string]::IsNullOrWhiteSpace($Fxc)) {
            throw "fxc.exe not found. Install the Windows 10/11 SDK before building a release."
        }
        $env:GPUI_FXC_PATH = $Fxc
        Write-Host "Using HLSL compiler: $Fxc"
    }

    $CargoArgs = @("build", "--locked", "-p", "ramag-bin")
    if ($Fast) {
        $CargoArgs += @("--profile", $BuildProfile)
    }
    elseif ($Release) {
        $CargoArgs += "--release"
    }
    Write-Host "Using Cargo profile: $BuildProfile"
    Write-Host "Running: cargo $($CargoArgs -join ' ')"

    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Windows $BuildProfile build failed with the $($Toolchain.Flavor) toolchain. Verify the selected Windows compiler, CMake, Ninja, and the Windows SDK, then retry."
    }

    $Exe = Join-Path $RepoDir "target\$Target\$BuildProfile\ramag.exe"
    if (-not (Test-Path -LiteralPath $Exe -PathType Leaf)) {
        throw "Build finished without the expected executable: $Exe"
    }

    $VersionInfo = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Exe)
    if ($VersionInfo.ProductName -ne "Ramag") {
        throw "The executable is missing the expected Windows version resource: $Exe"
    }
    Assert-PeTarget -Path $Exe -Gui $Release.IsPresent

    if ($Toolchain.Flavor -eq "GNU") {
        $Objdump = $Toolchain.Objdump
        Write-Host "Using GNU PE inspector: $Objdump"
        $Dependencies = (& $Objdump -p $Exe) -join "`n"
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to inspect executable dependencies with objdump.exe."
        }
        $DependencyNames = @(
            [regex]::Matches(
                $Dependencies,
                '(?im)^\s*DLL Name:\s*([A-Z0-9._+-]+\.dll)\s*$'
            ) |
                ForEach-Object { $_.Groups[1].Value } |
                Sort-Object -Unique
        )
        if ($DependencyNames.Count -eq 0) {
            throw "objdump.exe returned no PE dependencies for $Exe."
        }
        Write-Host "PE dependencies: $($DependencyNames -join ', ')"

        $DynamicGnuRuntime = @(
            $DependencyNames | Where-Object {
                $_ -match '^(libgcc_s_seh-1|libstdc\+\+-6|libwinpthread-1|libssp-0)\.dll$'
            }
        )
        if ($DynamicGnuRuntime.Count -gt 0) {
            throw "The executable depends on the dynamic GNU runtime: $($DynamicGnuRuntime -join ', ')"
        }

        $SystemDirectory = [System.Environment]::SystemDirectory
        $NonSystemDependencies = @(
            Get-UnpackagedPeDependencies `
                -DependencyNames $DependencyNames `
                -SystemDirectory $SystemDirectory
        )
        if ($NonSystemDependencies.Count -gt 0) {
            throw "The executable has unpackaged non-system dependencies: $($NonSystemDependencies -join ', ')"
        }
    }
    else {
        Write-Host "MSVC build selected; GNU-specific PE dependency inspection is not applicable."
    }

    $Size = (Get-Item -LiteralPath $Exe).Length
    Write-Host "Windows $BuildProfile build completed: $Exe ($Size bytes)"
}
finally {
    Restore-WindowsGnuEnvironment -Snapshot $EnvironmentSnapshot
}
