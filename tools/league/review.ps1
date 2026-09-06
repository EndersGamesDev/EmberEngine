<#
.SYNOPSIS
Hands-off visual review of Ultimate League: one practice client against
bots, drafted and fought by the sim itself, photographed at given seconds.

.DESCRIPTION
The house rule is that nothing an agent runs may move the operator's
cursor, synthesise a key or a click, or take the foreground. This script
therefore sends the client NO input at all. A practice match needs none:
the draft clock runs out on its own (60 s), every seat becomes a bot, and
the bots play the lane. The camera is moved with LEAGUE_CAM (read by
crates/league/src/scene.rs on native builds only):

  auto     follow the closest pair of opposing champions - the fight
  x,z      a fixed focus, e.g. "0,16" for the North Court
  slot:N   ride the champion in roster seat N

The window is raised TOPMOST without activation and placed at -X/-Y so it
is not under the operator's work; a screenshot is a read of the SCREEN,
so anything dragged over it lands in the picture. It is dropped back to
NOTOPMOST when the shots are done. The client is started with the
engine's activate:false (native), which the script checks in the source
before it launches anything.

.EXAMPLE
pwsh -File tools/league/review.ps1 -Mode squad -Shots "75,110,160" -Out tools/league/review

.NOTES
Build first: cargo build -p league --bin league-app   (debug profile)
Wall time is printed; a run with four shots at the defaults takes ~3.5 min.
#>
param(
    [ValidateSet("duel", "squad")] [string]$Mode = "duel",
    [string]$Cam = "auto",
    # Seconds after launch, comma or space separated, always with a dot
    # for decimals: the list is parsed invariantly, so a German-locale host
    # (comma = decimal point) cannot fold "66,80" into one number.
    [string]$Shots = "70,95,130,190",
    [string]$Out = "tools/league/review",
    [string]$Prefix = "",
    [int]$X = 0,
    [int]$Y = 40,
    [string]$Profile = "debug",
    [switch]$KeepRunning,
    [switch]$StopOnly
)

$ErrorActionPreference = "Stop"
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$inv = [System.Globalization.CultureInfo]::InvariantCulture
$shotTimes = @($Shots -split '[,; ]+' | Where-Object { $_ -ne "" } | ForEach-Object { [double]::Parse($_, $inv) } | Sort-Object)
if ($shotTimes.Count -eq 0) { throw "-Shots is empty; give seconds after launch, e.g. 66,80,100" }
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$client = Join-Path $repo "target\$Profile\league-app.exe"
$runDir = Join-Path $env:TEMP "ember-league-review"
New-Item -ItemType Directory -Force $runDir | Out-Null
$pidFile = Join-Path $runDir "league-app.pid"
$outDir = if ([System.IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $repo $Out }
New-Item -ItemType Directory -Force $outDir | Out-Null

Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class LWin {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    public static readonly IntPtr TOPMOST = new IntPtr(-1);
    public static readonly IntPtr NOTOPMOST = new IntPtr(-2);
    // SWP_NOACTIVATE only. Never SWP_SHOWWINDOW: it is ShowWindow by another
    // name, and ShowWindow hides a winit window.
    public const uint NOACTIVATE = 0x0010;
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

# Stop only the client this script started (its pid is on file). Never by
# image name: the operator may be running one from another checkout.
function Stop-Ours {
    if (Test-Path $pidFile) {
        $id = [int](Get-Content $pidFile -ErrorAction SilentlyContinue | Select-Object -First 1)
        $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($proc -and $proc.ProcessName -eq "league-app") {
            Stop-Process -Id $id -Force
            Write-Host "stopped league-app pid $id"
        }
        Remove-Item $pidFile -ErrorAction SilentlyContinue
    }
}
if ($StopOnly) { Stop-Ours; exit 0 }

if (-not (Test-Path $client)) {
    throw "no client at $client - build it first: cargo build -p league --bin league-app"
}
# The rule, checked at the source: a native client must open without
# taking the foreground. If someone flips it back, this script refuses.
$libSrc = Join-Path $repo "crates\league\src\lib.rs"
$opensQuietly = Select-String -Path $libSrc -Pattern 'activate:\s*(false|cfg!\(target_arch\s*=\s*"wasm32"\))' -Quiet
if (-not $opensQuietly) {
    throw "crates/league/src/lib.rs does not open its window with activate:false on native; a review client would steal the operator's focus. Refusing to launch."
}
Stop-Ours

$args = @()
if ($Mode -eq "squad") { $args = @("squad") }
$stdout = Join-Path $runDir "league-app.out.log"
$stderr = Join-Path $runDir "league-app.err.log"
Remove-Item $stdout, $stderr -ErrorAction SilentlyContinue
# Children inherit the environment; set, launch, unset.
$env:LEAGUE_CAM = $Cam
if (-not $env:RUST_LOG) { $env:RUST_LOG = "info" }
try {
    if ($args.Count -gt 0) {
        $p = Start-Process -FilePath $client -ArgumentList $args -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    } else {
        $p = Start-Process -FilePath $client -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
    }
} finally {
    Remove-Item Env:\LEAGUE_CAM -ErrorAction SilentlyContinue
}
Set-Content $pidFile $p.Id
$t0 = Get-Date
Write-Host ("league-app pid {0} started ({1}, LEAGUE_CAM={2}) at {3:HH:mm:ss}" -f $p.Id, $Mode, $Cam, $t0)

function Get-Rect([System.Diagnostics.Process]$proc) {
    $h = [LWin]::GameWindow([uint32]$proc.Id)
    if ($h -eq [IntPtr]::Zero) { throw "client $($proc.Id) has no game window" }
    $r = New-Object LWin+RECT
    [void][LWin]::GetWindowRect($h, [ref]$r)
    return @{ h = $h; l = $r.L; t = $r.T; w = ($r.R - $r.L); ht = ($r.B - $r.T) }
}
function Capture([System.Diagnostics.Process]$proc, [string]$path) {
    $r = Get-Rect $proc
    $bmp = New-Object System.Drawing.Bitmap $r.w, $r.ht
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.l, $r.t, 0, 0, $bmp.Size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

$h = [IntPtr]::Zero
$taken = @()
try {
    # wait for the window, then place and raise it WITHOUT activating it
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline -and [LWin]::GameWindow([uint32]$p.Id) -eq [IntPtr]::Zero) {
        if ($p.HasExited) { throw "league-app exited before it opened a window; see $stderr" }
        Start-Sleep -Milliseconds 100
    }
    $r = Get-Rect $p
    $h = $r.h
    [void][LWin]::SetWindowPos($h, [LWin]::TOPMOST, $X, $Y, $r.w, $r.ht, [LWin]::NOACTIVATE)
    Write-Host ("window {0}x{1} placed at {2},{3} after {4:N1} s" -f $r.w, $r.ht, $X, $Y, ((Get-Date) - $t0).TotalSeconds)

    foreach ($at in $shotTimes) {
        $wait = $at - ((Get-Date) - $t0).TotalSeconds
        if ($wait -gt 0) { Start-Sleep -Milliseconds ([int][Math]::Round($wait * 1000)) }
        if ($p.HasExited) { throw "league-app exited at $at s; see $stderr" }
        $name = "{0}{1}-{2:000}s.png" -f $Prefix, $Mode, [int]$at
        $path = Join-Path $outDir $name
        Capture $p $path
        $taken += $path
        Write-Host ("shot {0} at {1:N1} s" -f $name, ((Get-Date) - $t0).TotalSeconds)
    }
} finally {
    if ($h -ne [IntPtr]::Zero) {
        [void][LWin]::SetWindowPos($h, [LWin]::NOTOPMOST, 0, 0, 0, 0, ([LWin]::NOACTIVATE -bor [LWin]::NOMOVESIZE))
    }
    if (-not $KeepRunning) { Stop-Ours }
}

Write-Host "--- client log tail ($stderr)"
if (Test-Path $stderr) { Get-Content $stderr -Tail 12 }
Write-Host ("--- {0} shot(s) in {1}; wall time {2:N1} s" -f $taken.Count, $outDir, $sw.Elapsed.TotalSeconds)
$taken | ForEach-Object { Write-Host "  $_" }
