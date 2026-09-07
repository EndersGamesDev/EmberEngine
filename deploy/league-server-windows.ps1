# Each League runtime owns its selected port. Never stop an existing listener.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Binary,
      [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256,
      [ValidatePattern('^[a-z0-9-]{3,32}$')][string]$HostName='dusky-osprey',
      [ValidateRange(1024,65535)][int]$Port=7783,
      [ValidatePattern('^v[1-9][0-9]{0,5}$')][string]$TaskVersion='v1')
function Get-LeagueListeners { @([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | Where-Object Port -eq $Port) }
$ErrorActionPreference='Stop'
$timer=[Diagnostics.Stopwatch]::StartNew()
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='Idle'
if ($TaskVersion -ne 'v1' -and $Port -eq 7783) { throw 'A separate League task version must use its own port; 7783 belongs to the legacy runtime' }
if ((Get-FileHash -LiteralPath $Binary -Algorithm SHA256).Hash -ne $ExpectedSha256) { throw 'League binary hash mismatch' }
if (@(Get-LeagueListeners).Count) { throw ('League port '+$Port+' is already occupied') }
$stateName=if($TaskVersion -eq 'v1' -and $Port -eq 7783){'league-local'}else{'league-local-'+$TaskVersion+'-'+$Port}
$logs=Join-Path $env:USERPROFILE ('.ember/'+$stateName)
[IO.Directory]::CreateDirectory($logs) | Out-Null
$stem=Join-Path $logs ('server-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmss'))
$child=Start-Process -FilePath $Binary -ArgumentList @(('127.0.0.1:'+$Port),'--name',$HostName) -WindowStyle Hidden -RedirectStandardOutput ($stem+'.out.log') -RedirectStandardError ($stem+'.err.log') -PassThru
$child.PriorityClass='Idle'
$child.WaitForExit()
Write-Output ('League server exited after '+$timer.Elapsed.TotalSeconds+' seconds')
exit $child.ExitCode
