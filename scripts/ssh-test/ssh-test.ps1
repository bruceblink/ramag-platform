[CmdletBinding()]
param(
    [ValidateSet("up", "status", "test", "down", "clean")]
    [string]$Command = "status"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepositoryRoot = Split-Path -Parent (Split-Path -Parent $ScriptDirectory)
$ComposeFile = Join-Path $ScriptDirectory "compose.yaml"
$ProjectName = "ramag-ssh-test"
$ContainerName = "ramag-ssh-test"
$TestHost = "127.0.0.1"
$TestPort = 12222
$TestUser = "ramag"
$TestRoot = "/home/ramag/ramag-fixture"
$FixtureDirectory = Join-Path $ScriptDirectory "fixtures"
$AuthorizedKeysPath = Join-Path $FixtureDirectory "authorized_keys"
$CargoWrapper = Join-Path $RepositoryRoot "scripts\windows\invoke-cargo-msvc.ps1"
$KeyDirectory = Join-Path ([IO.Path]::GetTempPath()) ("ramag-ssh-test-{0}" -f [Guid]::NewGuid().ToString("N"))
$PrivateKeyPath = Join-Path $KeyDirectory "id_ed25519"
$PublicKeyPath = "$PrivateKeyPath.pub"
$KnownHostsPath = Join-Path $env:USERPROFILE ".ssh\known_hosts"
$KnownHostsBackupPath = Join-Path $KeyDirectory "known_hosts.backup"
$KnownHostsExisted = $false
$SshDirectoryCreated = $false
$TestKeyCreated = $false

function Invoke-SshCompose {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    & docker compose --project-name $ProjectName --file $ComposeFile @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "SSH Docker Compose failed with exit code $LASTEXITCODE"
    }
}

function Assert-SshTools {
    foreach ($tool in @("docker", "ssh-keygen", "ssh-keyscan")) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "$tool command is required for the local SSH integration test"
        }
    }
    if (-not (Test-Path -LiteralPath $CargoWrapper -PathType Leaf)) {
        throw "Windows Cargo wrapper is missing: $CargoWrapper"
    }
}

function New-TestKey {
    New-Item -ItemType Directory -Path $KeyDirectory -Force | Out-Null
    $keygen = [Diagnostics.Process]::new()
    $keygen.StartInfo.FileName = (Get-Command ssh-keygen).Source
    $keygenArguments = @(
        "-q",
        "-t",
        "ed25519",
        "-N",
        "",
        "-C",
        "ramag-local-ssh-test",
        "-f",
        $PrivateKeyPath
    )
    $keygen.StartInfo.Arguments = ($keygenArguments | ForEach-Object {
        '"{0}"' -f $_.Replace('"', '\"')
    }) -join ' '
    $keygen.StartInfo.UseShellExecute = $false
    $null = $keygen.Start()
    $keygen.WaitForExit()
    if ($keygen.ExitCode -ne 0) {
        throw "ssh-keygen failed with exit code $($keygen.ExitCode)"
    }
    Copy-Item -LiteralPath $PublicKeyPath -Destination $AuthorizedKeysPath -Force
    $script:TestKeyCreated = $true
}

function Ensure-SshHealthy {
    Invoke-SshCompose -Arguments @("up", "--build", "--detach")
    for ($attempt = 1; $attempt -le 30; $attempt++) {
        $health = (& docker inspect --format "{{.State.Health.Status}}" $ContainerName 2>$null | Out-String).Trim()
        if ($health -eq "healthy") {
            Write-Host "[ssh-test] OpenSSH/SFTP fixture is healthy at $TestHost`:$TestPort"
            return
        }
        $state = (& docker inspect --format "{{.State.Status}}" $ContainerName 2>$null | Out-String).Trim()
        if ($state -eq "exited" -or $state -eq "dead") {
            $logs = (& docker logs $ContainerName 2>&1 | Out-String).Trim()
            throw "OpenSSH fixture stopped before becoming healthy.`n$logs"
        }
        Start-Sleep -Seconds 1
    }
    $logs = (& docker logs $ContainerName 2>&1 | Out-String).Trim()
    throw "OpenSSH fixture did not become healthy within 30 seconds.`n$logs"
}

function Save-KnownHosts {
    $sshDirectory = Split-Path -Parent $KnownHostsPath
    if (-not (Test-Path -LiteralPath $sshDirectory -PathType Container)) {
        New-Item -ItemType Directory -Path $sshDirectory -Force | Out-Null
        $script:SshDirectoryCreated = $true
    }
    if (Test-Path -LiteralPath $KnownHostsPath -PathType Leaf) {
        Copy-Item -LiteralPath $KnownHostsPath -Destination $KnownHostsBackupPath -Force
        $script:KnownHostsExisted = $true
    }
    if (-not (Test-Path -LiteralPath $KnownHostsPath -PathType Leaf)) {
        New-Item -ItemType File -Path $KnownHostsPath -Force | Out-Null
    }
    & ssh-keygen -q -R "[$TestHost]:$TestPort" -f $KnownHostsPath 2>$null | Out-Null
    $hostKey = @(& ssh-keyscan -T 5 -p $TestPort $TestHost 2>$null)
    if ($LASTEXITCODE -ne 0 -or $hostKey.Count -eq 0) {
        throw "ssh-keyscan did not return a host key for $TestHost`:$TestPort"
    }
    Add-Content -LiteralPath $KnownHostsPath -Value $hostKey
}

function Set-IntegrationEnvironment {
    $env:RAMAG_TEST_SSH_HOST = $TestHost
    $env:RAMAG_TEST_SSH_PORT = [string]$TestPort
    $env:RAMAG_TEST_SSH_USER = $TestUser
    $env:RAMAG_TEST_SSH_KEY_PATH = $PrivateKeyPath
    $env:RAMAG_TEST_SSH_ROOT = $TestRoot
}

function Clear-IntegrationEnvironment {
    foreach ($name in @(
        "RAMAG_TEST_SSH_HOST",
        "RAMAG_TEST_SSH_PORT",
        "RAMAG_TEST_SSH_USER",
        "RAMAG_TEST_SSH_KEY_PATH",
        "RAMAG_TEST_SSH_ROOT"
    )) {
        Remove-Item "Env:$name" -ErrorAction SilentlyContinue
    }
}

function Restore-KnownHosts {
    if ($KnownHostsExisted -and (Test-Path -LiteralPath $KnownHostsBackupPath -PathType Leaf)) {
        Copy-Item -LiteralPath $KnownHostsBackupPath -Destination $KnownHostsPath -Force
    } elseif (Test-Path -LiteralPath $KnownHostsPath -PathType Leaf) {
        Remove-Item -LiteralPath $KnownHostsPath -Force
    }
    if ($SshDirectoryCreated -and (Test-Path -LiteralPath (Split-Path -Parent $KnownHostsPath) -PathType Container)) {
        $children = @(Get-ChildItem -LiteralPath (Split-Path -Parent $KnownHostsPath) -Force)
        if ($children.Count -eq 0) {
            Remove-Item -LiteralPath (Split-Path -Parent $KnownHostsPath) -Force
        }
    }
}

function Remove-TestArtifacts {
    Clear-IntegrationEnvironment
    Restore-KnownHosts
    if ($TestKeyCreated -and (Test-Path -LiteralPath $AuthorizedKeysPath -PathType Leaf)) {
        Remove-Item -LiteralPath $AuthorizedKeysPath -Force
    }
    if (Test-Path -LiteralPath $KeyDirectory -PathType Container) {
        Remove-Item -LiteralPath $KeyDirectory -Recurse -Force
    }
}

function Run-IntegrationTest {
    New-TestKey
    Ensure-SshHealthy
    Save-KnownHosts
    Set-IntegrationEnvironment
    & powershell -NoProfile -ExecutionPolicy Bypass -File $CargoWrapper test --locked -p ramag-infra-ssh --test integration -- --nocapture
    if ($LASTEXITCODE -ne 0) {
        throw "OpenSSH/SFTP integration test failed with exit code $LASTEXITCODE"
    }
    Write-Host "[ssh-test] OpenSSH/SFTP Docker integration test passed."
}

try {
    Assert-SshTools
    switch ($Command) {
        "up" {
            New-TestKey
            Ensure-SshHealthy
        }
        "status" {
            Invoke-SshCompose -Arguments @("ps")
            Write-Host "[ssh-test] Endpoint: $TestHost`:$TestPort"
        }
        "test" {
            Run-IntegrationTest
        }
        "down" {
            Invoke-SshCompose -Arguments @("down")
        }
        "clean" {
            Invoke-SshCompose -Arguments @("down", "--volumes", "--remove-orphans")
        }
    }
} finally {
    if ($Command -eq "test") {
        try {
            Invoke-SshCompose -Arguments @("down", "--volumes", "--remove-orphans")
        } catch {
            Write-Warning "SSH Docker cleanup failed: $($_.Exception.Message)"
        }
    }
    Remove-TestArtifacts
}
