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
no gamepad, and it never grabs the cursor. The grammar and the reasoning
live in one place, `crates/arena/src/script.rs`; the short of it is that
steps are separated by `;`, a step is a few words and an optional duration
in seconds, and time is the client's own frame clock:

    wait 1.5; walk w 2; sprint w 2; crouch w 2; aim 90; ads fire 0.12; wait 0.5

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
input. Anything the operator drags over them will land in the picture, so
give them a clear corner (-X, -Y, -Gap) or a second monitor.

Usage (from the repo root, after `cargo build -p arena -p arena-server --bins`; -Profile release for release binaries):

    powershell -ExecutionPolicy Bypass -File tools/v18/capture.ps1 -Out tools/v18 -Prefix smoke `
        -ScriptA 'wait 1.5; melee; wait 0.25; fire 0.05; wait 0.5' `
        -Shots @(@{name='idle'; at=1.4}, @{name='melee'; at=1.9}, @{name='shot'; at=2.3})

-ScriptA is client A's timeline; -Shots are the moments to photograph, in
seconds from the moment every client logged "EMBER_SCRIPT starts" (its first
scripted frame — NOT when its window appeared, which is several seconds
earlier), each written to <Out>/<Prefix>-<name>-<client>.png (-Out is
relative to the repo root, or absolute). -ScriptB is client B's, and
defaults to a long `wait` so the observer is hands-off too — an unscripted
client would still ask for the cursor if anything clicked it.

-Map picks the lobby's map (v18 servers), -Mode the lobby's mode (v19
servers: ffa, tdm or hill), -Cam pins client B's observer camera, -Weapon
sets EMBER_WEAPON on both (a native debug override of the DRAWN weapon).
-KeepRunning leaves everything up; -StopOnly just tears down.

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
    [object[]]$Shots = @(@{name = 'idle'; at = 1.5 }),
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
    /// Raise and place without ever activating: SWP_NOACTIVATE only. No
    /// SWP_SHOWWINDOW - it is ShowWindow by another name, and ShowWindow
    /// hides a winit window.
    public const uint NOACTIVATE = 0x0010;
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

function Stop-All {
    Get-Process arena-app -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
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
}
function Capture-Client([System.Diagnostics.Process]$p, [string]$path) {
    $r = Get-Rect $p
    $bmp = New-Object System.Drawing.Bitmap $r.w, $r.ht
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.l, $r.t, 0, 0, $bmp.Size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

Place-Client $a $X
if ($b) { Place-Client $b ($X + $Gap) }
# A window appears BEFORE the GPU context is built, so the window is not the
# starting gun: the client logs "EMBER_SCRIPT starts" on the frame its first
# step runs, and the shot clock starts once every client has said it. An
# earlier version timed from the window instead and photographed three
# identical frames of a client that had not begun.
function Wait-Script([string]$handle, [int]$seconds) {
    $log = Join-Path $runDir "$handle.log"
    $deadline = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $deadline) {
        if ((Test-Path $log) -and (Select-String -Path $log -Pattern "EMBER_SCRIPT starts" -Quiet)) { return $true }
        Start-Sleep -Milliseconds 50
    }
    Write-Warning "$handle never logged 'EMBER_SCRIPT starts' - the shot times will not line up"
    return $false
}
# The clock is ALPHA's, because alpha is the client the script drives. B
# comes up two seconds later and loads the same assets, so head a script
# with a `wait` long enough for it, or B's first picture is of a client that
# is not drawing yet; how much of the clock B cost is reported.
[void](Wait-Script "alpha" 60)
$clock = [Diagnostics.Stopwatch]::StartNew()
if ($b) {
    [void](Wait-Script "bravo" 30)
    $late = $clock.Elapsed.TotalSeconds
    if ($late -gt 0.1) { Write-Warning ("bravo started {0:n2}s after alpha's script did; every shot time is that much late" -f $late) }
}

# 3. The photographs, at their moments.
foreach ($s in ($Shots | Sort-Object { [double]$_.at })) {
    $left = [int](1000 * [double]$s.at) - [int]$clock.ElapsedMilliseconds
    if ($left -gt 0) { Start-Sleep -Milliseconds $left }
    Capture-Client $a (Join-Path $outDir "$Prefix-$($s.name)-A.png")
    if ($b) { Capture-Client $b (Join-Path $outDir "$Prefix-$($s.name)-B.png") }
    Write-Host ("captured {0} at {1:n2}s of script ({2:n1}s wall)" -f $s.name, $clock.Elapsed.TotalSeconds, $sw.Elapsed.TotalSeconds)
}

# 4. Evidence from the logs: that each client took its script at all (a
#    client that did not is a client that would read the operator's device),
#    and the status lines, which carry the ammo count and so prove a shot.
Start-Sleep -Milliseconds 500
foreach ($h in @("alpha", "bravo")) {
    $log = Join-Path $runDir "$h.log"
    if (-not (Test-Path $log)) { continue }
    $lines = Get-Content $log
    $drives = $lines | Select-String -Pattern "EMBER_SCRIPT drives this client" | Select-Object -Last 1
    if ($drives) { Write-Host "--- $h is script-driven (hands-off)" }
    else { Write-Warning "$h never reported EMBER_SCRIPT: it is reading the DEVICE. Fix that before the next run." }
    $spent = $lines | Select-String -Pattern "EMBER_SCRIPT is spent" | Select-Object -Last 1
    if ($spent) { Write-Host "    script finished" }
    $status = $lines | Select-String -Pattern "status=" | Select-Object -Last 3
    $status | ForEach-Object { Write-Host ("    " + $_.Line.Substring([Math]::Max(0, $_.Line.IndexOf('status=')))) }
}
if (-not $KeepRunning) { Stop-All }
Write-Host ("done in {0:n1}s; logs in {1}" -f $sw.Elapsed.TotalSeconds, $runDir)
