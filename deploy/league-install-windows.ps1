# Install only the new League services after the release has passed its gates.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Repository,
      [Parameter(Mandatory=$true)][string]$Version,
      [Parameter(Mandatory=$true)][ValidatePattern('^[0-9a-f]{7,40}$')][string]$Commit,
      [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256,
      [ValidateRange(1024,65535)][int]$Port=7783,
      [ValidatePattern('^v[1-9][0-9]{0,5}$')][string]$TaskVersion='v1',
      [ValidateRange(1,65535)][int]$Protocol=1,
      [ValidatePattern('^[a-z0-9-]{3,32}$')][string]$HostName='dusky-osprey')
function Get-LeagueListeners { @([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | Where-Object Port -eq $Port) }
$ErrorActionPreference='Stop'
$timer=[Diagnostics.Stopwatch]::StartNew()
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='Idle'
if (($TaskVersion -ne 'v1' -or $Protocol -ne 1) -and $Port -eq 7783) { throw 'A separate League runtime must use its own port; 7783 belongs to the legacy runtime' }
if ($Protocol -ne 1 -and ($TaskVersion -eq 'v1' -or $HostName -eq 'dusky-osprey')) { throw 'A new League protocol requires a separate task version and host name' }
$Repository=(Resolve-Path -LiteralPath $Repository).Path
$binary=Join-Path $Repository 'target/release/league-server.exe'
$probe=Join-Path $Repository 'target/release/examples/wsprobe.exe'
$node=(Get-Command node -ErrorAction Stop).Source
if((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash -ne $ExpectedSha256){throw 'Tested League binary changed'}
$serverTask='ember-league-server-'+$TaskVersion
$tunnelTask='ember-league-tunnel-'+$TaskVersion
foreach($name in $serverTask,$tunnelTask){
    if(Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue){throw ('Task already exists: '+$name+'; inspect it before changing a running service')}
}
if(@(Get-LeagueListeners).Count){throw ('League port '+$Port+' is occupied')}
$ps=Join-Path $env:WINDIR 'System32/WindowsPowerShell/v1.0/powershell.exe'
$user=[Security.Principal.WindowsIdentity]::GetCurrent().Name
$trigger=New-ScheduledTaskTrigger -AtLogOn -User $user
$settings=New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew -RestartCount 3 -RestartInterval (New-TimeSpan -Hours 1)
$runtimeArgs=' -Port '+$Port+' -TaskVersion '+$TaskVersion+' -HostName '+$HostName
$serverArgs='-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $Repository 'deploy/league-server-windows.ps1')+'" -Binary "'+$binary+'" -ExpectedSha256 '+$ExpectedSha256+$runtimeArgs
$action=New-ScheduledTaskAction -Execute $ps -Argument $serverArgs -WorkingDirectory $Repository
Register-ScheduledTask -TaskName $serverTask -Action $action -Trigger $trigger -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $serverTask
for($i=0;$i -lt 30;$i++){
    if(@(Get-LeagueListeners).Count){break}
    Start-Sleep -Seconds 1
}
$proof=& $probe ('ws://127.0.0.1:'+$Port) 'install-proof' --expect-commit $Commit
$healthy=$LASTEXITCODE -eq 0 -and ($proof -join "`n") -match ('wsprobe: league protocol v'+$Protocol+' healthy\b')
Write-Output $proof
if(-not $healthy){throw 'Installed League server failed loopback match/protocol proof; no tunnel opened'}
$tunnelArgs='-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $Repository 'deploy/league-tunnel-windows.ps1')+'" -Repository "'+$Repository+'" -Probe "'+$probe+'" -Node "'+$node+'" -Version '+$Version+' -Commit '+$Commit+$runtimeArgs+' -Protocol '+$Protocol+' -Publish'
$action=New-ScheduledTaskAction -Execute $ps -Argument $tunnelArgs -WorkingDirectory $Repository
# At logon the server gets time to start before the tunnel probes it.
$tunnelTrigger=New-ScheduledTaskTrigger -AtLogOn -User $user
$tunnelTrigger.Delay='PT30S'
Register-ScheduledTask -TaskName $tunnelTask -Action $action -Trigger $tunnelTrigger -Settings $settings | Out-Null
Start-ScheduledTask -TaskName $tunnelTask
Write-Output ('League tasks installed and server verified in '+$timer.Elapsed.TotalSeconds+' seconds; tunnel proof/publication runs independently')
