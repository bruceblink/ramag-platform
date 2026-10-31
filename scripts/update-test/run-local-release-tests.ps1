[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$TempBase = [System.IO.Path]::GetFullPath($env:TEMP).TrimEnd('\')
$TestRoot = Join-Path $TempBase ("ramag-local-release-" + [guid]::NewGuid().ToString("N"))
$ContainerName = "ramag-local-release-" + [guid]::NewGuid().ToString("N")
$Image = "nginx:1.27-alpine"
$ContainerStarted = $false
$PreviousRoot = [Environment]::GetEnvironmentVariable("RAMAG_UPDATE_TEST_ROOT", "Process")
$PreviousPort = [Environment]::GetEnvironmentVariable("RAMAG_UPDATE_TEST_PORT", "Process")

function Invoke-WslDocker {
    param([Parameter(Mandatory)][string[]]$Arguments)

    & wsl.exe -d Ubuntu-24.04 -- docker @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "WSL Docker command failed (exit $LASTEXITCODE): docker $($Arguments -join ' ')"
    }
}

function ConvertTo-WslPath {
    param([Parameter(Mandatory)][string]$Path)

    $absolutePath = [System.IO.Path]::GetFullPath($Path)
    if ($absolutePath -notmatch '^([A-Za-z]):\\(.*)$') {
        throw "The fixture path is not on a Windows drive: $absolutePath"
    }
    $drive = $Matches[1].ToLowerInvariant()
    $tail = $Matches[2].Replace('\', '/')
    return "/mnt/$drive/$tail"
}

function Get-FreeLoopbackPort {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try {
        return ([System.Net.IPEndPoint]$listener.LocalEndpoint).Port
    }
    finally {
        $listener.Stop()
    }
}

function Get-TestNginxImageId {
    $previousErrorAction = $ErrorActionPreference
    try {
        # Windows PowerShell treats redirected native stderr as an error record.
        # Probe failure is expected for a missing image; inspect the exit code.
        $ErrorActionPreference = "Continue"
        $output = @(& wsl.exe -d Ubuntu-24.04 -- docker image inspect $Image --format "{{.Id}}" 2>$null)
        if ($LASTEXITCODE -eq 0) {
            return $output | Select-Object -First 1
        }
        return $null
    }
    finally {
        $ErrorActionPreference = $previousErrorAction
    }
}

try {
    New-Item -ItemType Directory -Path $TestRoot | Out-Null
    $htmlRoot = Join-Path $TestRoot "html"
    $logRoot = Join-Path $TestRoot "logs"
    $certRoot = Join-Path $TestRoot "certs"
    $confRoot = Join-Path $TestRoot "conf"
    New-Item -ItemType Directory -Path $htmlRoot, $logRoot, $certRoot, $confRoot | Out-Null

    $certPath = Join-Path $certRoot "github.crt"
    $keyPath = Join-Path $certRoot "github.key"
    & wsl.exe -d Ubuntu-24.04 -- openssl req -x509 -newkey rsa:2048 -nodes -days 1 `
        -keyout (ConvertTo-WslPath $keyPath) -out (ConvertTo-WslPath $certPath) -subj "/CN=github.com" `
        -addext "subjectAltName=DNS:github.com"
    if ($LASTEXITCODE -ne 0) {
        throw "WSL OpenSSL failed to create the temporary test certificate."
    }

    $nginxConfig = @'
server {
    listen 443 ssl;
    server_name github.com;

    ssl_certificate /etc/nginx/certs/github.crt;
    ssl_certificate_key /etc/nginx/certs/github.key;
    access_log /var/log/nginx/access.log combined;
    error_log /var/log/nginx/error.log warn;

    location = /health {
        default_type text/plain;
        return 200 "ok\n";
    }

    location = /bruceblink/ramag-platform/releases/latest {
        return 302 https://github.com/bruceblink/ramag-platform/releases/tag/v1.1.0;
    }

    location = /bruceblink/ramag-platform/releases/tag/v1.1.0 {
        default_type text/plain;
        return 200 "v1.1.0\n";
    }

    location / {
        root /usr/share/nginx/html;
        try_files $uri =404;
    }
}
'@
    $nginxConfig = $nginxConfig.Replace("`r`n", "`n")
    [System.IO.File]::WriteAllText(
        (Join-Path $confRoot "default.conf"),
        $nginxConfig,
        [System.Text.UTF8Encoding]::new($false)
    )

    $port = Get-FreeLoopbackPort
    $env:RAMAG_UPDATE_TEST_ROOT = $TestRoot
    $env:RAMAG_UPDATE_TEST_PORT = "$port"
    Write-Host "Fixture root: $TestRoot"
    Write-Host "HTTPS loopback port: 127.0.0.1:$port"
    Write-Host "Nginx image: $Image"

    $imageId = Get-TestNginxImageId
    if (-not $imageId) {
        Write-Host "Image is not cached; pulling the requested Nginx image."
        Invoke-WslDocker -Arguments @("pull", $Image)
        $imageId = Get-TestNginxImageId
        if (-not $imageId) {
            throw "Could not inspect the pulled Nginx image."
        }
    }
    Write-Host "Nginx image ID: $imageId"

    $mounts = @(
        "type=bind,source=$(ConvertTo-WslPath $htmlRoot),target=/usr/share/nginx/html,readonly",
        "type=bind,source=$(ConvertTo-WslPath $logRoot),target=/var/log/nginx",
        "type=bind,source=$(ConvertTo-WslPath $confRoot)/default.conf,target=/etc/nginx/conf.d/default.conf,readonly",
        "type=bind,source=$(ConvertTo-WslPath $certRoot),target=/etc/nginx/certs,readonly"
    )
    $dockerArgs = @("run", "--detach", "--name", $ContainerName, "--publish", "127.0.0.1:${port}:443")
    foreach ($mount in $mounts) {
        $dockerArgs += @("--mount", $mount)
    }
    $dockerArgs += $Image
    Invoke-WslDocker -Arguments $dockerArgs
    $ContainerStarted = $true

    $healthy = $false
    $previousErrorAction = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        for ($attempt = 0; $attempt -lt 60; $attempt++) {
            $healthBody = & curl.exe --noproxy "*" --insecure --silent --show-error `
                "https://127.0.0.1:$port/health" 2>$null
            if ($LASTEXITCODE -eq 0 -and ($healthBody -join "").Trim() -eq "ok") {
                $healthy = $true
                break
            }
            Start-Sleep -Milliseconds 250
        }
    }
    finally {
        $ErrorActionPreference = $previousErrorAction
    }
    if (-not $healthy) {
        Invoke-WslDocker -Arguments @("logs", $ContainerName)
        throw "The local HTTPS /health endpoint did not become healthy."
    }
    Write-Host "HTTPS /health returned ok."

    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    . (Join-Path $RepositoryRoot "scripts/windows/enable-msvc-toolchain.ps1") -ShowStatus
    rustup run stable rustc -Vv
    if ($LASTEXITCODE -ne 0) {
        throw "Could not verify the stable Rust toolchain."
    }

    Push-Location $RepositoryRoot
    try {
        cargo test --locked -p ramag-infra-update --lib local_release_tests -- --ignored --test-threads=1
        if ($LASTEXITCODE -ne 0) {
            throw "The local Release integration test failed (cargo exit $LASTEXITCODE)."
        }
    }
    finally {
        Pop-Location
    }
}
finally {
    [Environment]::SetEnvironmentVariable("RAMAG_UPDATE_TEST_ROOT", $PreviousRoot, "Process")
    [Environment]::SetEnvironmentVariable("RAMAG_UPDATE_TEST_PORT", $PreviousPort, "Process")

    if ($ContainerStarted) {
        & wsl.exe -d Ubuntu-24.04 -- docker rm --force $ContainerName
        if ($LASTEXITCODE -eq 0) {
            Write-Host "Removed test container: $ContainerName"
        }
        else {
            Write-Warning "Could not remove test container $ContainerName (exit $LASTEXITCODE)."
        }
    }

    if (Test-Path -LiteralPath $TestRoot) {
        $resolvedTestRoot = [System.IO.Path]::GetFullPath($TestRoot).TrimEnd('\')
        if (-not $resolvedTestRoot.StartsWith("$TempBase\", [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove a temporary path outside TEMP: $resolvedTestRoot"
        }
        Remove-Item -LiteralPath $resolvedTestRoot -Recurse -Force
        Write-Host "Removed temporary fixture directory: $resolvedTestRoot"
    }
}
