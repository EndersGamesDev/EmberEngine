# Install only the new League services after the release has passed its gates.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Repository,
      [Parameter(Mandatory=$true)][string]$Version,
      [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-f]{7,40}$')][string]$Commit,
      [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256)
$ErrorActionPreference='Stop'
$timer=[Diagnostics.Stopwatch]::StartNew()
$Repository=(Resolve-Path -LiteralPath $Repository).Path
$binary=Join-Path $Repository 'target/release/league-server.exe'
$probe=Join-Path $Repository 'target/release/examples/wsprobe.exe'
$node=(Get-Command node -ErrorAction Stop).Source
if((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash -ne $ExpectedSha256){throw 'Tested League binary changed'}
foreach($name in 'ember-league-server-v1','ember-league-tunnel-v1'){
    if(Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue){throw ('Task already exists: '+$name+'; inspect it before changing a running service')}
}
if(@([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners()|Where-Object Port -eq 7783).Count){throw 'League port 7783 is occupied'}
$ps=Join-Path $env:WINDIR 'System32/WindowsPowerShell/v1.0/powershell.exe'
$user=[Security.Principal.WindowsIdentity]::GetCurrent().Name
$trigger=New-ScheduledTaskTrigger -AtLogOn -User $user
$settings=New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew -RestartCount 3 -RestartInterval (New-TimeSpan -Hours 1)
$serverArgs='-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $Repository 'deploy/league-server-windows.ps1')+'" -Binary "'+$binary+'" -ExpectedSha256 '+$ExpectedSha256
$action=New-ScheduledTaskAction -Execute $ps -Argument $serverArgs -WorkingDirectory $Repository
Register-ScheduledTask -TaskName 'ember-league-server-v1' -Action $action -Trigger $trigger -Settings $settings | Out-Null
Start-ScheduledTask -TaskName 'ember-league-server-v1'
for($i=0;$i -lt 30;$i++){
    if(@([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners()|Where-Object Port -eq 7783).Count){break}
    Start-Sleep -Seconds 1
}
& $probe 'ws://127.0.0.1:7783' 'install-proof' --expect-commit $Commit
if($LASTEXITCODE -ne 0){throw 'Installed League server failed loopback match proof; no tunnel opened'}
$tunnelArgs='-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $Repository 'deploy/league-tunnel-windows.ps1')+'" -Repository "'+$Repository+'" -Probe "'+$probe+'" -Node "'+$node+'" -Version '+$Version+' -Commit '+$Commit+' -Publish'
$action=New-ScheduledTaskAction -Execute $ps -Argument $tunnelArgs -WorkingDirectory $Repository
# At logon the server gets time to start before the tunnel probes it.
$tunnelTrigger=New-ScheduledTaskTrigger -AtLogOn -User $user
$tunnelTrigger.Delay='PT30S'
Register-ScheduledTask -TaskName 'ember-league-tunnel-v1' -Action $action -Trigger $tunnelTrigger -Settings $settings | Out-Null
Start-ScheduledTask -TaskName 'ember-league-tunnel-v1'
Write-Output ('League tasks installed and server verified in '+$timer.Elapsed.TotalSeconds+' seconds; tunnel proof/publication runs independently')
