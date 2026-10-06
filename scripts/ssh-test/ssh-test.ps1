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
$KnownHostsPath = Join-Path $KeyDirectory ".ssh\known_hosts"
$OriginalHome = $env:HOME
$HomeChanged = $false
$IntegrationEnvironment = @{}
$IsolatedSshPath = $null
$IsolatedKeyscanPath = $null
$TestKeyCreated = $false
$DockerLifecycleScript = Join-Path $RepositoryRoot "scripts\windows\docker-test-lifecycle.ps1"
$DockerTestContainers = @($ContainerName)

. $DockerLifecycleScript

function Invoke-SshCompose {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    & docker compose --project-name $ProjectName --file $ComposeFile @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "SSH Docker Compose failed with exit code $LASTEXITCODE"
    }
}

function Assert-SshTools {
    foreach ($tool in @("docker", "ssh-keygen", "git")) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "$tool command is required for the local SSH integration test"
        }
    }
    if (-not (Test-Path -LiteralPath $CargoWrapper -PathType Leaf)) {
        throw "Windows Cargo wrapper is missing: $CargoWrapper"
    }
    # Windows system OpenSSH ignores HOME for known_hosts. Git's OpenSSH uses
    # this process's isolated HOME, so the test never modifies the user's trust store.
    $gitRoot = Split-Path -Parent (Split-Path -Parent (Get-Command git).Source)
    $script:IsolatedSshPath = Join-Path $gitRoot "usr\bin\ssh.exe"
    $script:IsolatedKeyscanPath = Join-Path $gitRoot "usr\bin\ssh-keyscan.exe"
    foreach ($path in @($IsolatedSshPath, $IsolatedKeyscanPath)) {
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Git for Windows OpenSSH is required for isolated SSH testing: $path"
        }
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
    Repair-RamagDockerTestContainers `
        -ContainerNames $DockerTestContainers `
        -LogPrefix "ssh-test" | Out-Null
    for ($attemptNumber = 1; $attemptNumber -le 2; $attemptNumber++) {
        try {
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
        } catch {
            if ($attemptNumber -eq 2) {
                throw
            }
            Write-Warning "[ssh-test] OpenSSH fixture is unavailable; recreating its container before retrying."
            Recreate-RamagDockerTestContainers `
                -ContainerNames $DockerTestContainers `
                -LogPrefix "ssh-test"
        }
    }
}

function Save-IsolatedKnownHosts {
    $sshDirectory = Split-Path -Parent $KnownHostsPath
    New-Item -ItemType Directory -Path $sshDirectory -Force | Out-Null
    # Scan the loopback test endpoint only. PS5.1 treats native banner stderr
    # as a terminating error even when redirected; retain the exit-code check.
    $previousErrorAction = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $hostKey = @(& $IsolatedKeyscanPath -T 5 -p $TestPort $TestHost 2>$null)
        $hostKeyExitCode = $LASTEXITCODE
        if ($hostKeyExitCode -ne 0 -or $hostKey.Count -eq 0) {
            # Some Windows keyscan versions cannot negotiate the server KEX.
            # Read only this local test container's public host keys, then let
            # the integration client strictly verify the forwarded endpoint.
            $hostKey = @(& docker exec ramag-ssh-test ssh-keyscan -T 5 -p 22 127.0.0.1 2>$null)
            $hostKeyExitCode = $LASTEXITCODE
            $hostKey = @($hostKey | ForEach-Object {
                if ($_ -match '^\S+\s+(ssh-[\w-]+|ecdsa-[\w-]+)\s+([A-Za-z0-9+/=]+)$') {
                    "[$TestHost]:$TestPort $($Matches[1]) $($Matches[2])"
                }
            })
        }
    }
    finally {
        $ErrorActionPreference = $previousErrorAction
    }
    if ($hostKeyExitCode -ne 0 -or $hostKey.Count -eq 0) {
        throw "ssh-keyscan did not return a host key for $TestHost`:$TestPort"
    }
    [IO.File]::WriteAllLines($KnownHostsPath, [string[]]$hostKey, [Text.UTF8Encoding]::new($false))
}

function Set-IntegrationEnvironment {
    foreach ($name in @("RAMAG_TEST_SSH_HOST", "RAMAG_TEST_SSH_PORT", "RAMAG_TEST_SSH_USER", "RAMAG_TEST_SSH_KEY_PATH", "RAMAG_TEST_SSH_ROOT", "RAMAG_TEST_SSH_PATH")) {
        $IntegrationEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, "Process")
    }
    $normalizedHome = $KeyDirectory.Replace('\', '/')
    $env:HOME = '/{0}{1}' -f $normalizedHome.Substring(0, 1).ToLowerInvariant(), $normalizedHome.Substring(2)
    $script:HomeChanged = $true
    # Fail before Cargo if the selected executable does not honor the private HOME.
    $previousErrorAction = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $config = @(& $IsolatedSshPath -G -F /dev/null $TestHost 2>$null)
        $configExitCode = $LASTEXITCODE
    } finally {
        $ErrorActionPreference = $previousErrorAction
    }
    if ($configExitCode -ne 0 -or -not ($config -contains "userknownhostsfile $env:HOME/.ssh/known_hosts $env:HOME/.ssh/known_hosts2")) {
        throw "Selected OpenSSH does not use the isolated test known_hosts path"
    }
    $env:RAMAG_TEST_SSH_HOST = $TestHost
    $env:RAMAG_TEST_SSH_PORT = [string]$TestPort
    $env:RAMAG_TEST_SSH_USER = $TestUser
    $env:RAMAG_TEST_SSH_KEY_PATH = $PrivateKeyPath
    $env:RAMAG_TEST_SSH_ROOT = $TestRoot
    $env:RAMAG_TEST_SSH_PATH = $IsolatedSshPath
}

function Restore-IntegrationEnvironment {
    foreach ($name in $IntegrationEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $IntegrationEnvironment[$name], "Process")
    }
    if ($HomeChanged) {
        $env:HOME = $OriginalHome
    }
}

function Remove-TestArtifacts {
    Restore-IntegrationEnvironment
    if ($TestKeyCreated -and (Test-Path -LiteralPath $AuthorizedKeysPath -PathType Leaf)) {
        Remove-Item -LiteralPath $AuthorizedKeysPath -Force
    }
    if (Test-Path -LiteralPath $KeyDirectory -PathType Container) {
        $temporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        $cleanupPath = [IO.Path]::GetFullPath($KeyDirectory)
        if (-not $cleanupPath.StartsWith($temporaryRoot, [StringComparison]::OrdinalIgnoreCase) -or
            (Split-Path -Leaf $cleanupPath) -notmatch '^ramag-ssh-test-[0-9a-f]{32}$') {
            throw "Refusing SSH test cleanup outside its dedicated temporary directory"
        }
        Remove-Item -LiteralPath $KeyDirectory -Recurse -Force
    }
}

function Run-IntegrationTest {
    New-TestKey
    Ensure-SshHealthy
    Save-IsolatedKnownHosts
    Set-IntegrationEnvironment
    # The wrapper is an advanced PowerShell script: -p binds PipelineVariable.
    # Use Cargo's full --package spelling to keep this command scoped to SSH.
    & powershell -NoProfile -ExecutionPolicy Bypass -File $CargoWrapper test --locked --package ramag-infra-ssh --test integration
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
