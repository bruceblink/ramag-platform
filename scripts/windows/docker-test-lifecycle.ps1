# Shared lifecycle helpers for local Docker integration-test fixtures.
# These functions deliberately touch only the container names supplied by the
# caller; they never inspect or remove unrelated Docker workloads.

function Get-RamagDockerTestContainerState {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ContainerName
    )

    $inspection = @(
        & docker inspect --format "{{.State.Status}}|{{if .State.Health}}{{.State.Health.Status}}{{end}}" `
            $ContainerName 2>$null
    )
    if ($LASTEXITCODE -ne 0 -or $inspection.Count -eq 0) {
        return $null
    }

    $parts = $inspection[0].ToString().Trim().Split("|", 2)
    [pscustomobject]@{
        Name   = $ContainerName
        State  = $parts[0]
        Health = if ($parts.Count -gt 1) { $parts[1] } else { "" }
    }
}

function Test-RamagDockerTestContainerUnavailable {
    param(
        [Parameter(Mandatory = $true)]
        [pscustomobject]$Inspection
    )

    if ($Inspection.State -ne "running") {
        return $true
    }

    return $Inspection.Health -eq "unhealthy"
}

function Stop-RemoveRamagDockerTestContainers {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$ContainerNames,
        [Parameter(Mandatory = $true)]
        [string]$LogPrefix
    )

    foreach ($containerName in $ContainerNames) {
        $inspection = Get-RamagDockerTestContainerState -ContainerName $containerName
        if ($null -eq $inspection) {
            continue
        }

        Write-Host ("[{0}] Stopping and removing unavailable Docker test container {1} (state={2}, health={3})." -f `
                $LogPrefix, $containerName, $inspection.State, `
                $(if ($inspection.Health) { $inspection.Health } else { "none" }))

        & docker stop --time 10 $containerName *> $null
        & docker rm --force $containerName *> $null
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to remove Docker test container: $containerName"
        }
    }
}

function Repair-RamagDockerTestContainers {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$ContainerNames,
        [Parameter(Mandatory = $true)]
        [string]$LogPrefix
    )

    $unavailable = @(
        foreach ($containerName in $ContainerNames) {
            $inspection = Get-RamagDockerTestContainerState -ContainerName $containerName
            if ($null -ne $inspection -and (Test-RamagDockerTestContainerUnavailable -Inspection $inspection)) {
                $containerName
            }
        }
    )

    if ($unavailable.Count -eq 0) {
        return $false
    }

    Stop-RemoveRamagDockerTestContainers -ContainerNames $unavailable -LogPrefix $LogPrefix
    return $true
}

function Recreate-RamagDockerTestContainers {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$ContainerNames,
        [Parameter(Mandatory = $true)]
        [string]$LogPrefix
    )

    Stop-RemoveRamagDockerTestContainers -ContainerNames $ContainerNames -LogPrefix $LogPrefix
}
