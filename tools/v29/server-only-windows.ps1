# Server-only maintenance: obtain restart approval before registering/running.
# Deliberately no process stopping, tunnel creation or publication operations.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Fa-f0-9]{64}$')]
    [string]$ExpectedSha256
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$arenaBinary = 'C:\Users\end\dev\ember-environment\target\release\arena-server.exe'
$arenaWorkingDirectory = 'C:\Users\end\dev\ember-environment'
$arenaLogDirectory = 'C:\Users\end\.ember\arena-local\v29-server-only'
$arenaChild = $null
$arenaExitCode = 1

try {
    if (-not (Test-Path -LiteralPath $arenaBinary -PathType Leaf)) {
        throw "Pinned Arena binary is missing: $arenaBinary"
    }
    $arenaActualSha256 = (Get-FileHash -LiteralPath $arenaBinary -Algorithm SHA256).Hash
    if (-not [string]::Equals($arenaActualSha256, $ExpectedSha256, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Arena binary SHA256 mismatch. Expected $ExpectedSha256; found $arenaActualSha256."
    }

    # Include wildcard/IPv6 listeners. The server's bind is authoritative if
    # another process races this read-only check.
    $arenaListeners = [System.Net.NetworkInformation.IPGlobalProperties]::GetIPGlobalProperties().GetActiveTcpListeners()
    if (@($arenaListeners | Where-Object { $_.Port -eq 7780 }).Count -ne 0) {
        throw 'TCP port 7780 is already listening; this launcher never stops its owner.'
    }

    [System.Diagnostics.Process]::GetCurrentProcess().PriorityClass = [System.Diagnostics.ProcessPriorityClass]::Idle
    [System.IO.Directory]::CreateDirectory($arenaLogDirectory) | Out-Null
    $arenaLogStem = 'arena-v29-{0}-{1}' -f [DateTime]::UtcNow.ToString('yyyyMMddTHHmmssfffffffZ'), $PID
    $arenaStdout = Join-Path $arenaLogDirectory ($arenaLogStem + '.stdout.log')
    $arenaStderr = Join-Path $arenaLogDirectory ($arenaLogStem + '.stderr.log')
    $env:EMBER_HOST_NAME = 'dusky-osprey'
    $env:RUST_LOG = 'info'

    $arenaChild = Start-Process -FilePath $arenaBinary -ArgumentList @('--bind', '127.0.0.1:7780') -WorkingDirectory $arenaWorkingDirectory -WindowStyle Hidden -RedirectStandardOutput $arenaStdout -RedirectStandardError $arenaStderr -PassThru
    $arenaChild.PriorityClass = [System.Diagnostics.ProcessPriorityClass]::Idle
    Write-Output ('Arena v29 PID={0}; SHA256={1}; stdout={2}; stderr={3}' -f $arenaChild.Id, $arenaActualSha256, $arenaStdout, $arenaStderr)
    # Task Scheduler must own this launcher, which remains alive for its child.
    $arenaChild.WaitForExit()
    $arenaExitCode = $arenaChild.ExitCode
}
catch {
    [Console]::Error.WriteLine($_.Exception.Message)
    # A diagnostic failure after launch must not abandon a running child.
    if ($null -ne $arenaChild) {
        try { $arenaChild.WaitForExit() } catch { [Console]::Error.WriteLine($_.Exception.Message) }
    }
    $arenaExitCode = 1
}
finally {
    if ($null -ne $arenaChild) { $arenaChild.Dispose() }
}

exit $arenaExitCode
