# Launch a script inside a WSL distro as a DETACHED process and print its PID.
#
#   pwsh -NoProfile -File deploy/wsl-detach.ps1 -Distro claude-sdk `
#       -Script /mnt/c/Users/Admin/dev/ember/deploy/wsl/kings-run-server.sh 127.0.0.1:7782 amber-otter
#
# Why this exists. The Four Kings server and its Cloudflare tunnel run inside
# the claude-sdk WSL distro, which has no systemd, so something has to keep
# them alive after the deploy script returns. A `nohup ... &` inside
# `wsl -d claude-sdk -- bash ...` does NOT survive that command's exit: WSL
# reaps the session's processes and the port goes dead while the distro still
# looks healthy (the claude-web lesson in the global CLAUDE.md). What does
# survive is a wsl.exe that is still running, so this helper starts one with
# Start-Process, hidden, and lets it outlive the caller. The Linux process is
# that wsl.exe's foreground child; the run scripts under deploy/wsl/ exec it
# with output appended to a log inside the distro.
#
# `.ps1` is the only host scripting on this machine, which is why the launch
# is PowerShell rather than a .bat wrapper; deploy-kings-online.sh calls it
# through `pwsh -NoProfile -File`, falling back to powershell.exe.
#
# Every argument after -Script is passed to the script verbatim. Empty
# arguments cannot cross Start-Process (they are dropped), so callers omit an
# optional argument rather than passing "". The only line on stdout is the
# PID of the wsl.exe, so a caller can capture it.
[CmdletBinding()]
param(
    [string]$Distro = 'claude-sdk',
    [Parameter(Mandatory = $true)][string]$Script,
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$ScriptArgs = @()
)

$ErrorActionPreference = 'Stop'

if (-not $Script.StartsWith('/')) {
    Write-Error "wsl-detach: -Script must be a Linux path inside the distro (/mnt/c/...), got '$Script'"
    exit 2
}

# Start-Process joins the array with spaces and does not quote for us, so an
# argument carrying whitespace is wrapped here. None of the deploy's arguments
# do today (bind address, host name, port), but a repo path with a space in it
# would otherwise split silently.
function Quote([string]$s) {
    if ($s -match '\s') { return '"' + ($s -replace '"', '\"') + '"' }
    return $s
}

$argList = @('-d', (Quote $Distro), '--', 'bash', (Quote $Script))
foreach ($a in $ScriptArgs) { $argList += (Quote $a) }

$proc = Start-Process -FilePath 'wsl.exe' -ArgumentList $argList -WindowStyle Hidden -PassThru
if (-not $proc) {
    Write-Error 'wsl-detach: Start-Process returned nothing'
    exit 1
}
Write-Output $proc.Id
