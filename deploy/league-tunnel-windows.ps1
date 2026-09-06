# A separate durable task keeps the League tunnel alive across server updates.
# One mint per hour maximum, including failed attempts. No other game's process is touched.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Repository,
      [Parameter(Mandatory=$true)][string]$Probe,
      [Parameter(Mandatory=$true)][string]$Version,
      [Parameter(Mandatory=$true)][string]$Commit,
      [string]$HostName='dusky-osprey',
      [string]$TunnelBinary='C:/Users/end/tools/cloudflared.exe',
      [string]$Bash='C:/Program Files/Git/bin/bash.exe',
      [switch]$Publish)
$ErrorActionPreference='Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='Idle'
$timer=[Diagnostics.Stopwatch]::StartNew()
$run=Join-Path $env:USERPROFILE '.ember/league-local'
[IO.Directory]::CreateDirectory($run) | Out-Null
$attempt=Join-Path $run 'last-mint.txt'
if (Test-Path -LiteralPath $attempt) {
    $elapsed=([DateTime]::UtcNow-[DateTime]::Parse([IO.File]::ReadAllText($attempt)).ToUniversalTime()).TotalSeconds
    if ($elapsed -lt 3600) { throw 'League tunnel mint is cooling down for one hour; existing games remain running' }
}
if (-not @([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | Where-Object Port -eq 7783).Count) { throw 'League server must be listening before opening its tunnel' }
[IO.File]::WriteAllText($attempt,[DateTime]::UtcNow.ToString('o'))
$stem=Join-Path $run ('tunnel-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmss'))
$child=Start-Process -FilePath $TunnelBinary -ArgumentList @('tunnel','--url','http://127.0.0.1:7783','--no-autoupdate') -WindowStyle Hidden -RedirectStandardOutput ($stem+'.out.log') -RedirectStandardError ($stem+'.err.log') -PassThru
$child.PriorityClass='Idle'
try {
# Do not resolve a fresh quick-tunnel name before its DNS record has propagated.
Start-Sleep -Seconds 65
if ($child.HasExited) { throw 'League tunnel exited; inspect its log before retrying after the cooldown' }
$log=[IO.File]::ReadAllText($stem+'.err.log')
$match=[regex]::Match($log,'https://[a-z0-9-]+\.trycloudflare\.com')
if (-not $match.Success) { throw 'League tunnel did not report a public address' }
$url=$match.Value.Replace('https://','wss://')
$proven=$false
for ($i=0; $i -lt 3; $i++) {
    & $Probe $url ('deploy-'+[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) --expect-commit $Commit
    if ($LASTEXITCODE -eq 0) { $proven=$true; break }
    Start-Sleep -Seconds 30
}
if (-not $proven) { throw 'Public League match probe failed; address was not published' }
[IO.File]::WriteAllText((Join-Path $run 'url.txt'),$url)
$ready=@{url=$url;version=$Version;commit=$Commit;pid=$child.Id;verified=[DateTime]::UtcNow.ToString('o');elapsedSeconds=$timer.Elapsed.TotalSeconds}
[IO.File]::WriteAllText((Join-Path $run 'ready.json'),($ready | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
if ($Publish) {
    Push-Location -LiteralPath $Repository
    try {
        & node (Join-Path $Repository 'tools/league/publish-host.cjs') $url $Version $Commit $HostName
        if ($LASTEXITCODE -ne 0) { throw 'League is reachable but its address book publication failed' }
    } finally { Pop-Location }
}
$child.WaitForExit()
Write-Output ('League tunnel exited after '+$timer.Elapsed.TotalSeconds+' seconds; retry cooldown remains in force')
$result=$child.ExitCode
} catch {
    # Only the exact child created above belongs to this task. Leave every
    # other tunnel alone, and never orphan an unpublished replacement.
    if (-not $child.HasExited) { $child.Kill(); $child.WaitForExit() }
    throw
} finally { $child.Dispose() }
exit $result
