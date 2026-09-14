# Shared Windows MSVC discovery and process-environment setup.
# The helper keeps Rust, CMake, and native build scripts on one VS18 toolchain.

Set-StrictMode -Version Latest

$script:WindowsMsvcTarget = "x86_64-pc-windows-msvc"
$script:WindowsMsvcCMakeGenerator = "Visual Studio 18 2026"
$script:WindowsMsvcCMakePlatform = "x64"
$script:WindowsMsvcCMakeToolset = "host=x64"
$script:WindowsMsvcEnvironmentNames = @(
    "PATH",
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "RUSTUP_TOOLCHAIN",
    "CARGO_BUILD_TARGET",
    "CARGO_TARGET_DIR",
    "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
    "PROCESSOR_ARCHITECTURE",
    "PROCESSOR_ARCHITEW6432",
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
    "CMAKE_C_COMPILER",
    "CMAKE_CXX_COMPILER",
    "CMAKE_AR",
    "CMAKE_RANLIB",
    "CMAKE_RC_COMPILER",
    "CMAKE_SYSTEM_NAME",
    "CMAKE_TOOLCHAIN_FILE",
    "CMAKE_VS_GLOBALS",
    "CMAKE_TRY_COMPILE_PLATFORM_VARIABLES",
    "CMAKE_GENERATOR_PLATFORM",
    "CMAKE_GENERATOR_INSTANCE",
    "CMAKE_GENERATOR_TOOLSET",
    "GPUI_FXC_PATH",
    "VCToolsInstallDir",
    "VCToolsVersion",
    "VCINSTALLDIR",
    "VSINSTALLDIR",
    "VisualStudioVersion",
    "VSCMD_ARG_app_plat",
    "VSCMD_ARG_HOST_ARCH",
    "VSCMD_ARG_TGT_ARCH",
    "VSCMD_VER",
    "WindowsSdkDir",
    "WindowsSDKVersion",
    "UniversalCRTSdkDir",
    "UCRTVersion",
    "DevEnvDir",
    "CommandPromptType"
)

$script:WindowsMsvcOverrideNames = @(
    $script:WindowsMsvcEnvironmentNames | Where-Object {
        $_ -notin @(
            "PATH",
            "INCLUDE",
            "LIB",
            "LIBPATH",
            "PROCESSOR_ARCHITECTURE",
            "PROCESSOR_ARCHITEW6432"
        )
    }
) + @(
    "CMAKE_MAKE_PROGRAM",
    "CC_x86_64-pc-windows-gnu",
    "CXX_x86_64-pc-windows-gnu",
    "AR_x86_64-pc-windows-gnu",
    "CC_x86_64_pc_windows_gnu",
    "CXX_x86_64_pc_windows_gnu",
    "AR_x86_64_pc_windows_gnu",
    "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER"
)

function Get-WindowsMsvcTarget {
    return $script:WindowsMsvcTarget
}

function Get-WindowsMsvcCMakeGenerator {
    return $script:WindowsMsvcCMakeGenerator
}

function Get-WindowsMsvcCMakePlatform {
    return $script:WindowsMsvcCMakePlatform
}

function Get-WindowsMsvcCMakeToolset {
    return $script:WindowsMsvcCMakeToolset
}

function Get-WindowsMsvcCMakeToolchainFile {
    $ToolchainFile = Join-Path (Get-WindowsRepositoryRoot) "scripts\windows\msvc-cmake-toolchain.cmake"
    if (-not (Test-Path -LiteralPath $ToolchainFile -PathType Leaf)) {
        throw "Windows MSVC CMake toolchain file is missing: $ToolchainFile"
    }
    return $ToolchainFile
}

function Get-WindowsMsvcEnvironmentNames {
    return $script:WindowsMsvcEnvironmentNames
}

function Get-WindowsRepositoryRoot {
    return Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
}

function Get-WindowsCargoTargetDirectory {
    # Keep every Windows Cargo artifact below the repository target directory,
    # including commands launched from a different current directory.
    return Join-Path (Get-WindowsRepositoryRoot) "target"
}

function Get-WindowsRepositoryChannel {
    $ToolchainFile = Join-Path (Get-WindowsRepositoryRoot) "rust-toolchain.toml"
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

function Get-WindowsMsvcRustToolchain {
    $Channel = Get-WindowsRepositoryChannel
    if ($Channel -match '-x86_64-pc-windows-(gnu|msvc)$') {
        $Channel = $Channel -replace '-x86_64-pc-windows-(gnu|msvc)$', ''
    }
    if ($Channel -match '-x86_64-pc-windows-') {
        throw "rust-toolchain.toml must use a channel or the Windows MSVC host toolchain."
    }
    return "$Channel-$script:WindowsMsvcTarget"
}

function Get-VisualStudio18Toolchain {
    # Locate the newest complete VS18 installation with an x64 compiler.
    $ProgramFilesX86 = [System.Environment]::GetFolderPath("ProgramFilesX86")
    $Vswhere = Join-Path $ProgramFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $Vswhere -PathType Leaf)) {
        throw "vswhere.exe is required to locate Visual Studio 18 2026."
    }

    $JsonLines = & $Vswhere `
        -products '*' `
        -all `
        -prerelease `
        -version "[18.0,19.0)" `
        -format json 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to query Visual Studio 18 2026 installations."
    }

    try {
        $Instances = @(($JsonLines -join "`n") | ConvertFrom-Json)
    }
    catch {
        throw "Visual Studio 18 2026 installation data is invalid: $($_.Exception.Message)"
    }

    $Instances = @(
        $Instances |
            Where-Object {
                -not [string]::IsNullOrWhiteSpace([string]$_.installationPath) -and
                $_.isComplete -and
                $_.isLaunchable
            } |
            Sort-Object { [version]$_.installationVersion } -Descending
    )

    foreach ($Instance in $Instances) {
        $InstallationPath = [string]$Instance.installationPath
        $VcVarsAll = Join-Path $InstallationPath "VC\Auxiliary\Build\vcvarsall.bat"
        $MsvcRoot = Join-Path $InstallationPath "VC\Tools\MSVC"
        if (-not (Test-Path -LiteralPath $VcVarsAll -PathType Leaf) -or
            -not (Test-Path -LiteralPath $MsvcRoot -PathType Container)) {
            continue
        }

        $Toolset = Get-ChildItem -LiteralPath $MsvcRoot -Directory |
            Sort-Object { [version]$_.Name } -Descending |
            Where-Object {
                Test-Path -LiteralPath (Join-Path $_.FullName "bin\Hostx64\x64\cl.exe") -PathType Leaf
            } |
            Select-Object -First 1
        if ($null -eq $Toolset) {
            continue
        }

        $ToolsetVersion = ([regex]::Match($Toolset.Name, '^\d+\.\d+')).Value
        return [PSCustomObject]@{
            InstallationPath = $InstallationPath
            InstallationVersion = [string]$Instance.installationVersion
            VcVars64 = $VcVarsAll
            VcVarsArguments = @("x64", "-vcvars_ver=$ToolsetVersion")
            ToolsetVersion = [string]$Toolset.Name
            ToolsetPath = $Toolset.FullName
        }
    }

    throw "A complete, launchable Visual Studio 18 2026 installation with a usable x64 MSVC toolset is required."
}

function Resolve-WindowsMsvcTool {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,
        [string[]]$Candidates = @(),
        [switch]$AllowPathLookup
    )

    foreach ($Candidate in $Candidates) {
        if (-not [string]::IsNullOrWhiteSpace($Candidate) -and
            (Test-Path -LiteralPath $Candidate -PathType Leaf)) {
            return [System.IO.Path]::GetFullPath($Candidate)
        }
    }

    if ($AllowPathLookup) {
        $Command = Get-Command "$Name.exe" -ErrorAction SilentlyContinue
        if ($null -ne $Command) {
            return [System.IO.Path]::GetFullPath($Command.Source)
        }
    }

    throw "Visual Studio Windows tool '$Name.exe' was not found. Install the VS18 C++ workload and Windows SDK."
}

function Get-WindowsSdkTool {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $InstalledRoots = Get-ItemProperty `
        -LiteralPath "HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots" `
        -ErrorAction SilentlyContinue
    $KitsRootProperty = if ($null -ne $InstalledRoots) {
        $InstalledRoots.PSObject.Properties["KitsRoot10"]
    }
    else {
        $null
    }
    if ($null -eq $KitsRootProperty -or [string]::IsNullOrWhiteSpace([string]$KitsRootProperty.Value)) {
        throw "Windows SDK installation root was not found."
    }

    $BinRoot = Join-Path ([string]$KitsRootProperty.Value) "bin"
    $Tool = Get-ChildItem -LiteralPath $BinRoot -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+(\.\d+){1,3}$' } |
        Sort-Object { [version]$_.Name } -Descending |
        ForEach-Object {
            $Candidate = Join-Path $_.FullName "x64\$Name.exe"
            if (Test-Path -LiteralPath $Candidate -PathType Leaf) {
                $Candidate
            }
        } |
        Select-Object -First 1
    if ($null -eq $Tool) {
        throw "Windows SDK tool '$Name.exe' was not found. Install the Windows 10/11 SDK."
    }
    return [System.IO.Path]::GetFullPath([string]$Tool)
}

function Get-WindowsMsvcToolchain {
    $VisualStudio = Get-VisualStudio18Toolchain
    $NativeBin = Join-Path $VisualStudio.ToolsetPath "bin\Hostx64\x64"
    $VisualStudioCMake = Join-Path $VisualStudio.InstallationPath `
        "Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
    $CMake = Resolve-WindowsMsvcTool -Name "cmake" -Candidates @($VisualStudioCMake) -AllowPathLookup
    return [PSCustomObject]@{
        Target = Get-WindowsMsvcTarget
        RustToolchain = Get-WindowsMsvcRustToolchain
        VisualStudio = $VisualStudio
        Cl = Resolve-WindowsMsvcTool -Name "cl" -Candidates @(Join-Path $NativeBin "cl.exe")
        Link = Resolve-WindowsMsvcTool -Name "link" -Candidates @(Join-Path $NativeBin "link.exe")
        Lib = Resolve-WindowsMsvcTool -Name "lib" -Candidates @(Join-Path $NativeBin "lib.exe")
        Rc = Get-WindowsSdkTool -Name "rc"
        CMake = $CMake
    }
}

function Get-WindowsMsvcEnvironmentFromVisualStudio {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$VisualStudio
    )

    $VcVarsArguments = $VisualStudio.VcVarsArguments -join " "
    $Command = 'call "{0}" {1} >nul && set' -f $VisualStudio.VcVars64, $VcVarsArguments
    $Output = @(& cmd.exe /d /s /c $Command 2>&1)
    $ExitCode = $LASTEXITCODE
    if ($ExitCode -ne 0) {
        throw "Visual Studio vcvarsall.bat failed with exit code $ExitCode."
    }

    $Environment = @{}
    foreach ($Line in $Output) {
        $Text = [string]$Line
        if ($Text -match '^([^=]+)=(.*)$') {
            $Environment[$Matches[1]] = $Matches[2]
        }
    }
    if (-not $Environment.ContainsKey("PATH") -or
        -not $Environment.ContainsKey("INCLUDE") -or
        -not $Environment.ContainsKey("LIB")) {
        throw "Visual Studio vcvarsall.bat did not return a complete x64 compiler environment."
    }
    return $Environment
}

function Invoke-WindowsRustup {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    # rustup writes routine progress messages to stderr. Capture them without
    # letting Windows PowerShell's Stop preference turn them into exceptions;
    # the native exit code remains the authoritative failure signal.
    $PreviousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $Output = @(& rustup @Arguments 2>&1)
        $ExitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $PreviousErrorActionPreference
    }
    foreach ($Line in $Output) {
        Write-Host $Line
    }
    if ($ExitCode -ne 0) {
        throw "rustup $($Arguments -join ' ') failed with exit code $ExitCode."
    }
}

function Ensure-WindowsMsvcRustToolchain {
    # Install the repository's stable MSVC host and target before Cargo runs.
    if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
        throw "rustup not found. Install Rust from https://rustup.rs before building."
    }

    $RustToolchain = Get-WindowsMsvcRustToolchain
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
    if ($InstalledTargets -notcontains $script:WindowsMsvcTarget) {
        Invoke-WindowsRustup -Arguments @(
            "target", "add", "--toolchain", $RustToolchain, $script:WindowsMsvcTarget
        )
    }

    $RustcInfo = (@(& rustup run $RustToolchain rustc -vV 2>&1) -join "`n")
    if ($LASTEXITCODE -ne 0 -or $RustcInfo -notmatch '(?m)^host:\s+x86_64-pc-windows-msvc\s*$') {
        throw "Rust toolchain '$RustToolchain' is not an x86_64-pc-windows-msvc host."
    }
    return $RustToolchain
}

function Save-WindowsMsvcEnvironment {
    $Snapshot = @{}
    foreach ($Entry in Get-ChildItem Env:) {
        $Snapshot[$Entry.Name] = $Entry.Value
    }
    return $Snapshot
}

function Clear-WindowsMsvcOverrides {
    foreach ($Name in $script:WindowsMsvcOverrideNames) {
        [Environment]::SetEnvironmentVariable($Name, $null, "Process")
    }
}

function Set-WindowsMsvcEnvironment {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Toolchain
    )

    # Keep Cargo and system commands available while replacing compiler paths.
    $OriginalPath = [Environment]::GetEnvironmentVariable("PATH", "Process")

    # Remove inherited GNU/compiler overrides before importing VS18 variables.
    Clear-WindowsMsvcOverrides
    $VisualStudioEnvironment = Get-WindowsMsvcEnvironmentFromVisualStudio `
        -VisualStudio $Toolchain.VisualStudio
    foreach ($Name in Get-WindowsMsvcEnvironmentNames) {
        if ($VisualStudioEnvironment.ContainsKey($Name)) {
            [Environment]::SetEnvironmentVariable(
                $Name,
                $VisualStudioEnvironment[$Name],
                "Process"
            )
        }
    }

    # MSBuild chooses Hostx64 tools from the process architecture. Some
    # automation shells omit these standard variables, so restore the x64
    # value explicitly before CMake generates Visual Studio projects.
    $ProcessArchitecture = [Environment]::GetEnvironmentVariable(
        "PROCESSOR_ARCHITECTURE",
        "Process"
    )
    $ProcessArchitectureWow = [Environment]::GetEnvironmentVariable(
        "PROCESSOR_ARCHITEW6432",
        "Process"
    )
    if ($ProcessArchitecture -notin @("AMD64", "ARM64") -and
        $ProcessArchitectureWow -notin @("AMD64", "ARM64")) {
        if ([Environment]::Is64BitProcess) {
            $ProcessArchitecture = "AMD64"
        }
        elseif ([Environment]::Is64BitOperatingSystem) {
            $ProcessArchitecture = "x86"
            $ProcessArchitectureWow = "AMD64"
        }
        else {
            throw "A 64-bit Windows process is required for the x64 MSVC toolchain."
        }
        [Environment]::SetEnvironmentVariable(
            "PROCESSOR_ARCHITECTURE",
            $ProcessArchitecture,
            "Process"
        )
        [Environment]::SetEnvironmentVariable(
            "PROCESSOR_ARCHITEW6432",
            $ProcessArchitectureWow,
            "Process"
        )
    }

    $PathEntries = @(
        (Split-Path -Parent $Toolchain.Cl),
        (Split-Path -Parent $Toolchain.CMake)
    )
    $PathEntries += @(
        $env:PATH -split [System.IO.Path]::PathSeparator |
            Where-Object { $_ -notmatch '(?i)(msys|mingw|strawberry-perl)' }
    )
    $PathEntries += @(
        $OriginalPath -split [System.IO.Path]::PathSeparator |
            Where-Object { $_ -notmatch '(?i)(msys|mingw|strawberry-perl)' }
    )
    $PathEntries = @(
        $PathEntries |
            Where-Object { -not [string]::IsNullOrWhiteSpace($_) } |
            Select-Object -Unique
    )
    [Environment]::SetEnvironmentVariable(
        "PATH",
        ($PathEntries -join [System.IO.Path]::PathSeparator),
        "Process"
    )

    $VisualStudioInstance = [string]$Toolchain.VisualStudio.InstallationPath
    if ([string]::IsNullOrWhiteSpace($VisualStudioInstance)) {
        throw "Visual Studio installation path is required for the CMake generator instance."
    }

    $Environment = @{
        RUSTUP_TOOLCHAIN = $Toolchain.RustToolchain
        CARGO_BUILD_TARGET = $Toolchain.Target
        CARGO_TARGET_DIR = Get-WindowsCargoTargetDirectory
        CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = $Toolchain.Link
        CC = $Toolchain.Cl
        CXX = $Toolchain.Cl
        RC = $Toolchain.Rc
        CMAKE_GENERATOR = Get-WindowsMsvcCMakeGenerator
        CMAKE_C_COMPILER = $Toolchain.Cl
        CMAKE_CXX_COMPILER = $Toolchain.Cl
        CMAKE_RC_COMPILER = $Toolchain.Rc
        CMAKE_SYSTEM_NAME = "Windows"
        CMAKE_TOOLCHAIN_FILE = Get-WindowsMsvcCMakeToolchainFile
        # cmake crate forwards these CMAKE_* variables as configure -D options.
        CMAKE_VS_GLOBALS = "UseEnv=true"
        CMAKE_TRY_COMPILE_PLATFORM_VARIABLES = "CMAKE_VS_GLOBALS"
        CMAKE_GENERATOR_PLATFORM = Get-WindowsMsvcCMakePlatform
        CMAKE_GENERATOR_INSTANCE = $VisualStudioInstance
        CMAKE_GENERATOR_TOOLSET = Get-WindowsMsvcCMakeToolset
    }
    foreach ($Entry in $Environment.GetEnumerator()) {
        [Environment]::SetEnvironmentVariable($Entry.Key, $Entry.Value, "Process")
    }
    return $Toolchain
}

function Restore-WindowsMsvcEnvironment {
    param(
        [Parameter(Mandatory = $true)]
        [hashtable]$Snapshot
    )

    $CurrentNames = @(Get-ChildItem Env: | Select-Object -ExpandProperty Name)
    foreach ($Name in $CurrentNames) {
        if (-not $Snapshot.ContainsKey($Name)) {
            [Environment]::SetEnvironmentVariable($Name, $null, "Process")
        }
    }
    foreach ($Entry in $Snapshot.GetEnumerator()) {
        [Environment]::SetEnvironmentVariable($Entry.Key, $Entry.Value, "Process")
    }
}

function Initialize-WindowsMsvcEnvironment {
    $Toolchain = Get-WindowsMsvcToolchain
    $Toolchain.RustToolchain = Ensure-WindowsMsvcRustToolchain
    return Set-WindowsMsvcEnvironment -Toolchain $Toolchain
}
