# ember heartbeat - the local watcher.
# Appends one line to marketing/heartbeat.local.log (gitignored) and prints the
# state. ACTIVE means a commit landed in the last hour; IDLE means the lane is
# quiet and the board owes an explanation.
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $stamp = (Get-Date).ToString("yyyy-MM-dd HH:mm:ss")
    $hash  = (git log -1 --pretty=format:%h 2>$null)
    $state = "NO_HISTORY"
    $ct = git log -1 --pretty=format:%ct 2>$null
    if ($ct) {
        $hours = (Get-Date) - [DateTimeOffset]::FromUnixTimeSeconds([int64]$ct).LocalDateTime
        if ($hours.TotalHours -lt 1) { $state = "ACTIVE" } else { $state = "IDLE" }
    }
    $dirty = @(git status --porcelain)
    $line = "{0} status={1} last={2} dirty={3}" -f $stamp, $state, $hash, $dirty.Count
    Add-Content -LiteralPath (Join-Path $PSScriptRoot "heartbeat.local.log") -Value $line -Encoding ascii
    Write-Output $line
} finally {
    Pop-Location
}
