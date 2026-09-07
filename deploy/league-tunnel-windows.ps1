# A separate durable task keeps the League tunnel alive across server updates.
# Automated attempts keep a one-hour cooldown, including failed attempts.
# -RecoverSetupFromLog permits one explicit recovery of an unpublished initial
# setup whose own log proves creation and registration without rate limiting.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Repository,
      [Parameter(Mandatory=$true)][string]$Probe,
      [Parameter(Mandatory=$true)][string]$Version,
      [Parameter(Mandatory=$true)][string]$Commit,
      [ValidatePattern('^[a-z0-9-]{3,32}$')][string]$HostName='dusky-osprey',
      [string]$TunnelBinary='C:/Users/end/tools/cloudflared.exe',
      [Parameter(Mandatory=$true)][string]$Node,
      [string]$RecoverSetupFromLog,
      [switch]$Publish,
      [ValidateRange(1024,65535)][int]$Port=7783,
      [ValidatePattern('^v[1-9][0-9]{0,5}$')][string]$TaskVersion='v1',
      [ValidateRange(1,65535)][int]$Protocol=1)

function Get-LeagueListeners { @([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | Where-Object Port -eq $Port) }

function Read-LeagueTunnelLog {
    param([Parameter(Mandatory=$true)][string]$LogPath)
    # Get-Content opens with FileShare.ReadWrite; File.ReadAllText excludes
    # an active redirected writer and fails even when its contents are ready.
    Get-Content -LiteralPath $LogPath -Raw -ErrorAction Stop
}

function Use-LeagueSetupRecovery {
    param([Parameter(Mandatory=$true)][string]$LogPath,
          [Parameter(Mandatory=$true)][string]$RunDirectory,
          [Parameter(Mandatory=$true)][DateTime]$LastMint,
          [ValidateRange(1024,65535)][int]$OriginPort=7783)
    $prior=(Resolve-Path -LiteralPath $LogPath -ErrorAction Stop).Path
    $runPath=(Resolve-Path -LiteralPath $RunDirectory -ErrorAction Stop).Path
    if ([IO.Path]::GetDirectoryName($prior) -ne $runPath -or
        [IO.Path]::GetFileName($prior) -notmatch '^tunnel-(\d{8}T\d{6}(?:\d{3})?)\.err\.log$') {
        throw 'Setup recovery requires an own tunnel stderr log directly in the League run directory'
    }
    $format=if($Matches[1].Length -eq 15){'yyyyMMddTHHmmss'}else{'yyyyMMddTHHmmssfff'}
    $logged=[DateTime]::ParseExact($Matches[1],$format,[Globalization.CultureInfo]::InvariantCulture,[Globalization.DateTimeStyles]::AssumeUniversal).ToUniversalTime()
    if ([Math]::Abs(($logged-$LastMint.ToUniversalTime()).TotalSeconds) -gt 2) {
        throw 'Setup recovery log does not belong to the latest mint attempt'
    }
    if ((Test-Path -LiteralPath (Join-Path $runPath 'ready.json')) -or
        (Test-Path -LiteralPath (Join-Path $runPath 'url.txt'))) {
        throw 'Setup recovery is limited to an initial attempt that never became ready'
    }
    $priorText=Read-LeagueTunnelLog -LogPath $prior
    if ($priorText -notmatch 'Your quick Tunnel has been created' -or
        $priorText -notmatch 'Registered tunnel connection' -or
        $priorText -notmatch ([regex]::Escape('url:http://127.0.0.1:'+$OriginPort)+'(?:\]|\s)') -or
        $priorText -notmatch 'https://[a-z0-9-]+\.trycloudflare\.com') {
        throw 'Setup recovery log must prove creation and registration for the League origin'
    }
    if ($priorText -match '(?i)\b429\b|too many requests|rate[- ]limit') {
        throw 'Setup recovery cannot bypass rate limiting'
    }
    # CreateNew is an atomic, permanent one-use claim. Even a failed recovery
    # consumes it; automated retries must still wait the normal hour.
    $marker=Join-Path $runPath 'setup-recovery-used.json'
    $claim=[IO.File]::Open($marker,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    try {
        $data=@{priorLog=$prior;lastMint=$LastMint.ToUniversalTime().ToString('o');consumed=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json
        $bytes=[Text.UTF8Encoding]::new($false).GetBytes($data)
        $claim.Write($bytes,0,$bytes.Length)
    } finally { $claim.Dispose() }
    $prior
}

$ErrorActionPreference='Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='Idle'
if (($TaskVersion -ne 'v1' -or $Protocol -ne 1) -and $Port -eq 7783) { throw 'A separate League runtime must use its own port; 7783 belongs to the legacy runtime' }
if ($Protocol -ne 1 -and ($TaskVersion -eq 'v1' -or $HostName -eq 'dusky-osprey')) { throw 'A new League protocol requires a separate task version and host name' }
$timer=[Diagnostics.Stopwatch]::StartNew()
$stateName=if($TaskVersion -eq 'v1' -and $Port -eq 7783){'league-local'}else{'league-local-'+$TaskVersion+'-'+$Port}
$run=Join-Path $env:USERPROFILE ('.ember/'+$stateName)
$origin='http://127.0.0.1:'+$Port
[IO.Directory]::CreateDirectory($run) | Out-Null
$stem=Join-Path $run ('tunnel-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfff'))
$child=$null
$transcribing=$false
$stage='preflight'
$result=1
try {
    Start-Transcript -Path ($stem+'.task.log') -Force | Out-Null
    $transcribing=$true
    $imageName=[IO.Path]::GetFileName($TunnelBinary).Replace("'","''")
    $existing=@(Get-CimInstance Win32_Process -Filter "Name='$imageName'" -ErrorAction Stop |
        Where-Object { $_.CommandLine -match ([regex]::Escape($origin)+'(?:\s|"|$)') })
    if ($existing.Count) { throw ('A League tunnel is already running: '+($existing.ProcessId -join ', ')) }

    $attempt=Join-Path $run 'last-mint.txt'
    $lastMint=$null
    if (Test-Path -LiteralPath $attempt) {
        $lastMint=[DateTime]::Parse([IO.File]::ReadAllText($attempt)).ToUniversalTime()
    }
    if (-not @(Get-LeagueListeners).Count) {
        throw 'League server must be listening before opening its tunnel'
    }
    # Refuse a mismatched binary/protocol before consuming a mint attempt.
    $stage='proving loopback match'
    $localProof=& $Probe ('ws://127.0.0.1:'+$Port) 'tunnel-preflight' --expect-commit $Commit
    $localHealthy=$LASTEXITCODE -eq 0 -and ($localProof -join "`n") -match ('wsprobe: league protocol v'+$Protocol+' healthy\b')
    Write-Output $localProof
    if (-not $localHealthy) { throw 'League loopback match/protocol proof failed; no tunnel opened' }
    if ($RecoverSetupFromLog) {
        if ($null -eq $lastMint) { throw 'Setup recovery requires a recorded prior mint attempt' }
        $recovered=Use-LeagueSetupRecovery -LogPath $RecoverSetupFromLog -RunDirectory $run -LastMint $lastMint -OriginPort $Port
        Write-Output ('Consumed one-time initial setup recovery for '+$recovered)
    } elseif ($null -ne $lastMint -and ([DateTime]::UtcNow-$lastMint).TotalSeconds -lt 3600) {
        throw 'League tunnel mint is cooling down for one hour; existing games remain running'
    }
    [IO.File]::WriteAllText($attempt,[DateTime]::UtcNow.ToString('o'))
    $stage='starting tunnel'
    $child=Start-Process -FilePath $TunnelBinary -ArgumentList @('tunnel','--url',$origin,'--no-autoupdate') -WindowStyle Hidden -RedirectStandardOutput ($stem+'.out.log') -RedirectStandardError ($stem+'.err.log') -PassThru
    $child.PriorityClass='Idle'
    $stage='waiting for tunnel DNS'
    # Do not resolve a fresh quick-tunnel name before its DNS record propagates.
    Start-Sleep -Seconds 65
    if ($child.HasExited) { throw 'League tunnel exited; inspect its log before retrying after the cooldown' }
    $stage='reading active tunnel log'
    $log=Read-LeagueTunnelLog -LogPath ($stem+'.err.log')
    $match=[regex]::Match($log,'https://[a-z0-9-]+\.trycloudflare\.com')
    if (-not $match.Success) { throw 'League tunnel did not report a public address' }
    $url=$match.Value.Replace('https://','wss://')
    $stage='proving public match'
    $proven=$false
    for ($i=0; $i -lt 3; $i++) {
        $proof=& $Probe $url ('deploy-'+[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) --expect-commit $Commit
        $healthy=$LASTEXITCODE -eq 0 -and ($proof -join "`n") -match ('wsprobe: league protocol v'+$Protocol+' healthy\b')
        Write-Output $proof
        if ($healthy) { $proven=$true; break }
        Start-Sleep -Seconds 30
    }
    if (-not $proven) { throw 'Public League match probe failed; address was not published' }
    [IO.File]::WriteAllText((Join-Path $run 'url.txt'),$url)
    $ready=@{url=$url;version=$Version;commit=$Commit;port=$Port;protocol=$Protocol;taskVersion=$TaskVersion;host=$HostName;pid=$child.Id;verified=[DateTime]::UtcNow.ToString('o');elapsedSeconds=$timer.Elapsed.TotalSeconds}
    [IO.File]::WriteAllText((Join-Path $run 'ready.json'),($ready | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
    if ($Publish) {
        $stage='publishing address book'
        Push-Location -LiteralPath $Repository
        try {
            $published=$false
            for($try=0;$try -lt 3;$try++) {
                & $Node (Join-Path $Repository 'tools/league/publish-host.cjs') $url $Version $Commit $HostName "--protocol=$Protocol"
                if($LASTEXITCODE -eq 0){$published=$true;break}
                Start-Sleep -Seconds 5
            }
            if (-not $published) { throw 'League is reachable but its address book publication failed' }
        } finally { Pop-Location }
    }
    $stage='running tunnel'
    $child.WaitForExit()
    $result=$child.ExitCode
    if ($result -ne 0) { throw ('League tunnel exited with code '+$result) }
    Write-Output ('League tunnel exited after '+$timer.Elapsed.TotalSeconds+' seconds; retry cooldown remains in force')
} catch {
    $failure=@{time=[DateTime]::UtcNow.ToString('o');stage=$stage;message=$_.Exception.Message;error=($_ | Out-String);stack=$_.ScriptStackTrace;version=$Version;commit=$Commit;port=$Port;protocol=$Protocol;taskVersion=$TaskVersion;host=$HostName;elapsedSeconds=$timer.Elapsed.TotalSeconds;stderrLog=($stem+'.err.log');transcript=($stem+'.task.log');recoveryLog=$RecoverSetupFromLog}
    if ($null -ne $child) { $failure.pid=$child.Id }
    $failureText=$failure | ConvertTo-Json -Depth 5
    foreach($destination in @(($stem+'.error.json'),(Join-Path $run 'latest-failure.json'))) {
        try { [IO.File]::WriteAllText($destination,$failureText,[Text.UTF8Encoding]::new($false)) }
        catch { Write-Warning ('Could not persist League failure: '+$_.Exception.Message) }
    }
    # Only the exact child created by this invocation belongs to this task.
    if ($null -ne $child -and -not $child.HasExited) { $child.Kill(); $child.WaitForExit() }
    throw
} finally {
    if ($null -ne $child) { $child.Dispose() }
    if ($transcribing) { Stop-Transcript | Out-Null }
}
exit $result
