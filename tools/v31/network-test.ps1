# One owned private server for the eight-peer gate. Never touches live port7780.
$ErrorActionPreference = 'Stop'
$watch = [Diagnostics.Stopwatch]::StartNew()
[Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
$root = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$output = Join-Path $root 'target/killshot-v31-network'
New-Item -ItemType Directory -Path $output -Force | Out-Null
if (Get-NetTCPConnection -State Listen -LocalPort 7788 -ErrorAction SilentlyContinue) { throw 'QA port7788 is occupied; refusing to touch existing service' }
$child = $null
$result = 1
try {
    $child = Start-Process -FilePath (Join-Path $root 'target/release/arena-server.exe') -ArgumentList @('--bind','127.0.0.1:7788','--name','killshot-v31-network-qa') -WorkingDirectory $root -WindowStyle Hidden -RedirectStandardOutput (Join-Path $output 'server.stdout.log') -RedirectStandardError (Join-Path $output 'server.stderr.log') -PassThru
    $child.PriorityClass = 'Idle'
    $ready = $false
    for ($attempt=0; $attempt -lt 30; $attempt++) {
        if ($child.HasExited) { throw 'Owned QA server exited before ready' }
        $listener = Get-NetTCPConnection -State Listen -LocalPort 7788 -ErrorAction SilentlyContinue
        if ($listener) {
            if (@($listener | Where-Object OwningProcess -NE $child.Id).Count -gt 0) { throw 'QA7788 listener is not the owned child; refusing to use it' }
            $ready = $true
            break
        }
        Start-Sleep -Milliseconds 100
    }
    if (!$ready) { throw 'Owned QA server did not listen on127.0.0.1:7788 before timeout' }
    Push-Location $root
    try { node tools/v31/network-shotgun.cjs ws://127.0.0.1:7788; $result = $LASTEXITCODE } finally { Pop-Location }
} finally {
    if ($null -ne $child -and !$child.HasExited) { $child.Kill(); $child.WaitForExit() }
    Write-Output ('Network gate wall time: ' + $watch.Elapsed.TotalSeconds + 's')
}
exit $result
