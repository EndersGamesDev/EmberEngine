<#
.SYNOPSIS
The capture harness: a local server, one or two native clients that drive
THEMSELVES, and screenshots of the client windows.

.DESCRIPTION
THE CONTRACT, and it is the first thing to read: this script must never take
the operator's input again. Someone is using this machine. Nothing here may
move the cursor, synthesise a key or a click, or take the foreground — no
SetCursorPos, no mouse_event, no keybd_event, no SendInput, no
SetForegroundWindow, no ShowWindow. The earlier version did all of those,
and every run stole the machine from whoever was sitting at it: their
pointer was dragged into a game window, their clicks landed in it, and their
own mouse motion turned our camera (winit delivers raw mouse motion to a
window whether it is focused or not, which is why captured frames came back
with a view jump in them).

Driving the game is now the CLIENT'S job. `EMBER_SCRIPT` carries a timeline
of what to do and the client feeds it into its own input state each frame;
while it is set the client reads no keyboard, no mouse, no mouse motion and
no gamepad, it never grabs the cursor, and its window opens without taking
the foreground. The grammar and the reasoning live in one place,
`crates/arena/src/script.rs`; the short of it is that steps are separated by
`;`, a step is a few words and an optional duration in seconds, and time is
the client's own frame clock:

    wait 1.5; walk w 2; sprint w 2; crouch w 2; aim 90; ads fire 0.12; wait 0.5

EVERY client gets a script, including an idle observer, and an EMPTY one
counts: `EMBER_SCRIPT=""` is a blank timeline, not an unscripted client.
This script refuses to launch a client without one, because a client with no
EMBER_SCRIPT at all is the pre-fix client — it grabs the pointer, takes the
foreground and reads the device — and that is one flag away from happening
by accident.

This script's only jobs are: start the server on 7778 and the clients with
their environment, place their windows apart, capture window rectangles, and
stop everything.

What earlier sessions paid for and this still encodes:

  * the game window is found by enumerating a pid's visible top-level
    windows and taking the one whose title starts with "ember";
  * NEVER ShowWindow(SW_SHOW) a winit window from outside — measured on
    2026-09-03, it HIDES it (IsWindowVisible flips to false and stays);
  * the two windows open at the same spot, so they are placed apart or a
    capture holds the wrong client;
  * a shot is proved from the client's own `status=` lines (the ammo count),
    not from the picture;
  * a second client is told where to look with EMBER_CAM and both windows
    are captured in one pass, so a pair proves which side the action was on.

The capture reads the screen rectangle of each window, so the windows are
raised to topmost with SWP_NOACTIVATE — raised, never activated: that takes
screen space, which is visible and reversible, and never takes focus or
input. They are dropped back to NOTOPMOST in a `finally`, so a `-KeepRunning`
run does not leave two game windows sitting over the operator's work.
Anything the operator drags over them while the shots are being taken will
land in the picture, so give them a clear corner (-X, -Y, -Gap) or a second
monitor.

Usage (from the repo root, after `cargo build -p arena -p arena-server --bins`; -Profile release for release binaries):

    powershell -ExecutionPolicy Bypass -File tools/v18/capture.ps1 -Out tools/v18 -Prefix smoke `
        -ScriptA 'wait 1.5; melee; wait 0.25; fire 0.05; wait 0.5' `
        -Shots 'idle@1.4;melee#2;shot@2.3'

-ScriptA is client A's timeline; -ScriptB is client B's, and defaults to a
long `wait` so the observer is hands-off too. -Shots are the moments to
photograph, ONE `;`-separated string (an array works too, in-process), each
written to <Out>/<Prefix>-<name>-<client>.png (-Out is relative to the repo
root, or absolute), in either of two forms:

  * `name@SECONDS` — seconds on this script's WALL clock, counted from the
    moment client A logged "EMBER_SCRIPT starts" (its first scripted frame —
    NOT when its window appeared, which is several seconds earlier);
  * `name#N` — when client A begins step N of its script, read from A's own
    log. Prefer this. The two clocks drift: the engine clamps `dt` at 100 ms,
    so every frame slower than 10 fps loses script time for good, and a dev
    client on a machine someone is working at does hitch.

Both forms depend on the client's stdout, which reaches this script through
`Register-ObjectEvent` handlers that only run when the runspace is idle. At
startup that lags — measured at 2.0 s, while a burst of wgpu INFO lines
drains — so the wall clock is back-dated to the client's own stamp on
"EMBER_SCRIPT starts" and the lag is printed. A `#N` shot is still only as
prompt as the log stream, so do NOT photograph step 1 — its line arrives
during that drain (measured: `idle#1` landed 3.5 s into a script whose step 1
was `wait 3.0`, i.e. after it had ended). Head every script with a `wait` and
photograph from step 2 on; by then the stream has caught up, and every later
step in the runs this was written for landed within about 20 ms of the step
the client logged.

The evidence pass at the end waits, bounded, for alpha to log "EMBER_SCRIPT
is spent" before reading the status lines: the last photograph is usually
taken before the last step runs, and reading the log at that moment shows a
magazine that has not been fired yet.

Shots are taken in the order you give them, so the two forms interleave.
Both take a hashtable too (`@{name='shot'; at=2.3}`, `@{name='shot';
step=4}`) when the script is called IN-PROCESS. Through `powershell -File`
every argument arrives as a string and a hashtable is lost — the repo paid
for that once with `-Steps` — so the string forms above are the ones the
usage line uses, and they work either way.

-Map picks the lobby's map (v18 servers), -Mode the lobby's mode (v19
servers: ffa, tdm or hill), -Cam pins client B's observer camera, -Weapon
sets EMBER_WEAPON on both (a native debug override of the DRAWN weapon).
-KeepRunning leaves everything up; -StopOnly just tears down.

Teardown kills only what a capture can own: the -Profile client image this
script launches, and whatever listens on -Port. An operator's own release
client is not ours to kill.

(-Steps is gone. It described synthetic key presses and mouse buttons, which
is exactly what may no longer happen here; it is replaced by -ScriptA plus
-Shots.)
#>
param(
    [int]$Port = 7778,
    [string]$Lobby = "v18cap",
    [string]$Map = "",
    [string]$Mode = "",
    [string]$Cam = "",
    [int]$Weapon = 0,
    [int]$Clients = 2,
    [string]$Out = "tools/v18",
    [string]$Prefix = "cap",
    [string]$ScriptA = "wait 3",
    [string]$ScriptB = "wait 3600",
    [object[]]$Shots = @("idle@1.5"),   # 'a@1.0;b#4' or @('a@1.0','b#4') or @{name='a';at=1.0}; see Read-Shots
    [int]$X = 0,
    [int]$Y = 40,
    [int]$Gap = 860,
    [string]$Profile = "debug",
    [switch]$KeepRunning,
    [switch]$StopOnly
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
# The dev profile by default: on this workstation the release server binary is the live host's running image and cannot be replaced while it runs.
$server = Join-Path $repo "target\$Profile\arena-server.exe"
$client = Join-Path $repo "target\$Profile\arena-app.exe"
$runDir = Join-Path $env:TEMP "ember-v18-capture"
New-Item -ItemType Directory -Force $runDir | Out-Null
# An absolute -Out is taken as it stands, so a run can write outside the repo.
$outDir = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $repo $Out }
New-Item -ItemType Directory -Force $outDir | Out-Null

# A client with no script is the pre-fix client: it grabs the pointer, takes
# the foreground and reads the operator's keyboard and mouse. An empty string
# is a blank timeline in the client and would be safe, but .NET DROPS an
# empty environment variable rather than passing it, so the client would come
# up with EMBER_SCRIPT unset. Refuse both, here, before anything launches.
if ([string]::IsNullOrWhiteSpace($ScriptA)) {
    throw "-ScriptA is empty: a client without EMBER_SCRIPT reads the operator's device and grabs their cursor. Use 'wait 3600' for a client that does nothing."
}
if ($Clients -ge 2 -and [string]::IsNullOrWhiteSpace($ScriptB)) {
    throw "-ScriptB is empty: a client without EMBER_SCRIPT reads the operator's device and grabs their cursor. Use 'wait 3600' for an idle observer."
}

# -Shots, in either form, to a list of @{name; at} / @{name; step}.
function Read-Shots([object[]]$raw) {
    $out = @()
    foreach ($s in $raw) {
        if ($s -is [hashtable]) { $out += , $s; continue }
        # One string may carry the whole list. It has to: a shell that is not
        # PowerShell hands `-Shots 'a','b'` across a `-File` boundary as the
        # single token "a,b", and an array that silently became one string is
        # how the old `-Steps` lost its arguments.
        foreach ($t in ([string]$s).Split(@(';', ','), [StringSplitOptions]::RemoveEmptyEntries)) {
            $t = $t.Trim()
            if ($t -match '^(?<n>[^@#]+)#(?<v>\d+)$') { $out += , @{ name = $Matches.n; step = [int]$Matches.v } }
            elseif ($t -match '^(?<n>[^@#]+)@(?<v>[\d.]+)$') { $out += , @{ name = $Matches.n; at = [double]$Matches.v } }
            else { throw "cannot read shot '$t': write it as name@SECONDS or name#STEP, separated by ';'" }
        }
    }
    return $out
}
$shotList = Read-Shots $Shots

Add-Type -AssemblyName System.Drawing
# Only what a hands-off harness needs: measure a window, place it, read the
# screen. Nothing in here can produce an input event, and nothing may be
# added that can.
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    public static readonly IntPtr TOPMOST = new IntPtr(-1);
    /// Where a raised window goes back to when the shots are done: a capture
    /// borrows the operator's screen space, it does not keep it.
    public static readonly IntPtr NOTOPMOST = new IntPtr(-2);
    /// Raise and place without ever activating: SWP_NOACTIVATE only. No
    /// SWP_SHOWWINDOW - it is ShowWindow by another name, and ShowWindow
    /// hides a winit window.
    public const uint NOACTIVATE = 0x0010;
    /// Keep the position and size a SetWindowPos already set.
    public const uint NOMOVESIZE = 0x0002 | 0x0001;
    /// The visible top-level window of a process whose title starts with "ember", or zero.
    public static IntPtr GameWindow(uint pid) {
        IntPtr found = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint p; GetWindowThreadProcessId(h, out p);
            if (p != pid || !IsWindowVisible(h)) return true;
            var sb = new System.Text.StringBuilder(256);
            GetWindowText(h, sb, 256);
            if (sb.ToString().StartsWith("ember")) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@

# Kill only what a capture can own. Never by image name alone: the operator
# may be playing a release client, or running one from another checkout, and
# that is not ours to stop (the same rule MEMORY records for cloudflared).
function Stop-All {
    Get-Process arena-app -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $client } |
        Stop-Process -Force -ErrorAction SilentlyContinue
    $conn = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    foreach ($c in $conn) { Stop-Process -Id $c.OwningProcess -Force -ErrorAction SilentlyContinue }
}

if ($StopOnly) { Stop-All; Write-Host "stopped"; exit 0 }

$sw = [Diagnostics.Stopwatch]::StartNew()
Stop-All
Start-Sleep -Milliseconds 300

# 1. The server, on loopback.
$srv = Start-Process -FilePath $server -ArgumentList "--bind", "127.0.0.1:$Port" -PassThru -WindowStyle Hidden `
    -RedirectStandardOutput (Join-Path $runDir "server.out") -RedirectStandardError (Join-Path $runDir "server.err")
$deadline = (Get-Date).AddSeconds(8)
while ((Get-Date) -lt $deadline -and -not (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue)) { Start-Sleep -Milliseconds 100 }
if (-not (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue)) { throw "server did not listen on $Port" }
Write-Host ("server up on {0} after {1:n1}s (pid {2})" -f $Port, $sw.Elapsed.TotalSeconds, $srv.Id)

# 2. The clients. A creates the lobby (first person); B joins with the fixed camera.
function Start-Client([string]$handle, [string]$action, [hashtable]$envExtra) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $client
    # The map is the creating client's seventh positional argument and the mode (v19) its eighth (arena-app online URL create LOBBY - HANDLE MAP MODE); a joiner takes the lobby's. The mode needs the map in front of it, so an empty -Map goes as "" (the server's default) when -Mode is set.
    $mapArg = ""
    if ($action -eq "create" -and ($Map -or $Mode)) {
        $mapArg = if ($Map) { " $Map" } else { ' ""' }
        if ($Mode) { $mapArg += " $Mode" }
    }
    $psi.Arguments = "online ws://127.0.0.1:$Port $action $Lobby - $handle$mapArg"
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.WorkingDirectory = $repo
    $psi.EnvironmentVariables["RUST_LOG"] = "info"
    foreach ($k in $envExtra.Keys) { $psi.EnvironmentVariables[$k] = $envExtra[$k] }
    $p = [System.Diagnostics.Process]::Start($psi)
    $log = Join-Path $runDir "$handle.log"
    $null = Register-ObjectEvent -InputObject $p -EventName OutputDataReceived -Action { if ($EventArgs.Data) { Add-Content -Path $Event.MessageData -Value $EventArgs.Data } } -MessageData $log
    $null = Register-ObjectEvent -InputObject $p -EventName ErrorDataReceived -Action { if ($EventArgs.Data) { Add-Content -Path $Event.MessageData -Value $EventArgs.Data } } -MessageData $log
    $p.BeginOutputReadLine(); $p.BeginErrorReadLine()
    return $p
}
Remove-Item (Join-Path $runDir "*.log") -ErrorAction SilentlyContinue
# EMBER_SCRIPT is what makes a client hands-off, so BOTH get one.
$envA = @{ EMBER_SCRIPT = $ScriptA }
if ($Weapon -gt 0) { $envA["EMBER_WEAPON"] = "$Weapon" }
$a = Start-Client "alpha" "create" $envA
Start-Sleep -Seconds 2
$b = $null
if ($Clients -ge 2) {
    # The observer draws remote players with the same override, or its third-person capture would show the real (sidearm) gun.
    $envB = @{ EMBER_SCRIPT = $ScriptB }
    if ($Weapon -gt 0) { $envB["EMBER_WEAPON"] = "$Weapon" }
    if ($Cam) { $envB["EMBER_CAM"] = $Cam }
    $b = Start-Client "bravo" "join" $envB
}

function Get-Rect([System.Diagnostics.Process]$p) {
    $h = [Win]::GameWindow([uint32]$p.Id)
    if ($h -eq [IntPtr]::Zero) { throw "client $($p.Id) has no game window" }
    $r = New-Object Win+RECT
    [void][Win]::GetWindowRect($h, [ref]$r)
    return @{ h = $h; l = $r.L; t = $r.T; w = ($r.R - $r.L); ht = ($r.B - $r.T) }
}
# Wait for the window, then place it and raise it WITHOUT activating it.
function Place-Client([System.Diagnostics.Process]$p, [int]$x) {
    $deadline = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $deadline -and [Win]::GameWindow([uint32]$p.Id) -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 100 }
    $r = Get-Rect $p
    [void][Win]::SetWindowPos($r.h, [Win]::TOPMOST, $x, $Y, $r.w, $r.ht, [Win]::NOACTIVATE)
    return $r.h
}
# Give the screen space back. Still never activates, still never ShowWindow.
function Unraise([IntPtr]$h) {
    if ($h -ne [IntPtr]::Zero) { [void][Win]::SetWindowPos($h, [Win]::NOTOPMOST, 0, 0, 0, 0, [Win]::NOACTIVATE -bor [Win]::NOMOVESIZE) }
}
function Capture-Client([System.Diagnostics.Process]$p, [string]$path) {
    $r = Get-Rect $p
    $bmp = New-Object System.Drawing.Bitmap $r.w, $r.ht
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.l, $r.t, 0, 0, $bmp.Size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

$hA = Place-Client $a $X
$hB = [IntPtr]::Zero
if ($b) { $hB = Place-Client $b ($X + $Gap) }
# A window appears BEFORE the GPU context is built, so the window is not the
# starting gun: the client logs "EMBER_SCRIPT starts" on the frame its first
# step runs, and the shot clock starts once every client has said it. An
# earlier version timed from the window instead and photographed three
# identical frames of a client that had not begun.
function Wait-Log([string]$handle, [string]$pattern, [int]$seconds) {
    $log = Join-Path $runDir "$handle.log"
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path $log) {
            $m = Select-String -Path $log -Pattern $pattern | Select-Object -First 1
            if ($m) { return $m.Line }
        }
        Start-Sleep -Milliseconds 50
    }
    return $null
}
# How old a log line already was when we read it. The client's stdout reaches
# this script through Register-ObjectEvent handlers that only run when the
# runspace is idle, so a line can be SECONDS old by the time Wait-Log sees it
# (measured 2.0 s on a one-client run). Every `@SECONDS` shot would be that
# much late; the client stamps its own lines, so read the stamp instead.
function Log-Age([string]$line) {
    if ($line -and $line -match '^(\S+Z)\s') {
        $t = [datetime]::Parse($Matches[1], [cultureinfo]::InvariantCulture,
            [System.Globalization.DateTimeStyles]::AdjustToUniversal -bor [System.Globalization.DateTimeStyles]::AssumeUniversal)
        return [int]((Get-Date).ToUniversalTime() - $t).TotalMilliseconds
    }
    return 0
}
# The clock is ALPHA's, because alpha is the client the script drives. B
# comes up two seconds later and loads the same assets, so head a script
# with a `wait` long enough for it, or B's first picture is of a client that
# is not drawing yet; how much of the clock B cost is reported.
$startLine = Wait-Log "alpha" "EMBER_SCRIPT starts" 60
$clock = [Diagnostics.Stopwatch]::StartNew()
$lagMs = 0
if ($startLine) {
    $lagMs = Log-Age $startLine
    if ($lagMs -gt 100) { Write-Host ("alpha's log reached this script {0} ms late; the shot clock is back-dated by that" -f $lagMs) }
}
else { Write-Warning "alpha never logged 'EMBER_SCRIPT starts' - the shot times will not line up" }
# The script clock is ALPHA's own, `$lagMs` ahead of this stopwatch.
function Script-Ms { return [int]$clock.ElapsedMilliseconds + $lagMs }
if ($b) {
    $bLine = Wait-Log "bravo" "EMBER_SCRIPT starts" 30
    if (-not $bLine) { Write-Warning "bravo never logged 'EMBER_SCRIPT starts' - it may not be drawing yet" }
    else {
        $late = ((Script-Ms) - (Log-Age $bLine)) / 1000.0
        if ($late -gt 0.1) { Write-Warning ('bravo started {0:n2}s after alpha''s script did; head the script with a wait that long or B''s first picture is of a client that is not drawing yet' -f $late) }
    }
}

# 3. The photographs, at their moments. A `#N` shot waits for alpha's own log
#    to say that step began, which is the client's clock and cannot drift; an
#    `@S` shot waits on this wall clock, which can.
try {
    # In the ORDER YOU GAVE THEM, so the two forms interleave correctly. A
    # wall-clock moment already past is taken at once and said so.
    foreach ($s in $shotList) {
        if ($s.ContainsKey('at')) {
            $left = [int](1000 * [double]$s.at) - (Script-Ms)
            if ($left -gt 0) { Start-Sleep -Milliseconds $left }
            else { Write-Warning ("'{0}' at {1}s was already past when its turn came; taken at once" -f $s.name, $s.at) }
            $when = ("{0:n2}s" -f ((Script-Ms) / 1000.0))
        }
        else {
            $n = [int]$s.step
            if (-not (Wait-Log "alpha" "EMBER_SCRIPT step begins step=$n\b" 120)) { Write-Warning "alpha never reached step $n; '$($s.name)' is a photograph of whatever it was doing instead" }
            $when = ("step {0} at {1:n2}s" -f $n, ((Script-Ms) / 1000.0))
        }
        Capture-Client $a (Join-Path $outDir "$Prefix-$($s.name)-A.png")
        if ($b) { Capture-Client $b (Join-Path $outDir "$Prefix-$($s.name)-B.png") }
        Write-Host ("captured {0} at {1} ({2:n1}s wall)" -f $s.name, $when, $sw.Elapsed.TotalSeconds)
    }
}
finally {
    # Whatever happened above, the operator gets their screen back.
    Unraise $hA
    Unraise $hB
}

# 4. Evidence from the logs: that each client took its script at all (a
#    client that did not is a client that would read the operator's device),
#    that the script PARSED (an empty timeline is hands-off but photographs a
#    client standing still, and says so nowhere else), and the status lines,
#    which carry the ammo count and so prove a shot.
#    Wait for alpha's script to finish first, bounded: the last photograph is
#    often taken BEFORE the last step runs, and reading the log at that moment
#    shows a magazine that has not been fired yet. Measured: a `fire 0.6` two
#    steps past the final shot reported 8/8 and looked like a dud.
if (-not (Wait-Log "alpha" "EMBER_SCRIPT is spent" 8)) { Write-Host "alpha's script was still running 8s after the last shot; the status lines below are from mid-script" }
Start-Sleep -Milliseconds 500
foreach ($h in @("alpha", "bravo")) {
    $log = Join-Path $runDir "$h.log"
    if (-not (Test-Path $log)) { continue }
    $lines = Get-Content $log
    $drives = $lines | Select-String -Pattern "EMBER_SCRIPT drives this client" | Select-Object -Last 1
    if ($drives) { Write-Host "--- $h is script-driven (hands-off)" }
    else { Write-Warning "$h never reported EMBER_SCRIPT: it is reading the DEVICE. Fix that before the next run." }
    if ($lines | Select-String -Pattern "EMBER_SCRIPT did not parse" -Quiet) {
        Write-Warning "$h's script DID NOT PARSE - it stood still for the whole run and every picture of it is worthless. The client logs the reason above."
    }
    elseif ($drives -and $drives.Line -match 'steps=(\d+)' -and [int]$Matches[1] -eq 0) {
        Write-Warning "$h's script has no steps: it stood still for the whole run."
    }
    $spent = $lines | Select-String -Pattern "EMBER_SCRIPT is spent" | Select-Object -Last 1
    if ($spent) { Write-Host "    script finished" }
    $status = $lines | Select-String -Pattern "status=" | Select-Object -Last 3
    $status | ForEach-Object { Write-Host ("    " + $_.Line.Substring([Math]::Max(0, $_.Line.IndexOf('status=')))) }
}
if (-not $KeepRunning) {
    foreach ($p in @($a, $b, $srv)) { if ($p) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } }
    Stop-All
}
Write-Host ("done in {0:n1}s; logs in {1}" -f $sw.Elapsed.TotalSeconds, $runDir)
