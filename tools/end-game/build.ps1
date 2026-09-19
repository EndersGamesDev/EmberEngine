param([string]$TargetDir)
$ErrorActionPreference = 'Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
$watch = [Diagnostics.Stopwatch]::StartNew()
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location -LiteralPath $root
try {
    if ($TargetDir) { $env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($TargetDir) }
    $artifacts = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
    foreach ($name in @('warden-movement.wav','warden-unlocking.wav','warden-sword.wav','warden-death.wav')) {
        Copy-Item -LiteralPath (Join-Path 'assets/end-game/v6' $name) -Destination (Join-Path 'web/games/end-game/v12' $name)
    }
    # voice-lines is no longer copied from the asset tree: the live module is the
    # checked template web/games/end-game/v12/voice-lines.ts.j2, and dropping a
    # stale .js beside it would leave a second answer to the same question.
    foreach ($name in @('boss-intro.wav','boss-phase2.wav','boss-defeat.wav','escape-clue.wav','escape-ending.wav','castle-ambience.wav')) {
        Copy-Item -LiteralPath (Join-Path 'assets/end-game/v10' $name) -Destination (Join-Path 'web/games/end-game/v12' $name)
    }
    cargo test --locked -p end-game-core -p end-game --lib
    if ($LASTEXITCODE -ne 0) { throw 'Dungeon simulation tests failed' }
    # The five End Game suites and the six live modules are templates now, so
    # neither has a file that node can read directly. The typed web gate renders
    # them, compiles both projects and runs the suites by emitted path, which is
    # the same route CI takes; a local syntax check would only duplicate a
    # weaker part of it.
    $env:EMBER_TYPED_WEB_REQUIRED = '1'
    bash deploy/tests/test-typescript.sh
    if ($LASTEXITCODE -ne 0) { throw 'Typed web gate failed' }
    cargo build --locked --target wasm32-unknown-unknown --release -p end-game --lib
    if ($LASTEXITCODE -ne 0) { throw 'End Game WASM build failed' }
    wasm-bindgen --target web --no-typescript --out-dir web/games/end-game/v12/pkg (Join-Path $artifacts 'wasm32-unknown-unknown/release/end_game.wasm')
    if ($LASTEXITCODE -ne 0) { throw 'End Game WASM bindings failed' }
    New-Item -ItemType Directory -Force -Path web/pkg | Out-Null
    Copy-Item -LiteralPath web/games/end-game/v12/pkg/end_game.js,web/games/end-game/v12/pkg/end_game_bg.wasm -Destination web/pkg
    Write-Output ('WASM bytes: ' + (Get-Item web/games/end-game/v12/pkg/end_game_bg.wasm).Length)
} finally {
    Pop-Location
    Write-Output ('Wall seconds: ' + [math]::Round($watch.Elapsed.TotalSeconds, 2))
}
