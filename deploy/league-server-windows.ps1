# League owns only port 7783. This task never stops an existing listener.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][string]$Binary,
      [Parameter(Mandatory=$true)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$ExpectedSha256,
      [string]$HostName='dusky-osprey')
$ErrorActionPreference='Stop'
$timer=[Diagnostics.Stopwatch]::StartNew()
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='Idle'
if ((Get-FileHash -LiteralPath $Binary -Algorithm SHA256).Hash -ne $ExpectedSha256) { throw 'League binary hash mismatch' }
if (@([Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners() | Where-Object Port -eq 7783).Count) { throw 'Port 7783 is already occupied' }
$logs=Join-Path $env:USERPROFILE '.ember/league-local'
[IO.Directory]::CreateDirectory($logs) | Out-Null
$stem=Join-Path $logs ('server-'+[DateTime]::UtcNow.ToString('yyyyMMddTHHmmss'))
$child=Start-Process -FilePath $Binary -ArgumentList @('127.0.0.1:7783','--name',$HostName) -WindowStyle Hidden -RedirectStandardOutput ($stem+'.out.log') -RedirectStandardError ($stem+'.err.log') -PassThru
$child.PriorityClass='Idle'
$child.WaitForExit()
Write-Output ('League server exited after '+$timer.Elapsed.TotalSeconds+' seconds')
exit $child.ExitCode
