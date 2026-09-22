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
$ProxyContainerName = "ramag-api-grpc-proxy-test"
$TestHost = "127.0.0.1"
$TestPort = "18090"
$Endpoint = "http://{0}:{1}" -f $TestHost, $TestPort
$TlsEndpoint = "https://{0}:18092" -f $TestHost
$ProxyEndpoint = "http://{0}:18094" -f $TestHost
$ProxyUsername = "proxy-user"
$ProxyPassword = "proxy-secret"
$TlsDirectory = Join-Path $ScriptDirectory "tls"
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
        $proxyHealth = & docker inspect --format "{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}" $ProxyContainerName
        if ($LASTEXITCODE -eq 0 -and $health -eq "healthy" -and $proxyHealth -eq "healthy") {
            Write-Host "[api-grpc-test] gRPC test service is healthy: rust:1.91.0-bookworm at $Endpoint"
            Write-Host "[api-grpc-test] HTTP CONNECT proxy is healthy at $ProxyEndpoint"
            return
        }
        Start-Sleep -Seconds 1
    }
    throw "gRPC test service did not become healthy within 60 seconds"
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
        Write-Host "[api-grpc-test] mTLS endpoint: $TlsEndpoint"
        Write-Host "[api-grpc-test] proxy endpoint: $ProxyEndpoint"
    }
    "test" {
        Start-GrpcTest
        $env:RAMAG_TEST_API_GRPC_URL = $Endpoint
        $env:RAMAG_TEST_API_OAUTH2_TOKEN_URL = "http://127.0.0.1:18089/oauth/token"
        $env:RAMAG_TEST_API_OAUTH2_CLIENT_ID = "oauth-client"
        $env:RAMAG_TEST_API_OAUTH2_CLIENT_SECRET = "oauth-secret"
        $env:RAMAG_TEST_API_OAUTH2_SCOPE = "api.read"
        $env:RAMAG_TEST_API_GRPC_MTLS_URL = $TlsEndpoint
        $env:RAMAG_TEST_API_TLS_DIRECTORY = $TlsDirectory
        $env:RAMAG_TEST_API_GRPC_PROXY_URL = $ProxyEndpoint
        $env:RAMAG_TEST_API_PROXY_USERNAME = $ProxyUsername
        $env:RAMAG_TEST_API_PROXY_PASSWORD = $ProxyPassword
        & powershell -NoProfile -ExecutionPolicy Bypass -File $CargoWrapper test --locked -p ramag-infra-api --test docker_grpc
        if ($LASTEXITCODE -ne 0) {
            throw "API gRPC Docker integration test failed with exit code $LASTEXITCODE"
        }
        Write-Host "[api-grpc-test] Passed against local Docker gRPC test service; container remains running for reuse."
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
