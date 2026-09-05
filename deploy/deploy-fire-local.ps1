# Fire-only Windows host. Start detached through Task Scheduler if the invoking
# shell belongs to a job that terminates its children when the shell exits.
# The host remains available only while this workstation is awake.
[CmdletBinding()]
param(
    [ValidateSet('up', 'status', 'down')][string]$Action = 'up',
    [ValidateRange(1024, 65535)][int]$Port = 7781,
    [string]$RunDirectory = (Join-Path $env:USERPROFILE '.ember/fire-local'),
    [string]$BuildDirectory = '',
    [string]$Cloudflared = (Join-Path $env:USERPROFILE 'tools/cloudflared.exe'),
    [string]$GitBash = 'C:/Program Files/Git/bin/bash.exe',
    [string]$Ref = 'HEAD',
    [switch]$SkipBuild,
    [switch]$NoPublish
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$totalClock = [Diagnostics.Stopwatch]::StartNew()
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$runRoot = [IO.Path]::GetFullPath($RunDirectory)
$bindAddress = "127.0.0.1:$Port"
$utf8 = [Text.UTF8Encoding]::new($false)

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 6) + "`n", $utf8)
}

function Read-Json([string]$Path) {
    if (Test-Path -LiteralPath $Path) { return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json }
    return $null
}

function Invoke-Git([string[]]$Arguments) {
    $output = & git -C $repository @Arguments
    if ($LASTEXITCODE -ne 0) { throw "git failed: $($Arguments -join ' ')" }
    return $output
}

function Assert-CleanTree {
    $changes = @(Invoke-Git @('status', '--porcelain', '--untracked-files=normal'))
    if ($changes.Count -gt 0) { throw "Commit all changes before deploying. This host only publishes committed builds.`n$($changes -join "`n")" }
}

function Get-OwnedProcess($Record) {
    if ($null -eq $Record) { return $null }
    $process = Get-Process -Id $Record.Id -ErrorAction SilentlyContinue
    if ($null -eq $process) { return $null }
    $details = Get-CimInstance Win32_Process -Filter "ProcessId=$($Record.Id)"
    if ($null -eq $details) { return $null }
    $samePath = [string]::Equals($process.Path, $Record.ExecutablePath, [StringComparison]::OrdinalIgnoreCase)
    $sameStart = $process.StartTime.ToUniversalTime().Ticks.ToString() -eq $Record.StartedUtcTicks
    $tokenPattern = '(^|[\s"])' + [regex]::Escape($Record.CommandToken) + '(?=$|[\s"])'
    $sameCommand = $null -ne $details.CommandLine -and [regex]::IsMatch($details.CommandLine, $tokenPattern)
    if (!$samePath -or !$sameStart -or !$sameCommand) {
        throw "PID $($Record.Id) no longer matches this Fire host's saved executable, creation time and command line. Refusing to touch it."
    }
    return $process
}

function Stop-OwnedProcess($Record) {
    $process = Get-OwnedProcess $Record
    if ($null -ne $process) {
        Stop-Process -Id $process.Id -Force
        $process.WaitForExit(10000) | Out-Null
    }
}

function Start-OwnedProcess([string]$Name, [string]$Executable, [string[]]$Arguments, [string]$Token, [switch]$ReturnProcess) {
    # All current arguments are simple validated host names, local addresses,
    # flags or commit ids. FilePath handles spaces without building shell code.
    $process = Start-Process -FilePath $Executable -ArgumentList $Arguments -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $runRoot "$Name.log") `
        -RedirectStandardError (Join-Path $runRoot "$Name.err.log")
    $record = [pscustomobject]@{
        Id = $process.Id
        ExecutablePath = [IO.Path]::GetFullPath($Executable)
        StartedUtcTicks = $process.StartTime.ToUniversalTime().Ticks.ToString()
        CommandToken = $Token
    }
    Write-Json (Join-Path $runRoot "$Name.process.json") $record
    if ($ReturnProcess) { return [pscustomobject]@{ Record = $record; Process = $process } }
    return $record
}

function Assert-PortOwnership($Record) {
    $owned = Get-OwnedProcess $Record
    $listeners = @(Get-NetTCPConnection -State Listen -LocalPort $Port -ErrorAction SilentlyContinue)
    foreach ($listener in $listeners) {
        if ($null -eq $owned -or $listener.OwningProcess -ne $owned.Id) {
            throw "Port $Port is held by PID $($listener.OwningProcess), which this Fire host does not own. Other games and services are left running."
        }
    }
}

function Invoke-Probe([string]$Executable, [string]$Url, [string[]]$Extra = @()) {
    $arguments = @($Url) + $Extra
    $started = Start-OwnedProcess 'probe' $Executable $arguments $Url -ReturnProcess
    $record = $started.Record
    $process = $started.Process
    # The native probe has a protocol deadline; this also bounds DNS/TLS setup.
    if (!$process.WaitForExit(25000)) {
        Stop-OwnedProcess $record
        Write-Host "Probe timed out: $Url"
        return 1
    }
    $exitCode = $process.ExitCode
    Get-Content -LiteralPath (Join-Path $runRoot 'probe.log') -ErrorAction SilentlyContinue | ForEach-Object { Write-Host $_ }
    if ($exitCode -ne 0) {
        Get-Content -LiteralPath (Join-Path $runRoot 'probe.err.log') -ErrorAction SilentlyContinue | ForEach-Object { Write-Host $_ }
    }
    return $exitCode
}

function Assert-EmptyServer([string]$Probe) {
    $code = Invoke-Probe $Probe "ws://$bindAddress" @('--require-empty')
    if ($code -eq 2) { throw 'People are racing on this Fire server. Wait until its lobbies are empty before restarting or stopping it.' }
    if ($code -ne 0) { throw 'The current Fire server did not prove it is empty. Refusing to disconnect possible players.' }
}

function Publish-Fire($Release) {
    if ($NoPublish) { Write-Host "Verified Fire endpoint: $($Release.Url). Address book not changed (-NoPublish)."; return }
    $remote = [string](Invoke-Git @('remote', 'get-url', 'origin'))
    & $GitBash (Join-Path $PSScriptRoot 'publish-host.sh') --repo $remote --branch gh-pages `
        --name $Release.Host --game fire --url $Release.Url --proto $Release.Protocol `
        --version $Release.Version --commit $Release.Commit --by "$env:USERNAME@$env:COMPUTERNAME"
    if ($LASTEXITCODE -ne 0) { throw 'Fire runs and passed health checks, but publishing the address book failed.' }
}

try {
    $serverRecord = Read-Json (Join-Path $runRoot 'server.process.json')
    $tunnelRecord = Read-Json (Join-Path $runRoot 'tunnel.process.json')
    $release = Read-Json (Join-Path $runRoot 'release.json')
    if ($Action -eq 'status') {
        foreach ($pair in @(@('server', $serverRecord), @('tunnel', $tunnelRecord))) {
            $process = Get-OwnedProcess $pair[1]
            Write-Host "$($pair[0]): $(if ($null -ne $process) { "PID $($process.Id), $($process.Path)" } else { 'not running' })"
        }
        if ($null -ne $release) { $release | Format-List | Out-Host }
        return
    }
    Assert-PortOwnership $serverRecord
    if ($Action -eq 'down') {
        if ($null -ne (Get-OwnedProcess $serverRecord)) {
            if ($null -eq $release -or !(Test-Path -LiteralPath $release.Probe)) { throw 'Cannot prove the existing server is empty: no saved Fire probe.' }
            Assert-EmptyServer $release.Probe
        }
        Stop-OwnedProcess $tunnelRecord
        Stop-OwnedProcess $serverRecord
        Write-Host 'Stopped only the saved Fire server and tunnel. The address book is unchanged.'
        return
    }

    Assert-CleanTree
    $commitFull = [string](Invoke-Git @('rev-parse', '--verify', "$Ref^{commit}"))
    $head = [string](Invoke-Git @('rev-parse', 'HEAD'))
    if ($commitFull -ne $head) { throw 'This script builds the checked-out commit. Check out the requested Ref in an isolated worktree first.' }
    $commit = [string](Invoke-Git @('rev-parse', '--short', $commitFull))
    $version = 'r' + [string](Invoke-Git @('rev-list', '--count', $commitFull))
    $protocolText = (Invoke-Git @('show', "${commitFull}:crates/fire-core/src/proto.rs")) -join "`n"
    if ($protocolText -notmatch 'PROTO_VERSION: u16 = (\d+)') { throw 'No Fire protocol version in committed source.' }
    $protocol = [int]$Matches[1]
    if (!(Test-Path -LiteralPath $Cloudflared)) { throw "Missing tunnel binary: $Cloudflared" }
    if (!(Test-Path -LiteralPath $GitBash)) { throw "Missing Git Bash: $GitBash" }
    $hostName = [string](& $GitBash (Join-Path $PSScriptRoot 'host-name.sh'))
    if ($LASTEXITCODE -ne 0 -or $hostName -notmatch '^[a-z0-9-]{3,32}$') { throw 'Could not resolve this workstation host name.' }
    New-Item -ItemType Directory -Path $runRoot -Force | Out-Null
    if (!$BuildDirectory) { $BuildDirectory = Join-Path $runRoot 'build' }
    $BuildDirectory = [IO.Path]::GetFullPath($BuildDirectory)
    if (!$SkipBuild) {
        $buildClock = [Diagnostics.Stopwatch]::StartNew()
        $previousVersion = $env:EMBER_BUILD_VERSION
        $previousCommit = $env:EMBER_BUILD_COMMIT
        $currentProcess = [Diagnostics.Process]::GetCurrentProcess()
        $previousPriority = $currentProcess.PriorityClass
        try {
            $currentProcess.PriorityClass = 'Idle'
            $env:EMBER_BUILD_VERSION = $version
            $env:EMBER_BUILD_COMMIT = $commit
            & cargo build --manifest-path (Join-Path $repository 'Cargo.toml') --target-dir $BuildDirectory --release -p fire-server --bin fire-server --example probe
            if ($LASTEXITCODE -ne 0) { throw 'Fire release build failed.' }
        } finally {
            $env:EMBER_BUILD_VERSION = $previousVersion
            $env:EMBER_BUILD_COMMIT = $previousCommit
            $currentProcess.PriorityClass = $previousPriority
            Write-Host ('Fire build wall time: {0:F1}s' -f $buildClock.Elapsed.TotalSeconds)
        }
    }
    Assert-CleanTree
    if ([string](Invoke-Git @('rev-parse', 'HEAD')) -ne $commitFull) { throw 'HEAD changed while building. Nothing will be restarted.' }
    $sourceServer = Join-Path $BuildDirectory 'release/fire-server.exe'
    $sourceProbe = Join-Path $BuildDirectory 'release/examples/probe.exe'
    foreach ($binary in @($sourceServer, $sourceProbe)) { if (!(Test-Path -LiteralPath $binary)) { throw "Missing Fire binary: $binary" } }
    $hash = (Get-FileHash -LiteralPath $sourceServer -Algorithm SHA256).Hash.ToLowerInvariant()
    $releaseDirectory = Join-Path $runRoot "releases/$commit-$($hash.Substring(0, 12))"
    New-Item -ItemType Directory -Path $releaseDirectory -Force | Out-Null
    $serverBinary = Join-Path $releaseDirectory 'fire-server.exe'
    $probeBinary = Join-Path $releaseDirectory 'probe.exe'
    if (!(Test-Path -LiteralPath $serverBinary)) { Copy-Item -LiteralPath $sourceServer -Destination $serverBinary }
    Copy-Item -LiteralPath $sourceProbe -Destination $probeBinary -Force

    # Even SkipBuild must prove the candidate's compile-time stamp before the
    # current service is touched. A binary name or filesystem date is no proof.
    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $candidatePort = $listener.LocalEndpoint.Port
    $listener.Stop()
    $candidateBind = "127.0.0.1:$candidatePort"
    $candidate = Start-OwnedProcess 'candidate' $serverBinary @($candidateBind, '--name', $hostName) $candidateBind
    try {
        Start-Sleep -Milliseconds 500
        if ((Invoke-Probe $probeBinary "ws://$candidateBind" @('--expect-commit', $commit)) -ne 0) {
            throw 'Candidate Fire server did not prove its protocol and committed build stamp. Existing service is unchanged.'
        }
    } finally { Stop-OwnedProcess $candidate }

    Assert-PortOwnership $serverRecord
    $currentServer = Get-OwnedProcess $serverRecord
    $sameServer = $null -ne $currentServer -and [string]::Equals($currentServer.Path, $serverBinary, [StringComparison]::OrdinalIgnoreCase)
    if (!$sameServer) {
        if ($null -ne $currentServer) { Assert-EmptyServer $probeBinary }
        Stop-OwnedProcess $serverRecord
        Assert-PortOwnership $null
        $serverRecord = Start-OwnedProcess 'server' $serverBinary @($bindAddress, '--name', $hostName) $bindAddress
        Start-Sleep -Milliseconds 750
    }
    if ((Invoke-Probe $probeBinary "ws://$bindAddress" @('--expect-commit', $commit)) -ne 0) {
        throw 'Fire loopback health check failed. Address book is unchanged.'
    }

    $publicUrl = ''
    if ($null -ne (Get-OwnedProcess $tunnelRecord) -and $null -ne $release -and $release.Url -match '^wss://[a-z0-9-]+\.trycloudflare\.com$') {
        if ((Invoke-Probe $probeBinary $release.Url @('--expect-commit', $commit)) -eq 0) { $publicUrl = $release.Url }
    }
    if (!$publicUrl) {
        Stop-OwnedProcess $tunnelRecord
        $tunnelUrl = "http://$bindAddress"
        $tunnelRecord = Start-OwnedProcess 'tunnel' $Cloudflared @('tunnel', '--url', $tunnelUrl, '--no-autoupdate') $tunnelUrl
        for ($attempt = 0; $attempt -lt 30; $attempt++) {
            Start-Sleep -Seconds 2
            $log = [string](Get-Content -LiteralPath (Join-Path $runRoot 'tunnel.err.log') -Raw -ErrorAction SilentlyContinue)
            if ($log -match 'https://[a-z0-9-]+\.trycloudflare\.com') { $publicUrl = $Matches[0] -replace '^https:', 'wss:'; break }
            if ($null -eq (Get-OwnedProcess $tunnelRecord)) { throw 'Fire tunnel exited before producing an address.' }
        }
        if (!$publicUrl) { throw 'No Fire tunnel address appeared. Address book is unchanged.' }
        Write-Host 'Waiting 45 seconds for the new tunnel hostname to propagate.'
        Start-Sleep -Seconds 45
        $healthy = $false
        for ($attempt = 0; $attempt -lt 12; $attempt++) {
            if ((Invoke-Probe $probeBinary $publicUrl @('--expect-commit', $commit)) -eq 0) { $healthy = $true; break }
            Start-Sleep -Seconds 5
        }
        if (!$healthy) { throw 'Public Fire protocol/build check failed. Address book is unchanged.' }
    }
    $release = [pscustomobject]@{ Host = $hostName; Version = $version; Commit = $commit; Protocol = $protocol; Url = $publicUrl; Port = $Port; ServerSha256 = $hash; Probe = $probeBinary }
    Write-Json (Join-Path $runRoot 'release.json') $release
    Publish-Fire $release
    Write-Host "ONLINE: $hostName serves Fire protocol $protocol at $publicUrl ($version, $commit)."
} finally {
    Write-Host ('Fire host action wall time: {0:F1}s' -f $totalClock.Elapsed.TotalSeconds)
}
