# Regression tests for the single Windows MSVC toolchain selection.
BeforeAll {
    . (Join-Path $PSScriptRoot "msvc-toolchain.ps1")
}

Describe "Windows MSVC toolchain" {
    It "uses the stable MSVC target from the repository channel" {
        Get-WindowsMsvcTarget | Should -BeExactly "x86_64-pc-windows-msvc"
        Get-WindowsMsvcRustToolchain | Should -BeExactly "stable-x86_64-pc-windows-msvc"
    }

    It "does not expose GNU compiler variables in the active environment list" {
        $Names = @(Get-WindowsMsvcEnvironmentNames)

        $Names | Should -Contain "CARGO_BUILD_TARGET"
        $Names | Should -Contain "CARGO_TARGET_DIR"
        $Names | Should -Contain "CMAKE_GENERATOR"
        $Names | Should -Contain "CMAKE_GENERATOR_INSTANCE"
        $Names | Should -Contain "CMAKE_GENERATOR_PLATFORM"
        $Names | Should -Contain "CMAKE_GENERATOR_TOOLSET"
        $Names | Should -Not -Contain "CMAKE_MAKE_PROGRAM"
        $Names | Should -Not -Contain "CC_x86_64-pc-windows-gnu"
        $Names | Should -Not -Contain "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER"
    }

    It "uses the Visual Studio generator and repository target directory" {
        $Snapshot = Save-WindowsMsvcEnvironment
        try {
            Mock Get-WindowsMsvcEnvironmentFromVisualStudio {
                @{
                    PATH = "C:\\VS\\bin"
                    INCLUDE = "C:\\VS\\include"
                    LIB = "C:\\VS\\lib"
                }
            }
            $Toolchain = [PSCustomObject]@{
                Target = "x86_64-pc-windows-msvc"
                RustToolchain = "stable-x86_64-pc-windows-msvc"
                VisualStudio = [PSCustomObject]@{ InstallationPath = "C:\\VS" }
                Cl = "C:\\VS\\bin\\cl.exe"
                Link = "C:\\VS\\bin\\link.exe"
                Rc = "C:\\SDK\\bin\\rc.exe"
                CMake = "C:\\VS\\cmake\\cmake.exe"
            }

            $env:CMAKE_MAKE_PROGRAM = "C:\\old\\stale-make.exe"
            Set-WindowsMsvcEnvironment -Toolchain $Toolchain | Should -Not -BeNullOrEmpty

            $env:RUSTUP_TOOLCHAIN | Should -BeExactly "stable-x86_64-pc-windows-msvc"
            $env:CARGO_BUILD_TARGET | Should -BeExactly "x86_64-pc-windows-msvc"
            $env:CARGO_TARGET_DIR | Should -BeExactly (Get-WindowsCargoTargetDirectory)
            $env:CMAKE_GENERATOR | Should -BeExactly "Visual Studio 18 2026"
            $env:CMAKE_GENERATOR_PLATFORM | Should -BeExactly "x64"
            $env:CMAKE_GENERATOR_INSTANCE | Should -BeExactly "C:\\VS"
            $env:CMAKE_GENERATOR_TOOLSET | Should -BeExactly "host=x64"
            $env:CMAKE_MAKE_PROGRAM | Should -BeNullOrEmpty
            $env:PROCESSOR_ARCHITECTURE | Should -BeExactly "AMD64"
            $env:CC | Should -BeExactly "C:\\VS\\bin\\cl.exe"
            $env:CXX | Should -BeExactly "C:\\VS\\bin\\cl.exe"
        }
        finally {
            Restore-WindowsMsvcEnvironment -Snapshot $Snapshot
        }
    }
}
