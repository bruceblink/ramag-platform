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
$ProjectName = "ramag-api-http-test"
$ContainerName = "ramag-api-http-test"
$TestHost = "127.0.0.1"
$TestPort = "18089"
$Endpoint = "http://{0}:{1}" -f $TestHost, $TestPort
$CargoWrapper = Join-Path $RepositoryRoot "scripts\windows\invoke-cargo-msvc.ps1"

function Invoke-ApiCompose {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    & docker compose --project-name $ProjectName --file $ComposeFile @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose failed with exit code $LASTEXITCODE"
    }
}

function Wait-ApiHealthy {
    for ($attempt = 1; $attempt -le 30; $attempt++) {
        $health = & docker inspect --format "{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}" $ContainerName
        if ($LASTEXITCODE -eq 0 -and $health -eq "healthy") {
            Write-Host "[api-test] HTTP fixture is healthy: python:3.12.11-alpine3.22 at $Endpoint"
            return
        }
        Start-Sleep -Seconds 1
    }
    throw "HTTP fixture did not become healthy within 30 seconds"
}

function Start-ApiTest {
    Invoke-ApiCompose -Arguments @("up", "--build", "--detach")
    Wait-ApiHealthy
}

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "docker command is required"
}

switch ($Command) {
    "up" {
        Start-ApiTest
    }
    "status" {
        Invoke-ApiCompose -Arguments @("ps")
        Write-Host "[api-test] Endpoint: $Endpoint"
    }
    "test" {
        Start-ApiTest
        $env:RAMAG_TEST_API_HTTP_URL = $Endpoint
        & powershell -NoProfile -ExecutionPolicy Bypass -File $CargoWrapper test --locked -p ramag-infra-api --test docker_http
        if ($LASTEXITCODE -ne 0) {
            throw "API HTTP Docker integration test failed with exit code $LASTEXITCODE"
        }
        Write-Host "[api-test] Passed against local Docker ramag-api-http-test; container remains running for reuse."
    }
    "down" {
        Invoke-ApiCompose -Arguments @("down")
        Write-Host "[api-test] Stopped ramag-api-http-test."
    }
    "clean" {
        Invoke-ApiCompose -Arguments @("down", "--volumes", "--remove-orphans")
        Write-Host "[api-test] Removed ramag-api-http-test resources."
    }
}
