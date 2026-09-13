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
        $Names | Should -Contain "CMAKE_GENERATOR"
        $Names | Should -Not -Contain "CC_x86_64-pc-windows-gnu"
        $Names | Should -Not -Contain "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER"
    }

    It "imports VS variables and sets NMake for CMake" {
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
                VisualStudio = [PSCustomObject]@{}
                Cl = "C:\\VS\\bin\\cl.exe"
                Link = "C:\\VS\\bin\\link.exe"
                Rc = "C:\\SDK\\bin\\rc.exe"
                NMake = "C:\\VS\\bin\\nmake.exe"
                CMake = "C:\\VS\\cmake\\cmake.exe"
            }

            Set-WindowsMsvcEnvironment -Toolchain $Toolchain | Should -Not -BeNullOrEmpty

            $env:RUSTUP_TOOLCHAIN | Should -BeExactly "stable-x86_64-pc-windows-msvc"
            $env:CARGO_BUILD_TARGET | Should -BeExactly "x86_64-pc-windows-msvc"
            $env:CMAKE_GENERATOR | Should -BeExactly "NMake Makefiles"
            $env:CMAKE_MAKE_PROGRAM | Should -BeExactly "C:\\VS\\bin\\nmake.exe"
            $env:CC | Should -BeExactly "C:\\VS\\bin\\cl.exe"
            $env:CXX | Should -BeExactly "C:\\VS\\bin\\cl.exe"
        }
        finally {
            Restore-WindowsMsvcEnvironment -Snapshot $Snapshot
        }
    }
}
