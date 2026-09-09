# Repository shader policy

The repository renders production WGSL at runtime from the Rust types, enum discriminants, binding numbers and named constants used by the CPU. Templates may own shader-private control flow and stage plumbing, but a declaration or value that the CPU uploads, decodes or uses to construct a pipeline comes from `ember-shader` metadata rather than a second handwritten fact.

## Upstream and mechanism

`crates/labs/julibrot/shader` remains the design upstream. Mechanism changes originate there and are copied into `crates/ember-shader`; the shared crate never becomes a Julibrot dependency and does not diverge without the corresponding upstream change. The shared crate derives layouts through Rust offsets, sizes and alignments, including the fixed four-element `F32Vec4` arrays used by engine point lights, validates native renders with naga, audits every registered declaration against exact render-local emission bytes, and leaves wasm module validation to wgpu. Feature-full Minijinja supports runtime generation while its debug machinery remains outside release builds.

Every shared production template has one absolute `::ember_shader::production_template_test!` invocation in its owning crate. The deployment checker authenticates that reserved extern-prelude name through locked Cargo metadata, confirms the derived test in the compiled test listing, and runs the exact production renderer normally and with a nonce-bearing invalid-WGSL probe. Every compiled confirmation passes `--locked` with `CARGO_NET_OFFLINE=true`, so it can use only the already-cached locked graph.

The served `arena`, `end-game`, `fire`, `kings`, `league` and `what-is-this` bundles all create their scene and presentation pipelines through these two engine templates. Every frozen client version under `games/` reaches the same current engine renderer through `ember-legacy` rather than owning a private copy of the engine WGSL, so the engine proof is also the port of every served frozen version. The merge candidate must exercise at least one of these game bundles in a browser in addition to the native shader and pixel proofs.

## Closed migration debt

`deploy/tests/test-shaders.sh` remains the single shell-suite entry point. It delegates `crates/labs/julibrot` to Julibrot's checker and scans every other tracked package, including frozen packages under `games/`, for raw `.wgsl` files, WGSL `include_str!` calls, stage-bearing literals and untyped `ShaderSource::Wgsl` lowerings. The repository allowlist opened at forty reviewed records, including test fixtures because configuration is not an exemption; its executable ceiling is the exact opening set, so it can only shrink toward zero. The ceiling is repository-wide rather than per-package: a package added later is scanned and any new unrendered shader debt is refused unless a reviewed change explicitly reopens the ceiling with its reason. The post-ceiling `end-game` and `end-game-core` packages contain no shader findings, so they add no records and receive no exemption.

|Row|Owner|Order and evidence|
|---|-----|------------------|
|`SH-ENGINE`|`ember-engine`|First shared production proof: scene and present shaders leave the allowlist together after native naga validation, structural pins and GPU pixel readback.|
|`SH-WHAT-IS-THIS`|`what-is-this`|Migrate the diagnostic compute and progress renderer after the engine proof.|
|`SH-HEAP`|`ember-lab-heap`|Migrate heap kernel templates, generated stages and their test fixtures after what-is-this.|
|`SH-LAYER`|`ember-lab-layer`|Migrate layer's generated compute and demo renderer last, reusing the heap generator boundary where applicable.|

## Measured cost

|Commit|Configuration|arena release wasm bytes|Change|Interpretation|
|------|-------------|-----------------------:|-----:|--------------|
|`317681d1`|Shared metadata present; no shared renderer/validator|44,934,485|baseline|Measured on sokol before the `62f5c450` rebase.|
|`0d0cf51b`|Engine links the feature-full Minijinja runtime and renders both shipping shaders|45,690,043|+755,558 (+1.68%)|Measured against `317681d1` on the same sokol host. This historical attribution predates the `62f5c450` point-light rebase; the rebased merge candidate requires a fresh same-host row.|

Osprey produced 45,658,978 bytes for the same post-port source, 31,065 bytes below sokol, so the attributed figure uses only the same-host sokol pair rather than mixing build-host variance into the shader cost.
