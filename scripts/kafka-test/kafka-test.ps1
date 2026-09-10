[CmdletBinding()]
param(
    [ValidateSet("up", "status", "seed", "test", "down", "clean")]
    [string]$Command = "status",
    [ValidateRange(5000, 50000)]
    [int]$MessageCount = 5000
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ScriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$ComposeFile = Join-Path $ScriptDirectory "compose.yaml"
$DockerComposeFile = $ComposeFile
$FixtureFile = Join-Path $ScriptDirectory "fixtures\messages.txt"
$ToolchainScript = Join-Path $ScriptDirectory "..\windows\gnu-toolchain.ps1"
$ProjectName = "ramag-kafka-test"
$ContainerName = "ramag-kafka-test"
$ConnectContainerName = "ramag-kafka-connect-test"
$MetricsContainerName = "ramag-kafka-metrics-test"
$KsqlDbContainerName = "ramag-kafka-ksqldb-test"
$SchemaRegistryContainerName = "ramag-kafka-schema-registry-test"
$BootstrapServers = "127.0.0.1:19092"
$ConnectEndpoint = "http://127.0.0.1:18083"
$MetricsEndpoint = "http://127.0.0.1:19100/metrics"
$KsqlDbEndpoint = "http://127.0.0.1:18088"
$SchemaRegistryEndpoint = "http://127.0.0.1:18081"
$TopicName = "ramag.integration.messages"
$script:WslKeepAliveProcess = $null

$DockerCommand = Get-Command docker -ErrorAction SilentlyContinue
if ($null -eq $DockerCommand) {
    throw "docker command is required"
}

function ConvertTo-WslPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $resolvedPath = (Resolve-Path -LiteralPath $Path).Path
    if ($resolvedPath -notmatch "^(?<drive>[A-Za-z]):(?<tail>\\.*)$") {
        throw "Cannot convert the Windows path to a WSL path: $resolvedPath"
    }
    return "/mnt/{0}{1}" -f `
        $Matches.drive.ToLowerInvariant(),
        $Matches.tail.Replace("\", "/")
}

# The local docker command may be a PowerShell shim that forwards to WSL. In
# that case Docker Compose must receive a Linux path instead of a Windows path.
$UsesWslDockerShim = $DockerCommand.CommandType -eq "Alias" -and $DockerCommand.Definition -eq "docker-wsl"
if ($UsesWslDockerShim) {
    $DockerComposeFile = ConvertTo-WslPath -Path $ComposeFile
}

function Start-WslDockerKeepAlive {
    if (-not $UsesWslDockerShim -or $null -ne $script:WslKeepAliveProcess) {
        return
    }

    # Docker runs inside the local WSL distribution. Keep that distribution
    # alive for the duration of the test so its containers are not terminated
    # when an individual wsl.exe Docker client returns.
    $script:WslKeepAliveProcess = Start-Process `
        -FilePath "wsl.exe" `
        -ArgumentList @("--", "sleep", "3600") `
        -WindowStyle Hidden `
        -PassThru
    Start-Sleep -Milliseconds 250
    if ($script:WslKeepAliveProcess.HasExited) {
        $exitCode = $script:WslKeepAliveProcess.ExitCode
        $script:WslKeepAliveProcess = $null
        throw "Failed to keep the WSL Docker session alive; wsl.exe exited with code $exitCode"
    }
}

function Stop-WslDockerKeepAlive {
    if ($null -eq $script:WslKeepAliveProcess) {
        return
    }
    if (-not $script:WslKeepAliveProcess.HasExited) {
        Stop-Process -Id $script:WslKeepAliveProcess.Id -Force -ErrorAction SilentlyContinue
    }
    $script:WslKeepAliveProcess = $null
}

function Quote-BashLiteral {
    param([Parameter(Mandatory = $true)][string]$Value)

    return "'" + $Value.Replace("'", "'\''") + "'"
}

function ConvertTo-WslDockerArguments {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    if (-not $UsesWslDockerShim) {
        return $Arguments
    }

    # wsl.exe builds a shell command from positional arguments. Escape pipe
    # characters used by Kafka's key.separator property before forwarding it.
    return @($Arguments | ForEach-Object { $_.Replace("|", "\|") })
}

function Get-UiTopicDefinitions {
    # Keep a few semantic topics for visual cases, then add enough list rows to
    # exercise the left Topic pane, pagination, narrow widths, and its scrollbar.
    @(
        [pscustomobject]@{ Name = "ramag.ui.empty"; Partitions = 1 }
        [pscustomobject]@{ Name = "ramag.ui.short"; Partitions = 2 }
        [pscustomobject]@{ Name = "ramag.ui.partition-heavy"; Partitions = 12 }
        [pscustomobject]@{ Name = "ramag.ui.long-topic-name-for-responsive-layout-check"; Partitions = 3 }
    )
    for ($index = 1; $index -le 56; $index++) {
        [pscustomobject]@{
            Name = "ramag.ui.list-{0:D2}" -f $index
            Partitions = 1
        }
    }
}

function Get-FixtureTopicNames {
    @($TopicName)
    Get-UiTopicDefinitions | ForEach-Object { $_.Name }
}

if (-not (Test-Path -LiteralPath $ToolchainScript -PathType Leaf)) {
    throw "Windows GNU toolchain helper is missing: $ToolchainScript"
}

function Write-TestLog {
    param([Parameter(Mandatory = $true)][string]$Message)

    Write-Host "[kafka-test] $Message"
}

function Invoke-Compose {
    param([Parameter(Mandatory = $true)][string[]]$ComposeArguments)

    $dockerArguments = @(
        "compose", "--project-name", $ProjectName, "--file", $DockerComposeFile
    ) + $ComposeArguments
    & docker @(ConvertTo-WslDockerArguments -Arguments $dockerArguments)
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose failed with exit code $LASTEXITCODE"
    }
}

function Get-ComposeOutput {
    param([Parameter(Mandatory = $true)][string[]]$ComposeArguments)

    $stderrFile = [System.IO.Path]::GetTempFileName()
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = "Continue"
        $dockerArguments = @(
            "compose", "--project-name", $ProjectName, "--file", $DockerComposeFile
        ) + $ComposeArguments
        $output = @(& docker @(ConvertTo-WslDockerArguments -Arguments $dockerArguments) 2> $stderrFile)
        $exitCode = $LASTEXITCODE
        if ($exitCode -ne 0) {
            $stderr = Get-Content -LiteralPath $stderrFile -Raw
            throw "docker compose failed with exit code $exitCode`n$stderr"
        }
        return $output
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
        Remove-Item -LiteralPath $stderrFile -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-Kafka {
    param([Parameter(Mandatory = $true)][string[]]$KafkaArguments)

    Invoke-Compose -ComposeArguments (@("exec", "-T", "kafka") + $KafkaArguments)
}

function Get-KafkaOutput {
    param([Parameter(Mandatory = $true)][string[]]$KafkaArguments)

    return Get-ComposeOutput -ComposeArguments (@("exec", "-T", "kafka") + $KafkaArguments)
}

function Get-KafkaTopics {
    return @(Get-KafkaOutput -KafkaArguments @(
        "/opt/kafka/bin/kafka-topics.sh",
        "--bootstrap-server", "kafka:9092",
        "--list"
    ) | ForEach-Object { $_.ToString().Trim() })
}

function Wait-Healthy {
    for ($attempt = 1; $attempt -le 60; $attempt++) {
        $health = (& docker inspect --format "{{.State.Health.Status}}" $ContainerName 2>$null | Out-String).Trim()
        $state = (& docker inspect --format "{{.State.Status}}" $ContainerName 2>$null | Out-String).Trim()

        if ($health -eq "healthy") {
            Write-TestLog "Kafka container is healthy."
            return
        }
        if ($state -eq "exited" -or $state -eq "dead") {
            $logs = (& docker logs $ContainerName 2>&1 | Out-String).Trim()
            throw "Kafka container stopped before becoming healthy.`n$logs"
        }
        Start-Sleep -Seconds 2
    }

    $logs = (& docker logs $ContainerName 2>&1 | Out-String).Trim()
    throw "Kafka health check timed out.`n$logs"
}

function Wait-ConnectHealthy {
    for ($attempt = 1; $attempt -le 60; $attempt++) {
        $health = (& docker inspect --format "{{.State.Health.Status}}" $ConnectContainerName 2>$null | Out-String).Trim()
        $state = (& docker inspect --format "{{.State.Status}}" $ConnectContainerName 2>$null | Out-String).Trim()

        if ($health -eq "healthy") {
            Write-TestLog "Kafka Connect container is healthy."
            return
        }
        if ($state -eq "exited" -or $state -eq "dead") {
            $logs = (& docker logs $ConnectContainerName 2>&1 | Out-String).Trim()
            throw "Kafka Connect container stopped before becoming healthy.`n$logs"
        }
        Start-Sleep -Seconds 2
    }

    $logs = (& docker logs $ConnectContainerName 2>&1 | Out-String).Trim()
    throw "Kafka Connect health check timed out.`n$logs"
}

function Wait-MetricsHealthy {
    for ($attempt = 1; $attempt -le 60; $attempt++) {
        $health = (& docker inspect --format "{{.State.Health.Status}}" $MetricsContainerName 2>$null | Out-String).Trim()
        $state = (& docker inspect --format "{{.State.Status}}" $MetricsContainerName 2>$null | Out-String).Trim()

        if ($health -eq "healthy") {
            Write-TestLog "Kafka metrics fixture container is healthy."
            return
        }
        if ($state -eq "exited" -or $state -eq "dead") {
            $logs = (& docker logs $MetricsContainerName 2>&1 | Out-String).Trim()
            throw "Kafka metrics fixture container stopped before becoming healthy.`n$logs"
        }
        Start-Sleep -Seconds 2
    }

    $logs = (& docker logs $MetricsContainerName 2>&1 | Out-String).Trim()
    throw "Kafka metrics fixture health check timed out.`n$logs"
}

function Wait-KsqlDbHealthy {
    for ($attempt = 1; $attempt -le 90; $attempt++) {
        $health = (& docker inspect --format "{{.State.Health.Status}}" $KsqlDbContainerName 2>$null | Out-String).Trim()
        $state = (& docker inspect --format "{{.State.Status}}" $KsqlDbContainerName 2>$null | Out-String).Trim()

        if ($health -eq "healthy") {
            Write-TestLog "ksqlDB container is healthy."
            return
        }
        if ($state -eq "exited" -or $state -eq "dead") {
            $logs = (& docker logs $KsqlDbContainerName 2>&1 | Out-String -ErrorAction SilentlyContinue).Trim()
            throw "ksqlDB container stopped before becoming healthy.`n$logs"
        }
        Start-Sleep -Seconds 2
    }

    $logs = (& docker logs $KsqlDbContainerName 2>&1 | Out-String -ErrorAction SilentlyContinue).Trim()
    throw "ksqlDB health check timed out.`n$logs"
}

function Wait-SchemaRegistryHealthy {
    for ($attempt = 1; $attempt -le 90; $attempt++) {
        $health = (& docker inspect --format "{{.State.Health.Status}}" $SchemaRegistryContainerName 2>$null | Out-String).Trim()
        $state = (& docker inspect --format "{{.State.Status}}" $SchemaRegistryContainerName 2>$null | Out-String).Trim()

        if ($health -eq "healthy") {
            Write-TestLog "Schema Registry container is healthy."
            return
        }
        if ($state -eq "exited" -or $state -eq "dead") {
            $logs = (& docker logs $SchemaRegistryContainerName 2>&1 | Out-String -ErrorAction SilentlyContinue).Trim()
            throw "Schema Registry container stopped before becoming healthy.`n$logs"
        }
        Start-Sleep -Seconds 2
    }

    $logs = (& docker logs $SchemaRegistryContainerName 2>&1 | Out-String -ErrorAction SilentlyContinue).Trim()
    throw "Schema Registry health check timed out.`n$logs"
}

function Ensure-Healthy {
    Invoke-Compose -ComposeArguments @("up", "-d")
    Wait-Healthy
    Wait-ConnectHealthy
    Wait-MetricsHealthy
    Wait-KsqlDbHealthy
    Wait-SchemaRegistryHealthy
}

function Get-FixtureLines {
    # Keep the three semantic records stable, then add deterministic bulk data for
    # message pagination and bounded client-side search performance checks.
    Get-Content -LiteralPath $FixtureFile
    for ($index = 4; $index -le $MessageCount; $index++) {
        $payload = '{"event":"bulk","sequence":' + $index + ',"source":"docker-fixture","marker":"message-' + $index + '"}'
        '{0}|{1}' -f ("event-{0:D4}" -f $index), $payload
    }
}

function Reset-FixtureTopics {
    $topics = @(Get-KafkaTopics)
    foreach ($fixtureTopic in @(Get-FixtureTopicNames)) {
        if ($topics -notcontains $fixtureTopic) {
            continue
        }

        Invoke-Kafka -KafkaArguments @(
            "/opt/kafka/bin/kafka-topics.sh",
            "--bootstrap-server", "kafka:9092",
            "--delete",
            "--topic", $fixtureTopic
        )
        for ($attempt = 1; $attempt -le 30; $attempt++) {
            $topics = @(Get-KafkaTopics)
            if ($topics -notcontains $fixtureTopic) {
                break
            }
            Start-Sleep -Seconds 1
        }
        if ($topics -contains $fixtureTopic) {
            throw "Kafka fixture topic could not be deleted before reseeding: $fixtureTopic"
        }
    }
}

function New-FixtureTopic {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][int]$Partitions
    )

    Invoke-Kafka -KafkaArguments @(
        "/opt/kafka/bin/kafka-topics.sh",
        "--bootstrap-server", "kafka:9092",
        "--create", "--if-not-exists",
        "--topic", $Name,
        "--partitions", $Partitions,
        "--replication-factor", "1"
    )
}

function Invoke-KafkaProducer {
    $composeProducerArguments = @(
        "compose", "--project-name", $ProjectName, "--file", $DockerComposeFile,
        "exec", "-T", "kafka",
        "/opt/kafka/bin/kafka-console-producer.sh",
        "--bootstrap-server", "kafka:9092",
        "--topic", $TopicName,
        "--property", "parse.key=true",
        "--property", "key.separator=|",
        "--producer-property", "partitioner.class=org.apache.kafka.clients.producer.RoundRobinPartitioner"
    )
    $producerArguments = @(
        "docker", "compose",
        "--project-name", $ProjectName,
        "--file", $DockerComposeFile,
        "exec", "-T", "kafka",
        "/opt/kafka/bin/kafka-console-producer.sh",
        "--bootstrap-server", "kafka:9092",
        "--topic", $TopicName,
        "--property", "parse.key=true",
        "--property", "key.separator=|",
        "--producer-property", "partitioner.class=org.apache.kafka.clients.producer.RoundRobinPartitioner"
    )

    if (-not $UsesWslDockerShim) {
        Get-FixtureLines | & docker @composeProducerArguments
        if ($LASTEXITCODE -ne 0) {
            throw "Kafka fixture producer failed with exit code $LASTEXITCODE"
        }
        return
    }

    $fixtureTempFile = [System.IO.Path]::GetTempFileName()
    try {
        [System.IO.File]::WriteAllLines(
            $fixtureTempFile,
            [string[]]@(Get-FixtureLines),
            [System.Text.UTF8Encoding]::new($false)
        )
        $wslFixtureFile = ConvertTo-WslPath -Path $fixtureTempFile
        $producerCommand = (($producerArguments | ForEach-Object {
                Quote-BashLiteral -Value $_
            }) -join " ") + " < " + (Quote-BashLiteral -Value $wslFixtureFile)
        & wsl.exe -- bash -lc $producerCommand
        if ($LASTEXITCODE -ne 0) {
            throw "Kafka fixture producer failed with exit code $LASTEXITCODE"
        }
    } finally {
        Remove-Item -LiteralPath $fixtureTempFile -Force -ErrorAction SilentlyContinue
    }
}

function Seed-Fixture {
    Ensure-Healthy
    Reset-FixtureTopics
    New-FixtureTopic -Name $TopicName -Partitions 3
    foreach ($definition in @(Get-UiTopicDefinitions)) {
        New-FixtureTopic -Name $definition.Name -Partitions $definition.Partitions
    }

    Invoke-KafkaProducer
    Write-TestLog "Seeded $TopicName with $MessageCount deterministic messages."
    Write-TestLog "Created $(@(Get-UiTopicDefinitions).Count) additional UI topics with varied names and partition counts."
}

function Verify-Fixture {
    $consumerOutput = Get-KafkaOutput -KafkaArguments @(
        "/opt/kafka/bin/kafka-console-consumer.sh",
        "--bootstrap-server", "kafka:9092",
        "--topic", $TopicName,
        "--from-beginning",
        "--timeout-ms", "5000",
        "--property", "print.key=true",
        "--property", "key.separator=|"
    )
    $receivedLines = @($consumerOutput | ForEach-Object { $_.ToString().Trim() })

    $expectedLines = @(Get-FixtureLines)
    if ($receivedLines.Count -lt $MessageCount) {
        throw "Kafka fixture verification failed; expected at least $MessageCount records, received $($receivedLines.Count)"
    }
    $sentinelLines = @(
        $expectedLines[0],
        $expectedLines[1],
        $expectedLines[2],
        $expectedLines[$expectedLines.Count - 1]
    )
    foreach ($expectedLine in $sentinelLines) {
        if ($receivedLines -notcontains $expectedLine) {
            throw "Kafka fixture verification failed; missing record: $expectedLine`nReceived:`n$($receivedLines -join "`n")"
        }
    }
    Write-TestLog "Verified all $MessageCount fixture records in $TopicName."

    $availableTopics = @(Get-KafkaTopics)
    $missingTopics = @(Get-FixtureTopicNames | Where-Object { $availableTopics -notcontains $_ })
    if ($missingTopics.Count -gt 0) {
        throw "Kafka UI fixture verification failed; missing topics: $($missingTopics -join ", ")"
    }
    Write-TestLog "Verified $(@(Get-FixtureTopicNames).Count) topics for UI layout and scrollbar coverage."
}

function Invoke-KsqlDbStatement {
    param([Parameter(Mandatory = $true)][string]$Statement)

    $request = @{
        ksql             = $Statement
        streamsProperties = @{
            "ksql.streams.auto.offset.reset" = "earliest"
        }
    } | ConvertTo-Json -Compress
    try {
        return Invoke-RestMethod `
            -Uri "$KsqlDbEndpoint/ksql" `
            -Method Post `
            -ContentType "application/vnd.ksql.v1+json" `
            -Body $request
    } catch {
        throw "ksqlDB fixture statement failed: $Statement`n$($_.Exception.Message)"
    }
}

function Prepare-KsqlDbFixture {
    Invoke-KsqlDbStatement -Statement "DROP STREAM IF EXISTS RAMAG_INTEGRATION_STREAM;" | Out-Null
    Invoke-KsqlDbStatement -Statement @"
CREATE STREAM RAMAG_INTEGRATION_STREAM (
    EVENT VARCHAR,
    SEQUENCE BIGINT,
    SOURCE VARCHAR
) WITH (
    KAFKA_TOPIC='ramag.integration.messages',
    VALUE_FORMAT='JSON'
);
"@ | Out-Null
    Write-TestLog "Prepared ksqlDB stream RAMAG_INTEGRATION_STREAM over the seeded Kafka topic."
}

function Prepare-SchemaRegistryFixture {
    $subject = "ramag.integration.orders-value"
    $compatibility = @{ compatibility = "NONE" } | ConvertTo-Json -Compress
    Invoke-RestMethod `
        -Uri "$SchemaRegistryEndpoint/config/$subject" `
        -Method Put `
        -ContentType "application/vnd.schemaregistry.v1+json" `
        -Body $compatibility | Out-Null
    $schemaV1 = @{ schemaType = "JSON"; schema = '{"type":"object","properties":{"id":{"type":"integer"}}}' } | ConvertTo-Json -Compress
    $schemaV2 = @{ schemaType = "JSON"; schema = '{"type":"object","properties":{"id":{"type":"integer"},"status":{"type":"string"}}}' } | ConvertTo-Json -Compress
    foreach ($schema in @($schemaV1, $schemaV2)) {
        Invoke-RestMethod `
            -Uri "$SchemaRegistryEndpoint/subjects/$subject/versions" `
            -Method Post `
            -ContentType "application/vnd.schemaregistry.v1+json" `
            -Body $schema | Out-Null
    }
    Write-TestLog "Registered two Schema Registry versions for $subject."
}

function Run-RustIntegrationTest {
    # Run the Docker-backed Rust test with the same direct Cargo command used on
    # Linux and macOS, preferring GNU and falling back to the Windows MSVC
    # environment when the GNU prerequisites are unavailable.
    . $ToolchainScript
    $Toolchain = Select-WindowsToolchain -PreferGnu
    $EnvironmentSnapshot = $Toolchain.PreviousEnvironment
    $oldBootstrap = [Environment]::GetEnvironmentVariable("RAMAG_TEST_KAFKA_BOOTSTRAP", "Process")
    $oldConnectEndpoint = [Environment]::GetEnvironmentVariable("RAMAG_TEST_KAFKA_CONNECT", "Process")
    $oldMetricsEndpoint = [Environment]::GetEnvironmentVariable("RAMAG_TEST_KAFKA_METRICS", "Process")
    $oldKsqlDbEndpoint = [Environment]::GetEnvironmentVariable("RAMAG_TEST_KSQLDB", "Process")
    $oldSchemaRegistryEndpoint = [Environment]::GetEnvironmentVariable("RAMAG_TEST_SCHEMA_REGISTRY", "Process")
    $oldTargetDirectory = [Environment]::GetEnvironmentVariable("CARGO_TARGET_DIR", "Process")
    $env:RAMAG_TEST_KAFKA_BOOTSTRAP = $BootstrapServers
    $env:RAMAG_TEST_KAFKA_CONNECT = $ConnectEndpoint
    $env:RAMAG_TEST_KAFKA_METRICS = $MetricsEndpoint
    $env:RAMAG_TEST_KSQLDB = $KsqlDbEndpoint
    $env:RAMAG_TEST_SCHEMA_REGISTRY = $SchemaRegistryEndpoint
    $env:CARGO_TARGET_DIR = Join-Path ([System.IO.Path]::GetTempPath()) "ramag-kafka-docker-target"

    try {
        & cargo test --offline --locked -p ramag-infra-kafka --no-default-features --features cmake-build --test docker_kafka
        if ($LASTEXITCODE -ne 0) {
            throw "Rust Kafka integration test failed with exit code $LASTEXITCODE"
        }
    } finally {
        Restore-WindowsGnuEnvironment -Snapshot $EnvironmentSnapshot
        if ($null -eq $oldBootstrap) {
            Remove-Item Env:RAMAG_TEST_KAFKA_BOOTSTRAP -ErrorAction SilentlyContinue
        } else {
            $env:RAMAG_TEST_KAFKA_BOOTSTRAP = $oldBootstrap
        }
        if ($null -eq $oldConnectEndpoint) {
            Remove-Item Env:RAMAG_TEST_KAFKA_CONNECT -ErrorAction SilentlyContinue
        } else {
            $env:RAMAG_TEST_KAFKA_CONNECT = $oldConnectEndpoint
        }
        if ($null -eq $oldMetricsEndpoint) {
            Remove-Item Env:RAMAG_TEST_KAFKA_METRICS -ErrorAction SilentlyContinue
        } else {
            $env:RAMAG_TEST_KAFKA_METRICS = $oldMetricsEndpoint
        }
        if ($null -eq $oldKsqlDbEndpoint) {
            Remove-Item Env:RAMAG_TEST_KSQLDB -ErrorAction SilentlyContinue
        } else {
            $env:RAMAG_TEST_KSQLDB = $oldKsqlDbEndpoint
        }
        if ($null -eq $oldSchemaRegistryEndpoint) {
            Remove-Item Env:RAMAG_TEST_SCHEMA_REGISTRY -ErrorAction SilentlyContinue
        } else {
            $env:RAMAG_TEST_SCHEMA_REGISTRY = $oldSchemaRegistryEndpoint
        }
        if ($null -eq $oldTargetDirectory) {
            Remove-Item Env:CARGO_TARGET_DIR -ErrorAction SilentlyContinue
        } else {
            $env:CARGO_TARGET_DIR = $oldTargetDirectory
        }
    }
}

function Run-IntegrationTest {
    Seed-Fixture
    Verify-Fixture
    Prepare-KsqlDbFixture
    Prepare-SchemaRegistryFixture
    Run-RustIntegrationTest
    Write-TestLog "Docker Kafka integration test passed."
}

try {
    Start-WslDockerKeepAlive
    switch ($Command) {
        "up" {
            Ensure-Healthy
        }
        "status" {
            Invoke-Compose -ComposeArguments @("ps")
        }
        "seed" {
            Seed-Fixture
        }
        "test" {
            Run-IntegrationTest
        }
        "down" {
            Invoke-Compose -ComposeArguments @("down", "--remove-orphans")
        }
        "clean" {
            Invoke-Compose -ComposeArguments @("down", "--volumes", "--remove-orphans")
            Write-TestLog "Removed the dedicated Kafka test container, network, and volume."
        }
    }
} finally {
    Stop-WslDockerKeepAlive
}
