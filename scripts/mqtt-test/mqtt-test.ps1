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
$ProjectName = "ramag-mqtt-test"
$TestHost = "127.0.0.1"
$TestPort = "18883"
$CargoWrapper = Join-Path $RepositoryRoot "scripts\windows\invoke-cargo-msvc.ps1"

function Invoke-MqttCompose {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    & docker compose --project-name $ProjectName --file $ComposeFile @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose failed with exit code $LASTEXITCODE"
    }
}

function Wait-MqttHealthy {
    for ($attempt = 1; $attempt -le 30; $attempt++) {
        $health = & docker inspect --format "{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}" ramag-mqtt-test
        if ($LASTEXITCODE -eq 0 -and $health -eq "healthy") {
            Write-Host "[mqtt-test] Mosquitto is healthy: eclipse-mosquitto:2.0.20 at $TestHost port $TestPort"
            return
        }
        Start-Sleep -Seconds 1
    }
    throw "Mosquitto did not become healthy within 30 seconds"
}

function Start-MqttTest {
    Invoke-MqttCompose -Arguments @("up", "--detach")
    Wait-MqttHealthy
}

switch ($Command) {
    "up" {
        Start-MqttTest
    }
    "status" {
        Invoke-MqttCompose -Arguments @("ps")
        Write-Host ("[mqtt-test] Endpoint: mqtt://{0}:{1}" -f $TestHost, $TestPort)
    }
    "test" {
        Start-MqttTest
        $env:RAMAG_TEST_MQTT_HOST = $TestHost
        $env:RAMAG_TEST_MQTT_PORT = $TestPort
        & powershell -NoProfile -ExecutionPolicy Bypass -File $CargoWrapper test --locked -p ramag-infra-mqtt --features native --test docker_mqtt
        if ($LASTEXITCODE -ne 0) {
            throw "MQTT Docker integration test failed with exit code $LASTEXITCODE"
        }
        Write-Host "[mqtt-test] Passed against local Docker ramag-mqtt-test; retained test messages are cleared by the integration test and the container remains running for reuse."
    }
    "down" {
        Invoke-MqttCompose -Arguments @("down")
        Write-Host "[mqtt-test] Stopped ramag-mqtt-test; no named volume is retained."
    }
    "clean" {
        Invoke-MqttCompose -Arguments @("down", "--volumes", "--remove-orphans")
        Write-Host "[mqtt-test] Removed ramag-mqtt-test resources; no named volume is retained."
    }
}
