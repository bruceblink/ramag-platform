# 安装器冒烟测试；运行前拒绝覆盖任何已有 Ramag 安装。
param(
    [Parameter(Mandatory = $true)]
    [string]$SetupPath,
    [Parameter(Mandatory = $true)]
    [string]$ExpectedVersion,
    [Parameter(Mandatory = $true)]
    [string]$WorkDir,
    [Parameter(Mandatory = $true)]
    [string]$AppId
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([System.Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
    throw "Installer smoke tests must run on Windows."
}
if ($AppId -notmatch '^[A-Za-z0-9._-]+$') {
    throw "Installer AppId contains unsupported characters: $AppId"
}

function Assert-File {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Description
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Description is missing: $Path"
    }
    if ((Get-Item -LiteralPath $Path).Length -eq 0) {
        throw "$Description is empty: $Path"
    }
}

function Invoke-InstallerProcess {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,
        [Parameter(Mandatory = $true)]
        [string[]]$ArgumentList,
        [Parameter(Mandatory = $true)]
        [string]$Description
    )

    $Process = Start-Process `
        -FilePath $FilePath `
        -ArgumentList $ArgumentList `
        -Wait `
        -PassThru
    if ($Process.ExitCode -ne 0) {
        throw "$Description failed with exit code $($Process.ExitCode)."
    }
}

function Assert-NoExistingInstallation {
    $Processes = @(Get-Process -Name "ramag" -ErrorAction SilentlyContinue)
    if ($Processes.Count -gt 0) {
        throw "Ramag is running. Close it before running the installer smoke test."
    }

    $RegistryPaths = @(
        "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\${AppId}_is1",
        "HKCU:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\${AppId}_is1",
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\${AppId}_is1",
        "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\${AppId}_is1"
    )
    $ExistingKeys = @($RegistryPaths | Where-Object { Test-Path -LiteralPath $_ })
    if ($ExistingKeys.Count -gt 0) {
        throw "An existing Ramag installation was found; smoke testing would replace it: $($ExistingKeys -join ', ')"
    }
}

function Write-StartupDiagnostics {
    param([string]$Executable, [datetime]$StartedAt)

    # Windows 发行版没有控制台。只在临时 CI 运行器上输出本次启动的应用日志。
    if ($env:GITHUB_ACTIONS -ne 'true') {
        return
    }
    $LogPaths = @(
        (Join-Path ([Environment]::GetFolderPath('ApplicationData')) 'ramag\ramag\data\logs\ramag.log'),
        (Join-Path ([System.IO.Path]::GetTempPath()) 'ramag-logs\ramag.log')
    )
    foreach ($LogPath in $LogPaths) {
        $Log = Get-Item -LiteralPath $LogPath -ErrorAction SilentlyContinue
        if ($null -ne $Log -and $Log.LastWriteTime -ge $StartedAt) {
            Get-Content -LiteralPath $LogPath -Tail 30 -ErrorAction Continue |
                ForEach-Object { Write-Host "Ramag startup: $_" }
        }
    }

    # 进程可能在创建应用日志前退出。系统记录可以提供异常代码和故障模块。
    Get-WinEvent -FilterHashtable @{
        LogName = 'Application'
        ProviderName = 'Application Error'
        Id = 1000
        StartTime = $StartedAt
        AppPath = $Executable
    } -MaxEvents 1 -ErrorAction SilentlyContinue | ForEach-Object {
        $Fields = ([xml]$_.ToXml()).Event.EventData.Data
        foreach ($Field in $Fields) {
            if ($Field.Name -in 'ExceptionCode', 'ModuleName', 'FaultingOffset') {
                Write-Host "Ramag startup event: $($Field.Name)=$($Field.InnerText)"
            }
        }
    }
}

Assert-File -Path $SetupPath -Description "Windows installer"
Assert-NoExistingInstallation
$SmokeDir = Join-Path $WorkDir "smoke"
$InstallDir = Join-Path $SmokeDir "install"
$InstallLog = Join-Path $SmokeDir "install.log"
$UninstallLog = Join-Path $SmokeDir "uninstall.log"
New-Item -ItemType Directory -Path $SmokeDir -Force | Out-Null

$Installed = $false
try {
    # 即使安装器中途失败，也尝试使用已生成的卸载器清理本地测试安装。
    $Installed = $true
    Invoke-InstallerProcess `
        -FilePath $SetupPath `
        -ArgumentList @(
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/SP-",
            "/DIR=`"$InstallDir`"",
            "/LOG=`"$InstallLog`""
        ) `
        -Description "Silent installation"
    $InstalledExe = Join-Path $InstallDir "ramag.exe"
    Assert-File -Path $InstalledExe -Description "Installed executable"
    Assert-File -Path (Join-Path $InstallDir "LICENSE") -Description "Installed license"
    $InstalledVersion = (
        [System.Diagnostics.FileVersionInfo]::GetVersionInfo($InstalledExe)
    ).ProductVersion
    if ($InstalledVersion -ne $ExpectedVersion) {
        throw "Installed version $InstalledVersion does not match Cargo version $ExpectedVersion."
    }

    $StartedAt = Get-Date
    $AppProcess = Start-Process -FilePath $InstalledExe -PassThru
    try {
        if ($AppProcess.WaitForExit(5000)) {
            Write-StartupDiagnostics -Executable $InstalledExe -StartedAt $StartedAt
            throw "Installed application exited during startup with code $($AppProcess.ExitCode)."
        }
    }
    finally {
        if (-not $AppProcess.HasExited) {
            Stop-Process -Id $AppProcess.Id -Force
            $AppProcess.WaitForExit()
        }
        $AppProcess.Dispose()
    }

    $Uninstallers = @(Get-ChildItem -LiteralPath $InstallDir -Filter "unins*.exe" -File)
    if ($Uninstallers.Count -ne 1) {
        throw "Expected exactly one uninstaller, found $($Uninstallers.Count)."
    }
    Invoke-InstallerProcess `
        -FilePath $Uninstallers[0].FullName `
        -ArgumentList @(
            "/VERYSILENT",
            "/SUPPRESSMSGBOXES",
            "/NORESTART",
            "/LOG=`"$UninstallLog`""
        ) `
        -Description "Silent uninstallation"
    $Installed = $false

    for ($Attempt = 0; $Attempt -lt 40; $Attempt++) {
        if (-not (Test-Path -LiteralPath $InstalledExe)) {
            break
        }
        Start-Sleep -Milliseconds 250
    }
    if (Test-Path -LiteralPath $InstalledExe) {
        throw "Silent uninstallation left the installed executable behind: $InstalledExe"
    }
    Assert-NoExistingInstallation
    Write-Host "Installer smoke test completed."
}
finally {
    if ($Installed) {
        $FallbackUninstaller = Get-ChildItem `
            -LiteralPath $InstallDir `
            -Filter "unins*.exe" `
            -File `
            -ErrorAction SilentlyContinue |
            Select-Object -First 1
        if ($null -ne $FallbackUninstaller) {
            try {
                Invoke-InstallerProcess `
                    -FilePath $FallbackUninstaller.FullName `
                    -ArgumentList @(
                        "/VERYSILENT",
                        "/SUPPRESSMSGBOXES",
                        "/NORESTART"
                    ) `
                    -Description "Smoke-test cleanup"
            }
            catch {
                Write-Warning "Smoke-test cleanup failed: $($_.Exception.Message)"
            }
        }
    }
}
