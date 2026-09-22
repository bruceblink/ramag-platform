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
$ProjectName = "api-test"
$ContainerName = "ramag-api-http-test"
$ProxyContainerName = "ramag-api-http-proxy-test"
$TestHost = "127.0.0.1"
$TestPort = "18089"
$Endpoint = "http://{0}:{1}" -f $TestHost, $TestPort
$TlsEndpoint = "https://{0}:18091" -f $TestHost
$ProxyEndpoint = "http://{0}:18093" -f $TestHost
$ProxyUsername = "proxy-user"
$ProxyPassword = "proxy-secret"
$TlsDirectory = Join-Path $ScriptDirectory "tls"
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
        $proxyHealth = & docker inspect --format "{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}" $ProxyContainerName
        if ($LASTEXITCODE -eq 0 -and $health -eq "healthy" -and $proxyHealth -eq "healthy") {
            Write-Host "[api-test] HTTP fixture is healthy: python:3.12.11-alpine3.22 at $Endpoint"
            Write-Host "[api-test] HTTP CONNECT proxy is healthy at $ProxyEndpoint"
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
        Write-Host "[api-test] mTLS endpoint: $TlsEndpoint"
        Write-Host "[api-test] proxy endpoint: $ProxyEndpoint"
    }
    "test" {
        Start-ApiTest
        $env:RAMAG_TEST_API_HTTP_URL = $Endpoint
        $env:RAMAG_TEST_API_OAUTH2_TOKEN_URL = "$Endpoint/oauth/token"
        $env:RAMAG_TEST_API_OAUTH2_CLIENT_ID = "oauth-client"
        $env:RAMAG_TEST_API_OAUTH2_CLIENT_SECRET = "oauth-secret"
        $env:RAMAG_TEST_API_OAUTH2_SCOPE = "api.read"
        $env:RAMAG_TEST_API_HTTP_MTLS_URL = $TlsEndpoint
        $env:RAMAG_TEST_API_TLS_DIRECTORY = $TlsDirectory
        $env:RAMAG_TEST_API_HTTP_PROXY_URL = $ProxyEndpoint
        $env:RAMAG_TEST_API_PROXY_USERNAME = $ProxyUsername
        $env:RAMAG_TEST_API_PROXY_PASSWORD = $ProxyPassword
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
