#requires -Version 7.2
<#
.SYNOPSIS
Runs Docker against the local Engine in a WSL distribution.
.DESCRIPTION
ConfigureStartup enables systemd Docker services and registers a current-user
logon task. The task keeps WSL running independently of the invoking terminal.
#>
[CmdletBinding(PositionalBinding = $false)]
param(
    [ValidateNotNullOrEmpty()]
    [string]$Distribution = 'Ubuntu-24.04',
    [switch]$ConfigureStartup,
    [switch]$StopKeepAlive,
    [Parameter(Position = 0, ValueFromRemainingArguments = $true)]
    [string[]]$DockerArguments = @()
)

$ErrorActionPreference = 'Stop'
if (-not $IsWindows) {
    throw 'Run this helper from PowerShell on Windows.'
}
if ($Distribution -cnotmatch '^[A-Za-z0-9][A-Za-z0-9._-]*$') {
    throw 'The WSL distribution name contains an unsupported character.'
}
if ($StopKeepAlive -and ($ConfigureStartup -or $DockerArguments.Count -gt 0)) {
    throw '-StopKeepAlive cannot be combined with other operations.'
}

$wslPath = Join-Path $env:WINDIR 'System32\wsl.exe'
$taskCommandInterpreter = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
$taskName = "Ramag-WSL-Docker-$Distribution"
$taskWorkingDirectory = Join-Path $env:WINDIR 'System32'
$startupCommand = "& '$wslPath' --distribution '$Distribution' --cd '/' --exec '/usr/bin/sleep' '2147483647' *> `$null; exit `$LASTEXITCODE"
$encodedStartupCommand = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($startupCommand))
$taskArguments = "-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand $encodedStartupCommand"
$currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()

# Verify ownership and action before changing an existing task. The scheduler's
# IgnoreNew policy prevents concurrent invocations from creating extra keepalives.
function Get-VerifiedStartupTask {
    $task = Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
    if ($null -eq $task) {
        return $null
    }
    $taskUser = $task.Principal.UserId
    if ($taskUser -notmatch '^S-1-') {
        $taskUser = [Security.Principal.NTAccount]::new($taskUser).
            Translate([Security.Principal.SecurityIdentifier]).Value
    }
    $validExecutables = @($taskCommandInterpreter)
    $validArguments = @($taskArguments)
    $validWorkingDirectories = @($taskWorkingDirectory)
    if ($ConfigureStartup) {
        $validExecutables += $wslPath
        $validArguments += @(
            "--distribution `"$Distribution`" --cd / --exec /usr/bin/sleep infinity",
            "--distribution $Distribution --cd / --exec /usr/bin/sleep infinity",
            "--distribution `"$Distribution`" --cd / --exec /usr/bin/sleep 2147483647",
            "--distribution $Distribution --cd / --exec /usr/bin/sleep 2147483647",
            "--distribution `"$Distribution`" --exec /usr/bin/sleep infinity",
            "--distribution $Distribution --exec /usr/bin/sleep infinity"
        )
        $validWorkingDirectories += $null
    }
    if ($taskUser -ne $currentIdentity.User.Value -or $task.Actions.Count -ne 1 -or
        $task.Actions[0].Execute -cnotin $validExecutables -or $task.Actions[0].Arguments -cnotin $validArguments -or
        $task.Actions[0].WorkingDirectory -cnotin $validWorkingDirectories) {
        throw "The existing task '$taskName' has a different owner or action."
    }
    return $task
}

$task = Get-VerifiedStartupTask
if ($StopKeepAlive) {
    if ($null -ne $task) {
        Stop-ScheduledTask -TaskName $taskName
        Write-Host "Stopped $taskName. Its next logon trigger remains enabled."
    }
    exit 0
}

if ($ConfigureStartup) {
    $registeredDistributions = @(& $wslPath --list --quiet 2>$null) |
        ForEach-Object { ($_ -replace '\x00', '').Trim() } |
        Where-Object { $_ }
    if ($LASTEXITCODE -ne 0 -or $Distribution -cnotin $registeredDistributions) {
        throw "The WSL distribution '$Distribution' is not registered."
    }
    & $wslPath --distribution $Distribution --user root --exec systemctl enable --now docker.service containerd.service
    if ($LASTEXITCODE -ne 0) {
        throw "Cannot enable Docker services in '$Distribution'. Check its Docker installation and systemd configuration."
    }
    $action = New-ScheduledTaskAction -Execute $taskCommandInterpreter -Argument $taskArguments `
        -WorkingDirectory $taskWorkingDirectory
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $currentIdentity.Name
    $principal = New-ScheduledTaskPrincipal -UserId $currentIdentity.User.Value -LogonType Interactive -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet -Hidden -StartWhenAvailable `
        -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew `
        -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) `
        -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
    $definition = New-ScheduledTask -Action $action -Trigger $trigger -Principal $principal `
        -Settings $settings -Description "Keep $Distribution running for the local systemd Docker Engine."
    Register-ScheduledTask -TaskName $taskName -InputObject $definition -Force | Out-Null
    $task = Get-VerifiedStartupTask
    Write-Host "Configured current-user logon startup: $taskName"
}
if ($null -eq $task) {
    throw 'Configure logon startup once with -ConfigureStartup before using this helper.'
}
if ($task.State -ne 'Running') {
    Start-ScheduledTask -TaskName $taskName
}

# Use the distribution's Unix socket explicitly. Desktop contexts cannot redirect
# the request. A bounded readiness check reports failure while preserving the task
# for diagnosis; -StopKeepAlive stops that task without deleting its definition.
$ready = $false
for ($attempt = 0; $attempt -lt 6; $attempt++) {
    & $wslPath --distribution $Distribution --exec /usr/bin/timeout 5 `
        docker --host unix:///var/run/docker.sock info --format '{{.ServerVersion}}' *> $null
    if ($LASTEXITCODE -eq 0) {
        $ready = $true
        break
    }
    Start-Sleep -Milliseconds 500
}
if (-not $ready) {
    throw "Docker Engine is unavailable in '$Distribution'. Check systemctl status docker inside WSL."
}
if ($DockerArguments.Count -eq 0) {
    $DockerArguments = @('info', '--format', '{{.ServerVersion}}')
}
& $wslPath --distribution $Distribution --exec docker --host unix:///var/run/docker.sock @DockerArguments
exit $LASTEXITCODE
