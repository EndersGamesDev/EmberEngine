# Exercise the destructive boundary with mocked process APIs. The deployment
# script's top level is never executed and no real process is queried/stopped.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$clock = [Diagnostics.Stopwatch]::StartNew()
$tokens = $null
$parseErrors = $null
$source = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../deploy-fire-local.ps1'))
$ast = [Management.Automation.Language.Parser]::ParseFile($source, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count) { throw ($parseErrors | Out-String) }
foreach ($definition in $ast.FindAll({
    param($node)
    $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -in @('Get-OwnedProcess', 'Stop-OwnedProcess')
}, $true)) { Invoke-Expression $definition.Extent.Text }

$sampleStart = [datetime]::UtcNow
$script:fakeProcess = [pscustomobject]@{ Id = 123; Path = 'C:\fire\fire-server.exe'; StartTime = $sampleStart }
$script:fakeProcess | Add-Member ScriptMethod WaitForExit { param($Timeout) return $true }
$script:fakeCommand = 'C:\fire\fire-server.exe 127.0.0.1:7781 --name test-host'
$script:stopped = @()
function Get-Process { param($Id, $ErrorAction) return $script:fakeProcess }
function Get-CimInstance { param($ClassName, $Filter) return [pscustomobject]@{ CommandLine = $script:fakeCommand } }
function Stop-Process { param($Id, [switch]$Force) $script:stopped += $Id }
$record = [pscustomobject]@{
    Id = 123
    ExecutablePath = 'C:\fire\fire-server.exe'
    StartedUtcTicks = $sampleStart.Ticks.ToString()
    CommandToken = '127.0.0.1:7781'
}

function Assert-Refused([string]$Reason) {
    $refused = $false
    try { Stop-OwnedProcess $record } catch { $refused = $true }
    if (!$refused) { throw "Ownership guard accepted $Reason" }
    if ($script:stopped.Count -ne 0) { throw "Stopped a process despite $Reason" }
}

if ((Get-OwnedProcess $record).Id -ne 123) { throw 'Matching process was refused.' }
$script:fakeCommand = 'C:\fire\fire-server.exe 127.0.0.1:77810 --name test-host'
Assert-Refused 'a different port sharing the same prefix'
$script:fakeCommand = 'C:\fire\fire-server.exe 127.0.0.1:7781'
$script:fakeProcess.Path = 'C:\arena\arena-server.exe'
Assert-Refused 'a different executable'
$script:fakeProcess.Path = 'C:\fire\fire-server.exe'
$script:fakeProcess.StartTime = $sampleStart.AddSeconds(1)
Assert-Refused 'a recycled PID'
$script:fakeProcess.StartTime = $sampleStart
Stop-OwnedProcess $record
if ($script:stopped.Count -ne 1 -or $script:stopped[0] -ne 123) { throw 'Exact owned process was not stopped once.' }
$script:fakeProcess = $null
Stop-OwnedProcess $record
if ($script:stopped.Count -ne 1) { throw 'A dead process record stopped an unrelated process.' }

$launches = $ast.FindAll({
    param($node)
    $node -is [Management.Automation.Language.CommandAst] -and $node.GetCommandName() -eq 'Start-Process'
}, $true)
foreach ($launch in $launches) {
    if ($launch.Extent.Text -notmatch '-WindowStyle Hidden') { throw 'A helper could open a foreground window.' }
}
Write-Output ('PASS: Fire deploy parser, exact process ownership, stale PID/port rejection, hidden launches. Wall time {0:F2}s.' -f $clock.Elapsed.TotalSeconds)
