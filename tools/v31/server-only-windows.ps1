# Server-only owner for approved Killshot v31; never creates/stops a tunnel.
[CmdletBinding()]
param([Parameter(Mandatory=$true)][ValidatePattern('^[A-Fa-f0-9]{64}$')][string]$ExpectedSha256)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$directory = 'C:/Users/end/dev/ember-killshot-live-v31'
$binary = Join-Path $directory 'target/release/arena-server.exe'
$logs = 'C:/Users/end/.ember/arena-local/v31-server-only'
$child = $null
$result = 1
try {
    if (!(Test-Path -LiteralPath $binary -PathType Leaf)) { throw 'Pinned Arena binary is missing' }
    $actual = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
    if (![string]::Equals($actual,$ExpectedSha256,[StringComparison]::OrdinalIgnoreCase)) { throw 'Arena binary SHA256 mismatch; refusing launch' }
    $listeners = [Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners()
    if (@($listeners | Where-Object Port -eq 7780).Count -ne 0) { throw 'Port7780 is occupied; this launcher never stops its owner' }
    [Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
    [IO.Directory]::CreateDirectory($logs) | Out-Null
    $stem = 'arena-v31-{0}-{1}' -f [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ'),$PID
    $env:EMBER_HOST_NAME = 'dusky-osprey'
    $env:RUST_LOG = 'info'
    $child = Start-Process -FilePath $binary -ArgumentList @('--bind','127.0.0.1:7780') -WorkingDirectory $directory -WindowStyle Hidden -RedirectStandardOutput (Join-Path $logs ($stem+'.stdout.log')) -RedirectStandardError (Join-Path $logs ($stem+'.stderr.log')) -PassThru
    $child.PriorityClass = 'Idle'
    Write-Output ('Killshot v31 PID={0}; SHA256={1}' -f $child.Id,$actual)
    $child.WaitForExit()
    $result = $child.ExitCode
} catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    if ($null -ne $child) { try { $child.WaitForExit() } catch { [Console]::Error.WriteLine($_.Exception.Message) } }
} finally {
    if ($null -ne $child) { $child.Dispose() }
}
exit $result
