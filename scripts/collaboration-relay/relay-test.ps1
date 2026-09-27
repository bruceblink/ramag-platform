$ErrorActionPreference = 'Stop'

$compose = Join-Path $PSScriptRoot 'compose.yaml'
$projectName = 'ramag-collaboration-relay-test'

docker compose -p $projectName -f $compose up --build -d
$previousRelayUrl = $env:RAMAG_COLLAB_RELAY_URL
try {
    $deadline = (Get-Date).AddMinutes(5)
    do {
        try {
            $health = Invoke-WebRequest -UseBasicParsing -Uri 'http://127.0.0.1:18080/health' -TimeoutSec 2
            if ($health.StatusCode -eq 200 -and $health.Content.Trim() -eq 'ok') { break }
        } catch {
            # The container is still starting; poll until the bounded deadline.
        }
        if ((Get-Date) -gt $deadline) { throw 'Relay Docker 服务启动超时' }
        Start-Sleep -Seconds 2
    } while ($true)

    $env:RAMAG_COLLAB_RELAY_URL = 'http://127.0.0.1:18080'
    cargo test -p ramag-infra-collaboration
    Write-Output 'Relay Docker service health and client publish/fetch smoke passed on 127.0.0.1:18080.'
}
finally {
    if ($null -eq $previousRelayUrl) {
        Remove-Item Env:RAMAG_COLLAB_RELAY_URL -ErrorAction SilentlyContinue
    } else {
        $env:RAMAG_COLLAB_RELAY_URL = $previousRelayUrl
    }
    docker compose -p $projectName -f $compose down --volumes --remove-orphans
}
