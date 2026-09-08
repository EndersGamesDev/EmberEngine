param([string]$TargetDir)
$ErrorActionPreference = 'Stop'
[Diagnostics.Process]::GetCurrentProcess().PriorityClass = 'Idle'
$watch = [Diagnostics.Stopwatch]::StartNew()
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
Push-Location -LiteralPath $root
try {
    if ($TargetDir) { $env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($TargetDir) }
    $artifacts = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
    cargo test --locked -p end-game-core -p end-game --lib
    if ($LASTEXITCODE -ne 0) { throw 'Dungeon simulation tests failed' }
    node --test tools/end-game/quality.test.mjs
    if ($LASTEXITCODE -ne 0) { throw 'Resolution controller tests failed' }
    node --check web/games/end-game/v5/main.js
    if ($LASTEXITCODE -ne 0) { throw 'Game shell syntax failed' }
    cargo build --locked --target wasm32-unknown-unknown --release -p end-game --lib
    if ($LASTEXITCODE -ne 0) { throw 'End Game WASM build failed' }
    wasm-bindgen --target web --no-typescript --out-dir web/games/end-game/v5/pkg (Join-Path $artifacts 'wasm32-unknown-unknown/release/end_game.wasm')
    if ($LASTEXITCODE -ne 0) { throw 'End Game WASM bindings failed' }
    New-Item -ItemType Directory -Force -Path web/pkg | Out-Null
    Copy-Item -LiteralPath web/games/end-game/v5/pkg/end_game.js,web/games/end-game/v5/pkg/end_game_bg.wasm -Destination web/pkg
    Write-Output ('WASM bytes: ' + (Get-Item web/games/end-game/v5/pkg/end_game_bg.wasm).Length)
} finally {
    Pop-Location
    Write-Output ('Wall seconds: ' + [math]::Round($watch.Elapsed.TotalSeconds, 2))
}
