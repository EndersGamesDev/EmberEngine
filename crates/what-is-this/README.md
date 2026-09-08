# What Is This diagnostic client

`what-is-this` is the browser diagnostic and benchmark client that measures CPU, floating-point, memory, WebGPU compute, presentation, and selected Julibrot workloads, then derives a deterministic evidence-backed personality from the report.

The wasm page consumes its inventories, progress, status, and verdict JSON; canonical submission uses `ember-client-net` and the frozen `ember-game-what-is-this-v1` contract rather than inventing a second report schema.

The rlib keeps kernel inventories, report interpretation, and page contracts testable natively, while GPU and surface paths remain wasm-only owners with explicit unavailable and device-loss outcomes.

The shared device floor and measurement cautions live in [`../../docs/minimum-requirements.md`](../../docs/minimum-requirements.md) and the relevant GPU/Julibrot design records; repository layering remains governed by [`../../CLAUDE.md`](../../CLAUDE.md).
