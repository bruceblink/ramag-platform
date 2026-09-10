# Regression tests for Windows GNU preference and MSVC fallback selection.
BeforeAll {
    . (Join-Path $PSScriptRoot "gnu-toolchain.ps1")
}

Describe "Select-WindowsToolchain" {
    BeforeEach {
        $script:EnvironmentSnapshot = Save-WindowsGnuEnvironment
    }

    AfterEach {
        Restore-WindowsGnuEnvironment -Snapshot $script:EnvironmentSnapshot
    }

    It "selects GNU when all GNU prerequisites are available" {
        Mock Get-WindowsGnuToolchain {
            [PSCustomObject]@{
                Target = "x86_64-pc-windows-gnu"
                RustToolchain = "stable-x86_64-pc-windows-gnu"
                Gcc = "gcc.exe"
                Objdump = "objdump.exe"
            }
        }
        Mock Ensure-WindowsGnuRustToolchain { "stable-x86_64-pc-windows-gnu" }
        Mock Set-WindowsGnuEnvironment {}

        $Selection = Select-WindowsToolchain -PreferGnu

        $Selection.Flavor | Should -BeExactly "GNU"
        $Selection.Target | Should -BeExactly "x86_64-pc-windows-gnu"
        $Selection.RustToolchain | Should -BeExactly "stable-x86_64-pc-windows-gnu"
        Should -Invoke Set-WindowsGnuEnvironment -Times 1 -Exactly
    }

    It "falls back to MSVC when GNU setup fails" {
        Mock Get-WindowsGnuToolchain { throw "GNU prerequisites missing" }
        Mock Set-WindowsMsvcEnvironment {}
        Mock Get-WindowsMsvcRustToolchain { "stable-x86_64-pc-windows-msvc" }

        $Selection = Select-WindowsToolchain -PreferGnu

        $Selection.Flavor | Should -BeExactly "MSVC"
        $Selection.Target | Should -BeExactly "x86_64-pc-windows-msvc"
        $Selection.RustToolchain | Should -BeExactly "stable-x86_64-pc-windows-msvc"
        Should -Invoke Set-WindowsMsvcEnvironment -Times 1 -Exactly
    }
}
