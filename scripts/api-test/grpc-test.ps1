[CmdletBinding()]
param(
    [ValidateSet("up", "status", "test", "down", "clean")]
    [string]$Command = "status"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepositoryRoot = Split-Path -Parent (Split-Path -Parent $ScriptDirectory)
$ComposeFile = Join-Path $ScriptDirectory "grpc-compose.yaml"
$ProjectName = "ramag-api-grpc-test"
$ContainerName = "ramag-api-grpc-test"
$TestHost = "127.0.0.1"
$TestPort = "18090"
$Endpoint = "http://{0}:{1}" -f $TestHost, $TestPort
$CargoWrapper = Join-Path $RepositoryRoot "scripts\windows\invoke-cargo-msvc.ps1"

function Invoke-GrpcCompose {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    & docker compose --project-name $ProjectName --file $ComposeFile @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose failed with exit code $LASTEXITCODE"
    }
}

function Wait-GrpcHealthy {
    for ($attempt = 1; $attempt -le 60; $attempt++) {
        $health = & docker inspect --format "{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}" $ContainerName
        if ($LASTEXITCODE -eq 0 -and $health -eq "healthy") {
            Write-Host "[api-grpc-test] gRPC fixture is healthy: rust:1.91.0-bookworm at $Endpoint"
            return
        }
        Start-Sleep -Seconds 1
    }
    throw "gRPC fixture did not become healthy within 60 seconds"
}

function Start-GrpcTest {
    Invoke-GrpcCompose -Arguments @("up", "--build", "--detach")
    Wait-GrpcHealthy
}

if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    throw "docker command is required"
}

switch ($Command) {
    "up" {
        Start-GrpcTest
    }
    "status" {
        Invoke-GrpcCompose -Arguments @("ps")
        Write-Host "[api-grpc-test] Endpoint: $Endpoint"
    }
    "test" {
        Start-GrpcTest
        $env:RAMAG_TEST_API_GRPC_URL = $Endpoint
        & powershell -NoProfile -ExecutionPolicy Bypass -File $CargoWrapper test --locked -p ramag-infra-api --test docker_grpc
        if ($LASTEXITCODE -ne 0) {
            throw "API gRPC Docker integration test failed with exit code $LASTEXITCODE"
        }
        Write-Host "[api-grpc-test] Passed against local Docker ramag-api-grpc-test; container remains running for reuse."
    }
    "down" {
        Invoke-GrpcCompose -Arguments @("down")
        Write-Host "[api-grpc-test] Stopped ramag-api-grpc-test."
    }
    "clean" {
        Invoke-GrpcCompose -Arguments @("down", "--volumes", "--remove-orphans")
        Write-Host "[api-grpc-test] Removed ramag-api-grpc-test resources."
    }
}
