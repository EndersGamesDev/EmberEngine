<#
.SYNOPSIS
The v18 capture harness: a local server, one or two native clients, synthetic
input, and screenshots of the client windows.

.DESCRIPTION
Nothing of the v16/v17 harness was committed; this is the rewrite the v18
plan (section 9.4) budgets. What the earlier sessions learned and this
script encodes:

  * keybd_event / SendInput keyboard input reaches the winit window;
  * SetForegroundWindow from a background script is refused by the Windows
    foreground lock, so a window is focused by a synthetic CLICK at its
    centre (which also captures the mouse, as the game asks for);
  * a second client is told where to look with EMBER_CAM, the first is left
    in first person, and both windows are captured in one pass so a
    screenshot pair proves the input landed on the right side.

Usage (from the repo root, after `cargo build -p arena -p arena-server --bins`; -Profile release for release binaries):

    powershell -ExecutionPolicy Bypass -File tools/v18/capture.ps1 -Out tools/v18 -Prefix smoke `
        -Steps @(@{name='idle';wait=1.5}, @{name='melee';key='E';wait=0.25}, @{name='shot';mouse=300;wait=0.05})

Each step focuses client A, optionally presses a key (virtual-key name from
[System.Windows.Forms.Keys]) or holds the left mouse button for N ms, waits,
and captures every client window to <Out>/<Prefix>-<name>-<client>.png.
-Map picks the lobby's map (v18 servers), -Cam pins the observer camera,
-Weapon sets EMBER_WEAPON on client A (a native debug override of the drawn
weapon, when the client supports it). -KeepRunning leaves everything up.
#>
param(
    [int]$Port = 7778,
    [string]$Lobby = "v18cap",
    [string]$Map = "",
    [string]$Cam = "",
    [int]$Weapon = 0,
    [int]$Clients = 2,
    [string]$Out = "tools/v18",
    [string]$Prefix = "cap",
    [object[]]$Steps = @(@{name = 'idle'; wait = 1.5 }),
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
New-Item -ItemType Directory -Force (Join-Path $repo $Out) | Out-Null

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    public const uint LEFTDOWN = 0x0002, LEFTUP = 0x0004, KEYUP = 0x0002;
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
    # The map is the creating client's seventh positional argument (arena-app online URL create LOBBY - HANDLE MAP); a joiner takes the lobby's.
    $mapArg = if ($Map -and $action -eq "create") { " $Map" } else { "" }
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
$envA = @{}
if ($Weapon -gt 0) { $envA["EMBER_WEAPON"] = "$Weapon" }
$a = Start-Client "alpha" "create" $envA
Start-Sleep -Seconds 2
$b = $null
if ($Clients -ge 2) {
    # The observer draws remote players with the same override, or its third-person capture would show the real (sidearm) gun.
    $envB = @{}
    if ($Weapon -gt 0) { $envB["EMBER_WEAPON"] = "$Weapon" }
    if ($Cam) { $envB["EMBER_CAM"] = $Cam }
    $b = Start-Client "bravo" "join" $envB
}
Start-Sleep -Seconds 3

function Get-Rect([System.Diagnostics.Process]$p) {
    $h = [Win]::GameWindow([uint32]$p.Id)
    if ($h -eq [IntPtr]::Zero) { throw "client $($p.Id) has no game window" }
    $r = New-Object Win+RECT
    [void][Win]::GetWindowRect($h, [ref]$r)
    return @{ h = $h; l = $r.L; t = $r.T; w = ($r.R - $r.L); ht = ($r.B - $r.T) }
}
# The two windows open at the same spot; side by side, a click lands on the one it is aimed at and a capture holds one client only.
function Place-Client([System.Diagnostics.Process]$p, [int]$x) {
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline -and [Win]::GameWindow([uint32]$p.Id) -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 100 }
    $r = Get-Rect $p
    [void][Win]::MoveWindow($r.h, $x, 40, $r.w, $r.ht, $true)
}
Place-Client $a 0
if ($b) { Place-Client $b 860 }
Start-Sleep -Milliseconds 400
# A synthetic click at the window's centre focuses it and captures the mouse. Never ShowWindow(SW_SHOW) a winit window from outside: measured on 2026-09-03, it HIDES the window (IsWindowVisible flips to false and stays there).
function Focus-Client([System.Diagnostics.Process]$p) {
    $r = Get-Rect $p
    [void][Win]::SetCursorPos($r.l + [int]($r.w / 2), $r.t + [int]($r.ht / 2))
    [Win]::mouse_event([Win]::LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [Win]::mouse_event([Win]::LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 250
    return ([Win]::GetForegroundWindow() -eq $r.h)
}
function Capture-Client([System.Diagnostics.Process]$p, [string]$path) {
    $r = Get-Rect $p
    $bmp = New-Object System.Drawing.Bitmap $r.w, $r.ht
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.l, $r.t, 0, 0, $bmp.Size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}
function Press-Key([string]$name, [int]$holdMs) {
    $vk = [byte][int][System.Windows.Forms.Keys]::$name
    [Win]::keybd_event($vk, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds $holdMs
    [Win]::keybd_event($vk, 0, [Win]::KEYUP, [UIntPtr]::Zero)
}

# 3. The steps.
$focused = Focus-Client $a
Write-Host ("client A focused: {0}" -f $focused)
foreach ($s in $Steps) {
    $name = $s.name
    # `key` with `hold` holds one key; `tap` at `tapAt` ms presses a second key (Space) while the first is held, for a running jump.
    if ($s.ContainsKey('key')) {
        $hold = $(if ($s.ContainsKey('hold')) { $s.hold } else { 80 })
        if ($s.ContainsKey('tap')) {
            $vk = [byte][int][System.Windows.Forms.Keys]::($s.key)
            [Win]::keybd_event($vk, 0, 0, [UIntPtr]::Zero)
            Start-Sleep -Milliseconds $s.tapAt
            Press-Key $s.tap 60
            Start-Sleep -Milliseconds ([Math]::Max(0, $hold - $s.tapAt - 60))
            [Win]::keybd_event($vk, 0, [Win]::KEYUP, [UIntPtr]::Zero)
        } else {
            Press-Key $s.key $hold
        }
    }
    if ($s.ContainsKey('mouse')) {
        [Win]::mouse_event([Win]::LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds $s.mouse
        [Win]::mouse_event([Win]::LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
    }
    if ($s.ContainsKey('wait')) { Start-Sleep -Milliseconds ([int](1000 * $s.wait)) }
    # A movement step (nocap) only drives the player; nothing is captured.
    if ($s.ContainsKey('nocap')) { continue }
    Capture-Client $a (Join-Path $repo (Join-Path $Out "$Prefix-$name-A.png"))
    if ($b) { Capture-Client $b (Join-Path $repo (Join-Path $Out "$Prefix-$name-B.png")) }
    Write-Host ("captured {0} at {1:n1}s" -f $name, $sw.Elapsed.TotalSeconds)
}

# 4. Evidence from the logs: the status lines carry the ammo count.
Start-Sleep -Milliseconds 500
foreach ($h in @("alpha", "bravo")) {
    $log = Join-Path $runDir "$h.log"
    if (Test-Path $log) {
        $lines = Get-Content $log | Select-String -Pattern "status=" | Select-Object -Last 3
        Write-Host "--- $h last status lines:"; $lines | ForEach-Object { Write-Host ("    " + $_.Line.Substring([Math]::Max(0, $_.Line.IndexOf('status=')))) }
    }
}
if (-not $KeepRunning) { Stop-All }
Write-Host ("done in {0:n1}s; logs in {1}" -f $sw.Elapsed.TotalSeconds, $runDir)
