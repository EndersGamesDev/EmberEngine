# Isolated Windows script fixtures. Every process/task operation is intercepted;
# only files beneath this test's temporary directory are written.
$ErrorActionPreference='Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass='Idle'
$watch=[Diagnostics.Stopwatch]::StartNew()
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$temp=Join-Path ([IO.Path]::GetTempPath()) ('league-windows-test-'+[Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($temp) | Out-Null
$oldProfile=$env:USERPROFILE
$utf8=[Text.UTF8Encoding]::new($false)
$checks=0
function Check($condition,[string]$message) { if(-not $condition){throw $message}; $script:checks++; Write-Output ('PASS '+$message) }
function Refuses([scriptblock]$body,[string]$pattern) { try { & $body | Out-Null } catch { Check ($_.Exception.Message -match $pattern) ('rejects '+$pattern); return }; throw ('Expected rejection: '+$pattern) }
function Reset-Fixture {
    $env:USERPROFILE=Microsoft.PowerShell.Management\Join-Path $temp ([Guid]::NewGuid().ToString('N'))
    [IO.Directory]::CreateDirectory($env:USERPROFILE) | Out-Null
    $global:leagueFixture=@{registered=[Collections.Generic.List[object]]::new();started=[Collections.Generic.List[string]]::new();processes=@();children=[Collections.Generic.List[object]]::new();probes=[Collections.Generic.List[object]]::new();published=[Collections.Generic.List[object]]::new();existing=@();listening=$false;probeProtocol=1;probeExit=0;failPublic=$false}
}
function Fixture-Listeners { if($global:leagueFixture.listening){[pscustomobject]@{Port=7784}} }
# Alias precedence keeps the production helper's implementation from querying
# real ports, including when the script defines its own helper function.
Set-Alias -Name Get-LeagueListeners -Value Fixture-Listeners
function Join-Path {
    param([string]$Path,[string]$ChildPath)
    if($ChildPath -eq 'target/release/examples/wsprobe.exe'){return $global:leagueFixturePaths.probe}
    Microsoft.PowerShell.Management\Join-Path -Path $Path -ChildPath $ChildPath
}
function Get-Command { param([string]$Name) if($Name -ne 'node'){throw ('Unexpected command lookup '+$Name)}; [pscustomobject]@{Source=$global:leagueFixturePaths.node} }
function Get-ScheduledTask { param($TaskName) if($global:leagueFixture.existing -contains $TaskName){[pscustomobject]@{TaskName=$TaskName}} }
function New-ScheduledTaskTrigger { param([switch]$AtLogOn,$User) [pscustomobject]@{User=$User;Delay=$null;AtLogOn=$AtLogOn.IsPresent} }
function New-ScheduledTaskSettingsSet { param([switch]$AllowStartIfOnBatteries,[switch]$DontStopIfGoingOnBatteries,$ExecutionTimeLimit,$MultipleInstances,$RestartCount,$RestartInterval) [pscustomobject]$PSBoundParameters }
function New-ScheduledTaskAction { param($Execute,$Argument,$WorkingDirectory) [pscustomobject]$PSBoundParameters }
function Register-ScheduledTask { param($TaskName,$Action,$Trigger,$Settings) $global:leagueFixture.registered.Add([pscustomobject]$PSBoundParameters) }
function Start-ScheduledTask { param($TaskName) $global:leagueFixture.started.Add($TaskName);$global:leagueFixture.listening=$true }
function Get-CimInstance { param($ClassName,$Filter) if($ClassName -ne 'Win32_Process'){throw 'Unexpected CIM class'}; $global:leagueFixture.processes }
function Start-Sleep { param($Seconds,$Milliseconds) }
function Start-Transcript { param($Path,[switch]$Force) }
function Stop-Transcript { }
function Start-Process {
    param($FilePath,$ArgumentList,$WindowStyle,$RedirectStandardOutput,$RedirectStandardError,[switch]$PassThru)
    $child=[pscustomobject]@{Id=424242;PriorityClass='Idle';HasExited=$false;ExitCode=0;Killed=$false;FilePath=$FilePath;Arguments=$ArgumentList;ErrorLog=$RedirectStandardError;WindowStyle=$WindowStyle}
    $child | Add-Member ScriptMethod WaitForExit { $this.HasExited=$true }
    $child | Add-Member ScriptMethod Kill { $this.Killed=$true;$this.HasExited=$true }
    $child | Add-Member ScriptMethod Dispose { }
    [IO.File]::WriteAllText($RedirectStandardError,'Your quick Tunnel has been created https://fixture-only.trycloudflare.com Registered tunnel connection url:'+($ArgumentList -join ' '),$global:leagueFixturePaths.utf8)
    $global:leagueFixture.children.Add($child)
    $child
}
try {
    $probe=Microsoft.PowerShell.Management\Join-Path $temp 'probe.ps1'
    [IO.File]::WriteAllText($probe,@'
$global:leagueFixture.probes.Add(@($args))
$global:LASTEXITCODE=if($global:leagueFixture.failPublic -and $args[0] -like 'wss:*'){1}else{$global:leagueFixture.probeExit}
'wsprobe: league protocol v'+$global:leagueFixture.probeProtocol+' healthy in 0.1s'
'@,$utf8)
    $node=Microsoft.PowerShell.Management\Join-Path $temp 'node.ps1'
    [IO.File]::WriteAllText($node,'$global:leagueFixture.published.Add(@($args)); $global:LASTEXITCODE=0',$utf8)
    $global:leagueFixturePaths=@{probe=$probe;node=$node;utf8=$utf8}
    $binary=Microsoft.PowerShell.Management\Join-Path $temp 'target/release/league-server.exe'
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($binary)) | Out-Null
    [IO.File]::WriteAllText($binary,'fixture bytes only',$utf8)
    $sha=(Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
    $install=Microsoft.PowerShell.Management\Join-Path $root 'deploy/league-install-windows.ps1'
    $server=Microsoft.PowerShell.Management\Join-Path $root 'deploy/league-server-windows.ps1'
    $tunnel=Microsoft.PowerShell.Management\Join-Path $root 'deploy/league-tunnel-windows.ps1'
    $common=@{Repository=$temp;Version='r1700';Commit=('a'*40);ExpectedSha256=$sha}
    $v3=@{Port=7784;TaskVersion='v3';Protocol=2;HostName='dusky-osprey-league-v3'}
    Reset-Fixture
    & $install @common | Out-Null
    Check (($leagueFixture.started -join ',') -eq 'ember-league-server-v1,ember-league-tunnel-v1') 'legacy task names remain unchanged'
    Check ($leagueFixture.registered[0].Action.Argument -match ' -Port 7783 -TaskVersion v1 -HostName dusky-osprey$') 'legacy server action defaults to port 7783'
    Check ($leagueFixture.registered[1].Action.Argument -match ' -Protocol 1 -Publish$') 'legacy tunnel action defaults to protocol 1'
    Check ($leagueFixture.probes[0][0] -eq 'ws://127.0.0.1:7783') 'legacy proof uses the legacy origin'
    Reset-Fixture;$leagueFixture.probeProtocol=2;$leagueFixture.existing=@('ember-league-server-v1','ember-league-tunnel-v1')
    & $install @common @v3 | Out-Null
    Check (($leagueFixture.started -join ',') -eq 'ember-league-server-v3,ember-league-tunnel-v3') 'v3 install never touches existing v1 tasks'
    Check ($leagueFixture.registered[0].Action.Argument -match ' -Port 7784 -TaskVersion v3 -HostName dusky-osprey-league-v3$') 'v3 server action preserves port and host'
    Check ($leagueFixture.registered[1].Action.Argument -match ' -Protocol 2 -Publish$') 'v3 future tunnel action preserves protocol 2'
    Check ($leagueFixture.registered[1].Trigger.Delay -eq 'PT30S' -and $leagueFixture.registered[0].Settings.MultipleInstances -eq 'IgnoreNew' -and $leagueFixture.registered[0].Settings.RestartInterval.TotalHours -eq 1) 'durable task settings and delayed tunnel trigger survive'
    Reset-Fixture;$leagueFixture.existing=@('ember-league-server-v3')
    Refuses { & $install @common @v3 } 'Task already exists'
    Check ($leagueFixture.registered.Count -eq 0) 'duplicate installation registers nothing'
    Reset-Fixture;$leagueFixture.listening=$true
    Refuses { & $install @common @v3 } 'port 7784 is occupied'
    Check ($leagueFixture.registered.Count -eq 0) 'occupied selected port registers nothing'
    Reset-Fixture
    Refuses { & $install @common @v3 } 'loopback match/protocol proof'
    Check ($leagueFixture.registered.Count -eq 1 -and $leagueFixture.started.Count -eq 1) 'protocol mismatch leaves only its new server task for inspection'
    Refuses { & $install @common -TaskVersion v3 -Protocol 2 -HostName dusky-osprey-league-v3 } 'legacy runtime'

    Reset-Fixture
    & $server -Binary $binary -ExpectedSha256 $sha | Out-Null
    Check ($leagueFixture.children[0].Arguments[0] -eq '127.0.0.1:7783' -and $leagueFixture.children[0].ErrorLog.Replace('\','/') -like '*/.ember/league-local/*') 'legacy server origin and log directory survive'
    Reset-Fixture
    & $server -Binary $binary -ExpectedSha256 $sha -Port 7784 -TaskVersion v3 -HostName dusky-osprey-league-v3 | Out-Null
    Check ($leagueFixture.children[0].Arguments[0] -eq '127.0.0.1:7784' -and $leagueFixture.children[0].ErrorLog.Replace('\','/') -like '*/.ember/league-local-v3-7784/*') 'v3 server uses separate origin and state'

    $tunnelCommon=@{Repository=$temp;Version='r1700';Commit=('a'*40);Probe=$probe;Node=$node;Publish=$true}
    Reset-Fixture;$leagueFixture.listening=$true
    & $tunnel @tunnelCommon | Out-Null
    Check ($leagueFixture.children[0].Arguments[2] -eq 'http://127.0.0.1:7783') 'legacy tunnel still uses port 7783'
    Check ($leagueFixture.published[0][-1] -eq '--protocol=1') 'legacy publisher receives protocol 1'
    Check (Test-Path -LiteralPath (Join-Path $env:USERPROFILE '.ember/league-local/ready.json')) 'legacy ready file path survives'
    Reset-Fixture;$leagueFixture.listening=$true;$leagueFixture.probeProtocol=2
    $leagueFixture.processes=@([pscustomobject]@{ProcessId=11200;CommandLine='cloudflared tunnel --url http://127.0.0.1:7783 --no-autoupdate'})
    & $tunnel @tunnelCommon @v3 | Out-Null
    Check ($leagueFixture.children.Count -eq 1 -and $leagueFixture.children[0].Arguments[2] -eq 'http://127.0.0.1:7784') 'v3 tunnel ignores the existing legacy origin'
    Check ($leagueFixture.published[0][-1] -eq '--protocol=2' -and $leagueFixture.published[0][-2] -eq 'dusky-osprey-league-v3') 'v3 publication uses its own host and protocol'
    $state=Join-Path $env:USERPROFILE '.ember/league-local-v3-7784'
    $ready=Get-Content -LiteralPath (Join-Path $state 'ready.json') -Raw | ConvertFrom-Json
    Check ($ready.port -eq 7784 -and $ready.protocol -eq 2 -and $ready.taskVersion -eq 'v3') 'v3 proof state records its selected identity'
    Refuses { & $tunnel @tunnelCommon @v3 } 'cooling down'
    Check ($leagueFixture.children.Count -eq 1) 'v3 cooldown cannot mint a second tunnel'
    Reset-Fixture;$leagueFixture.listening=$true;$leagueFixture.probeProtocol=2
    $leagueFixture.processes=@([pscustomobject]@{ProcessId=999;CommandLine='cloudflared tunnel --url http://127.0.0.1:7784 --no-autoupdate'})
    Refuses { & $tunnel @tunnelCommon @v3 } 'already running'
    Check ($leagueFixture.children.Count -eq 0 -and $leagueFixture.probes.Count -eq 0) 'existing own tunnel is refused before any probe or process'
    Reset-Fixture;$leagueFixture.listening=$true
    Refuses { & $tunnel @tunnelCommon @v3 } 'loopback match/protocol proof'
    Check ($leagueFixture.children.Count -eq 0 -and -not (Test-Path -LiteralPath (Join-Path $env:USERPROFILE '.ember/league-local-v3-7784/last-mint.txt'))) 'mismatched protocol consumes no mint attempt'
    Reset-Fixture;$leagueFixture.listening=$true;$leagueFixture.probeProtocol=2;$leagueFixture.failPublic=$true
    Refuses { & $tunnel @tunnelCommon @v3 } 'Public League match probe failed'
    Check ($leagueFixture.children.Count -eq 1 -and $leagueFixture.children[0].Killed) 'public proof failure kills only the child created by this invocation'
    $failure=Get-Content -LiteralPath (Join-Path $env:USERPROFILE '.ember/league-local-v3-7784/latest-failure.json') -Raw | ConvertFrom-Json
    Check ($failure.port -eq 7784 -and $failure.protocol -eq 2 -and $leagueFixture.published.Count -eq 0) 'failed v3 proof remains inspectable and publishes nothing'

    # Extract only the two pure log/recovery functions, never the task body.
    $tokens=$null;$errors=$null
    $ast=[Management.Automation.Language.Parser]::ParseFile($tunnel,[ref]$tokens,[ref]$errors)
    foreach($name in @('Read-LeagueTunnelLog','Use-LeagueSetupRecovery')){
        $definition=$ast.FindAll({param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst]},$false) | Where-Object Name -eq $name
        . ([scriptblock]::Create($definition.Extent.Text))
    }
    Reset-Fixture
    $recoveryDir=Join-Path $env:USERPROFILE '.ember/league-local-v3-7784'
    [IO.Directory]::CreateDirectory($recoveryDir) | Out-Null
    $mint=[DateTime]::UtcNow
    $recoveryLog=Join-Path $recoveryDir ('tunnel-'+$mint.ToString('yyyyMMddTHHmmssfff')+'.err.log')
    $logText='Your quick Tunnel has been created https://fixture-only.trycloudflare.com Registered tunnel connection url:http://127.0.0.1:7784]'
    $writer=[IO.File]::Open($recoveryLog,[IO.FileMode]::Create,[IO.FileAccess]::Write,[IO.FileShare]::Read)
    try {
        $payload=$utf8.GetBytes($logText);$writer.Write($payload,0,$payload.Length);$writer.Flush()
        Check ((Read-LeagueTunnelLog -LogPath $recoveryLog) -eq $logText) 'active redirected log permits shared reading'
    } finally { $writer.Dispose() }
    Refuses { Use-LeagueSetupRecovery -LogPath $recoveryLog -RunDirectory $recoveryDir -LastMint $mint } 'prove creation and registration'
    Check (-not (Test-Path -LiteralPath (Join-Path $recoveryDir 'setup-recovery-used.json'))) 'wrong recovery origin consumes no marker'
    [IO.File]::WriteAllText($recoveryLog,($logText+' 429 too many requests'),$utf8)
    Refuses { Use-LeagueSetupRecovery -LogPath $recoveryLog -RunDirectory $recoveryDir -LastMint $mint -OriginPort 7784 } 'cannot bypass rate limiting'
    [IO.File]::WriteAllText($recoveryLog,$logText,$utf8)
    $recovered=Use-LeagueSetupRecovery -LogPath $recoveryLog -RunDirectory $recoveryDir -LastMint $mint -OriginPort 7784
    Check ($recovered -eq $recoveryLog -and (Test-Path -LiteralPath (Join-Path $recoveryDir 'setup-recovery-used.json'))) 'v3 recovery consumes only its own permanent marker'
    Refuses { Use-LeagueSetupRecovery -LogPath $recoveryLog -RunDirectory $recoveryDir -LastMint $mint -OriginPort 7784 } 'already exists'
    $otherDir=Join-Path $env:USERPROFILE '.ember/league-local'
    [IO.Directory]::CreateDirectory($otherDir) | Out-Null
    Refuses { Use-LeagueSetupRecovery -LogPath $recoveryLog -RunDirectory $otherDir -LastMint $mint -OriginPort 7784 } 'own tunnel stderr log'
    Write-Output ('Windows deployment fixtures: '+$checks+' checks passed in '+$watch.Elapsed.TotalSeconds+' seconds')
} finally {
    $env:USERPROFILE=$oldProfile
    Remove-Item Alias:Get-LeagueListeners -ErrorAction SilentlyContinue
    $resolved=[IO.Path]::GetFullPath($temp)
    if([IO.Path]::GetDirectoryName($resolved) -ne [IO.Path]::GetTempPath().TrimEnd('\','/') -or [IO.Path]::GetFileName($resolved) -notlike 'league-windows-test-*'){throw 'Unsafe fixture cleanup path'}
    Remove-Item -LiteralPath $resolved -Recurse -Force
    Remove-Variable -Name leagueFixture -Scope Global -ErrorAction SilentlyContinue
    Remove-Variable -Name leagueFixturePaths -Scope Global -ErrorAction SilentlyContinue
}
