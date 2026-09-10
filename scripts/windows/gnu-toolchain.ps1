# Shared Windows GNU/MSVC toolchain discovery and process-environment setup.
# Keeping this in one file prevents Cargo, CMake, and native tests from
# selecting different compilers or generators.

Set-StrictMode -Version Latest

$script:WindowsGnuTarget = "x86_64-pc-windows-gnu"
$script:WindowsMsvcTarget = "x86_64-pc-windows-msvc"
# HOST_* is kept in the cleanup list only. Setting it to MinGW while Rust still
# uses the MSVC host toolchain makes cc-rs compile host build scripts for the
# wrong ABI; the repository now selects the official GNU Rust host instead.
$script:WindowsGnuEnvironmentNames = @(
    "PATH",
    "RUSTUP_TOOLCHAIN",
    "CC",
    "CXX",
    "AR",
    "RANLIB",
    "WINDRES",
    "RC",
    "HOST_CC",
    "HOST_CXX",
    "HOST_AR",
    "HOST_RANLIB",
    "CFLAGS",
    "CXXFLAGS",
    "ARFLAGS",
    "CMAKE_GENERATOR",
    "CMAKE_MAKE_PROGRAM",
    "CMAKE_C_COMPILER",
    "CMAKE_CXX_COMPILER",
    "CMAKE_AR",
    "CMAKE_RANLIB",
    "CMAKE_RC_COMPILER",
    "CMAKE_SYSTEM_NAME",
    "CMAKE_TOOLCHAIN_FILE",
    "CMAKE_GENERATOR_PLATFORM",
    "CMAKE_GENERATOR_INSTANCE",
    "CMAKE_GENERATOR_TOOLSET",
    "CARGO_BUILD_TARGET",
    "GPUI_FXC_PATH",
    "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER",
    "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
    "CC_x86_64-pc-windows-gnu",
    "CXX_x86_64-pc-windows-gnu",
    "AR_x86_64-pc-windows-gnu",
    "CC_x86_64_pc_windows_gnu",
    "CXX_x86_64_pc_windows_gnu",
    "AR_x86_64_pc_windows_gnu",
    "CC_x86_64-pc-windows-msvc",
    "CXX_x86_64-pc-windows-msvc",
    "AR_x86_64-pc-windows-msvc",
    "CC_x86_64_pc_windows_msvc",
    "CXX_x86_64_pc_windows_msvc",
    "AR_x86_64_pc_windows_msvc"
)

function Get-WindowsGnuTarget {
    return $script:WindowsGnuTarget
}

function Get-WindowsMsvcTarget {
    return $script:WindowsMsvcTarget
}

function Get-WindowsRepositoryChannel {
    # Read only the channel from rust-toolchain.toml. The Windows host suffix is
    # selected by the environment helper so GNU and MSVC can coexist.
    $RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    $ToolchainFile = Join-Path $RepositoryRoot "rust-toolchain.toml"
    if (-not (Test-Path -LiteralPath $ToolchainFile -PathType Leaf)) {
        throw "Rust toolchain file is missing: $ToolchainFile"
    }

    $ChannelLine = Get-Content -LiteralPath $ToolchainFile |
        Where-Object { $_ -match '^\s*channel\s*=' } |
        Select-Object -First 1
    if ($ChannelLine -notmatch '"([^"]+)"') {
        throw "rust-toolchain.toml does not contain a valid channel."
    }

    return $Matches[1]
}

function Get-WindowsRustToolchainForTarget {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Target
    )

    $Channel = Get-WindowsRepositoryChannel
    if ($Channel -match '-x86_64-pc-windows-(gnu|msvc)$') {
        $Channel = $Channel -replace '-x86_64-pc-windows-(gnu|msvc)$', ''
    }
    if ($Channel -match '-x86_64-pc-windows-') {
        throw "rust-toolchain.toml must use a channel or a supported Windows host toolchain."
    }
    return "$Channel-$Target"
}

function Get-WindowsGnuRustToolchain {
    # Keep the Rust host and application target on the same GNU ABI.
    return Get-WindowsRustToolchainForTarget -Target $script:WindowsGnuTarget
}

function Get-WindowsMsvcRustToolchain {
    # Keep the Rust host and application target on the same MSVC ABI.
    return Get-WindowsRustToolchainForTarget -Target $script:WindowsMsvcTarget
}

function Invoke-WindowsRustup {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    # rustup writes progress messages to stderr; keep them visible without
    # letting PowerShell's Stop preference turn normal progress into a failure.
    $ErrorActionPreference = "Continue"
    $Output = @(& rustup @Arguments 2>&1)
    $ExitCode = $LASTEXITCODE
    foreach ($Line in $Output) {
        Write-Host $Line
    }
    if ($ExitCode -ne 0) {
        throw "rustup $($Arguments -join ' ') failed with exit code $ExitCode."
    }
}

function Ensure-WindowsGnuRustToolchain {
    # Install the GNU Rust host and its repository components once, then verify
    # that rustc really reports the GNU host before Cargo is allowed to run.
    if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
        throw "rustup not found. Install Rust from https://rustup.rs before building."
    }

    $RustToolchain = Get-WindowsGnuRustToolchain
    $InstalledToolchains = @(& rustup toolchain list 2>$null)
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to query installed Rust toolchains."
    }
    $InstalledNames = @(
        $InstalledToolchains |
            ForEach-Object { ($_ -split '\s+')[0] } |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    )

    if ($InstalledNames -notcontains $RustToolchain) {
        Invoke-WindowsRustup -Arguments @(
            "toolchain", "install", $RustToolchain,
            "--profile", "minimal",
            "--component", "rustfmt",
            "--component", "clippy",
            "--component", "rust-analyzer",
            "--component", "rust-src"
        )
    }
    else {
        Invoke-WindowsRustup -Arguments @(
            "component", "add",
            "--toolchain", $RustToolchain,
            "rustfmt", "clippy", "rust-analyzer", "rust-src"
        )
    }

    $InstalledTargets = @(& rustup target list --installed --toolchain $RustToolchain 2>$null)
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to query installed Rust targets for $RustToolchain."
    }
    if ($InstalledTargets -notcontains $script:WindowsGnuTarget) {
        Invoke-WindowsRustup -Arguments @(
            "target", "add", "--toolchain", $RustToolchain, $script:WindowsGnuTarget
        )
    }

    $RustcInfo = (@(& rustup run $RustToolchain rustc -vV 2>&1) -join "`n")
    if ($LASTEXITCODE -ne 0 -or $RustcInfo -notmatch '(?m)^host:\s+x86_64-pc-windows-gnu\s*$') {
        throw "Rust toolchain '$RustToolchain' is not an x86_64-pc-windows-gnu host."
    }
    return $RustToolchain
}

function Resolve-WindowsGnuTool {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [string[]]$Candidates = @()
    )

    foreach ($Candidate in $Candidates) {
        if (-not [string]::IsNullOrWhiteSpace($Candidate) -and
            (Test-Path -LiteralPath $Candidate -PathType Leaf)) {
            return [System.IO.Path]::GetFullPath($Candidate)
        }
    }

    $Command = Get-Command "$Name.exe" -ErrorAction SilentlyContinue
    if ($null -ne $Command) {
        return [System.IO.Path]::GetFullPath($Command.Source)
    }

    throw "Windows GNU tool '$Name.exe' was not found. Install the MSYS2 UCRT64 MinGW-w64 toolchain and add its bin directory to PATH."
}

function Get-WindowsGnuToolchain {
    # Resolve GCC first, then prefer sibling tools from the same MinGW prefix.
    # This avoids mixing UCRT64 binaries with another MinGW installation.
    $MsysRoots = @()
    if (-not [string]::IsNullOrWhiteSpace($env:MSYS2_ROOT)) {
        $MsysRoots += $env:MSYS2_ROOT
    }
    $MsysRoots += @("C:\msys64", "C:\msys2")
    $MsysBins = @()
    $MsysBins += $MsysRoots | ForEach-Object { Join-Path $_ "ucrt64\bin" }
    $MsysBins += $MsysRoots | ForEach-Object { Join-Path $_ "mingw64\bin" }

    $Gcc = Resolve-WindowsGnuTool -Name "gcc" -Candidates @(
        $MsysBins | ForEach-Object { Join-Path $_ "gcc.exe" }
    )
    $BinDirectory = Split-Path -Parent $Gcc
    $Gxx = Resolve-WindowsGnuTool -Name "g++" -Candidates @(Join-Path $BinDirectory "g++.exe")
    $Ar = Resolve-WindowsGnuTool -Name "ar" -Candidates @(Join-Path $BinDirectory "ar.exe")
    $Ranlib = Resolve-WindowsGnuTool -Name "ranlib" -Candidates @(Join-Path $BinDirectory "ranlib.exe")
    $Windres = Resolve-WindowsGnuTool -Name "windres" -Candidates @(Join-Path $BinDirectory "windres.exe")
    $Objdump = Resolve-WindowsGnuTool -Name "objdump" -Candidates @(Join-Path $BinDirectory "objdump.exe")
    $Dlltool = Resolve-WindowsGnuTool -Name "dlltool" -Candidates @(Join-Path $BinDirectory "dlltool.exe")
    $Cmake = Resolve-WindowsGnuTool -Name "cmake" -Candidates @(Join-Path $BinDirectory "cmake.exe")
    $Ninja = Resolve-WindowsGnuTool -Name "ninja" -Candidates @(Join-Path $BinDirectory "ninja.exe")

    return [PSCustomObject]@{
        Target = Get-WindowsGnuTarget
        RustToolchain = Get-WindowsGnuRustToolchain
        BinDirectory = $BinDirectory
        Gcc = $Gcc
        Gxx = $Gxx
        Ar = $Ar
        Ranlib = $Ranlib
        Windres = $Windres
        Objdump = $Objdump
        Dlltool = $Dlltool
        Cmake = $Cmake
        Ninja = $Ninja
    }
}

function Get-WindowsGnuEnvironmentNames {
    return $script:WindowsGnuEnvironmentNames
}

function Save-WindowsGnuEnvironment {
    $Snapshot = @{}
    foreach ($Name in Get-WindowsGnuEnvironmentNames) {
        $Value = [Environment]::GetEnvironmentVariable($Name, "Process")
        if ($null -ne $Value) {
            $Snapshot[$Name] = $Value
        }
    }
    return $Snapshot
}

function Clear-WindowsGnuEnvironment {
    foreach ($Name in Get-WindowsGnuEnvironmentNames) {
        [Environment]::SetEnvironmentVariable($Name, $null, "Process")
    }
}

function Set-WindowsGnuEnvironment {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Toolchain
    )

    $OriginalPath = $env:PATH
    Clear-WindowsGnuEnvironment

    $PathEntries = @($Toolchain.BinDirectory) + @(
        $OriginalPath -split [System.IO.Path]::PathSeparator |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    )
    $PathEntries = @($PathEntries | Select-Object -Unique)
    [Environment]::SetEnvironmentVariable("PATH", ($PathEntries -join [System.IO.Path]::PathSeparator), "Process")

    $Environment = @{
        RUSTUP_TOOLCHAIN = $Toolchain.RustToolchain
        CC = $Toolchain.Gcc
        CXX = $Toolchain.Gxx
        AR = $Toolchain.Ar
        RANLIB = $Toolchain.Ranlib
        WINDRES = $Toolchain.Windres
        RC = $Toolchain.Windres
        CMAKE_GENERATOR = "Ninja"
        CMAKE_MAKE_PROGRAM = $Toolchain.Ninja
        CMAKE_C_COMPILER = $Toolchain.Gcc
        CMAKE_CXX_COMPILER = $Toolchain.Gxx
        CMAKE_AR = $Toolchain.Ar
        CMAKE_RANLIB = $Toolchain.Ranlib
        CMAKE_RC_COMPILER = $Toolchain.Windres
        CMAKE_SYSTEM_NAME = "Windows"
        CARGO_BUILD_TARGET = $Toolchain.Target
        CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $Toolchain.Gcc
    }
    foreach ($Entry in $Environment.GetEnumerator()) {
        [Environment]::SetEnvironmentVariable($Entry.Key, $Entry.Value, "Process")
    }

    foreach ($Prefix in @("CC", "CXX", "AR")) {
        [Environment]::SetEnvironmentVariable(
            "${Prefix}_$($Toolchain.Target)",
            $Environment[$Prefix],
            "Process"
        )
        [Environment]::SetEnvironmentVariable(
            "${Prefix}_$($Toolchain.Target.Replace('-', '_'))",
            $Environment[$Prefix],
            "Process"
        )
    }
}

function Restore-WindowsGnuEnvironment {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$Snapshot
    )

    Clear-WindowsGnuEnvironment
    foreach ($Entry in $Snapshot.GetEnumerator()) {
        [Environment]::SetEnvironmentVariable($Entry.Key, $Entry.Value, "Process")
    }
}

function Set-WindowsMsvcEnvironment {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$Snapshot
    )

    $OriginalPath = $null
    if ($Snapshot.ContainsKey("PATH")) {
        $OriginalPath = $Snapshot["PATH"]
    }
    if ($null -eq $OriginalPath) {
        $OriginalPath = [Environment]::GetEnvironmentVariable("PATH", "Process")
    }

    # Remove GNU-specific compiler, Cargo target, and CMake overrides while
    # preserving the caller's original PATH for the Windows default tools.
    Clear-WindowsGnuEnvironment
    if ($null -ne $OriginalPath) {
        [Environment]::SetEnvironmentVariable("PATH", $OriginalPath, "Process")
    }

    $MsvcRustToolchain = Get-WindowsMsvcRustToolchain
    $InstalledToolchains = @(& rustup toolchain list 2>$null)
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to query installed Rust toolchains for the MSVC fallback."
    }
    $InstalledNames = @(
        $InstalledToolchains |
            ForEach-Object { ($_ -split '\s+')[0] } |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
    )
    if ($InstalledNames -notcontains $MsvcRustToolchain) {
        throw "The Windows MSVC fallback toolchain is not installed: $MsvcRustToolchain"
    }

    [Environment]::SetEnvironmentVariable("RUSTUP_TOOLCHAIN", $MsvcRustToolchain, "Process")
    [Environment]::SetEnvironmentVariable("CARGO_BUILD_TARGET", $script:WindowsMsvcTarget, "Process")
}

function Select-WindowsToolchain {
    param(
        [switch]$PreferGnu
    )

    $EnvironmentSnapshot = Save-WindowsGnuEnvironment
    if (-not $PreferGnu) {
        Set-WindowsMsvcEnvironment -Snapshot $EnvironmentSnapshot
        return [PSCustomObject]@{
            Flavor = "MSVC"
            Target = $script:WindowsMsvcTarget
            RustToolchain = Get-WindowsMsvcRustToolchain
            PreviousEnvironment = $EnvironmentSnapshot
            Gcc = $null
            Objdump = $null
        }
    }

    try {
        $Toolchain = Get-WindowsGnuToolchain
        $RustToolchain = Ensure-WindowsGnuRustToolchain
        Set-WindowsGnuEnvironment -Toolchain $Toolchain
        return [PSCustomObject]@{
            Flavor = "GNU"
            Target = $Toolchain.Target
            RustToolchain = $RustToolchain
            PreviousEnvironment = $EnvironmentSnapshot
            Gcc = $Toolchain.Gcc
            Objdump = $Toolchain.Objdump
        }
    }
    catch {
        $GnuFailure = $_.Exception.Message
        try {
            Set-WindowsMsvcEnvironment -Snapshot $EnvironmentSnapshot
        }
        catch {
            Restore-WindowsGnuEnvironment -Snapshot $EnvironmentSnapshot
            throw "GNU toolchain setup failed ($GnuFailure), and the MSVC fallback could not be activated: $($_.Exception.Message)"
        }

        Write-Warning "Windows GNU toolchain is unavailable ($GnuFailure). Falling back to the Windows default MSVC toolchain."
        return [PSCustomObject]@{
            Flavor = "MSVC"
            Target = $script:WindowsMsvcTarget
            RustToolchain = Get-WindowsMsvcRustToolchain
            PreviousEnvironment = $EnvironmentSnapshot
            Gcc = $null
            Objdump = $null
        }
    }
}
