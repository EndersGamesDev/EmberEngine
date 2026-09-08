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
        Copy-Item -LiteralPath (Join-Path 'assets/end-game/v6' $name) -Destination (Join-Path 'web/games/end-game/v10' $name)
    }
    foreach ($name in @('voice-lines.js','boss-intro.wav','boss-phase2.wav','boss-defeat.wav','escape-clue.wav','escape-ending.wav','castle-ambience.wav')) {
        Copy-Item -LiteralPath (Join-Path 'assets/end-game/v10' $name) -Destination (Join-Path 'web/games/end-game/v10' $name)
    }
    cargo test --locked -p end-game-core -p end-game --lib
    if ($LASTEXITCODE -ne 0) { throw 'Dungeon simulation tests failed' }
    node --test tools/end-game/quality.test.mjs tools/end-game/dialogue.test.mjs tools/end-game/guard.test.mjs tools/end-game/castle.test.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Resolution or dialogue tests failed' }
    node --check web/games/end-game/v10/main.js
    if ($LASTEXITCODE -ne 0) { throw 'Game shell syntax failed' }
    node --check web/games/end-game/v10/dialogue.js
    if ($LASTEXITCODE -ne 0) { throw 'Dialogue player syntax failed' }
    foreach ($name in @('castle-audio.js','castle-ui.js','voice-lines.js')) {
        node --check (Join-Path 'web/games/end-game/v10' $name)
        if ($LASTEXITCODE -ne 0) { throw 'Castle shell syntax failed' }
    }
    cargo build --locked --target wasm32-unknown-unknown --release -p end-game --lib
    if ($LASTEXITCODE -ne 0) { throw 'End Game WASM build failed' }
    wasm-bindgen --target web --no-typescript --out-dir web/games/end-game/v10/pkg (Join-Path $artifacts 'wasm32-unknown-unknown/release/end_game.wasm')
    if ($LASTEXITCODE -ne 0) { throw 'End Game WASM bindings failed' }
    New-Item -ItemType Directory -Force -Path web/pkg | Out-Null
    Copy-Item -LiteralPath web/games/end-game/v10/pkg/end_game.js,web/games/end-game/v10/pkg/end_game_bg.wasm -Destination web/pkg
    Write-Output ('WASM bytes: ' + (Get-Item web/games/end-game/v10/pkg/end_game_bg.wasm).Length)
} finally {
    Pop-Location
    Write-Output ('Wall seconds: ' + [math]::Round($watch.Elapsed.TotalSeconds, 2))
}
