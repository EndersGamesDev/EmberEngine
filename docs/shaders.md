# Repository shader policy

The repository renders production WGSL at runtime from the Rust types, enum discriminants, binding numbers and named constants used by the CPU. Templates may own shader-private control flow and stage plumbing, but a declaration or value that the CPU uploads, decodes or uses to construct a pipeline comes from `ember-shader` metadata rather than a second handwritten fact.

## Upstream and mechanism

`crates/labs/julibrot/shader` remains the design upstream. Mechanism changes originate there and are copied into `crates/ember-shader`; the shared crate never becomes a Julibrot dependency and does not diverge without the corresponding upstream change. The shared crate derives layouts through Rust offsets, sizes and alignments, validates native renders with naga, audits every registered declaration against exact render-local emission bytes, and leaves wasm module validation to wgpu. Feature-full Minijinja supports runtime generation while its debug machinery remains outside release builds.

Every shared production template has one absolute `::ember_shader::production_template_test!` invocation in its owning crate. The deployment checker authenticates that reserved extern-prelude name through locked Cargo metadata, confirms the derived test in the compiled test listing, and runs the exact production renderer normally and with a nonce-bearing invalid-WGSL probe.

## Closed migration debt

`deploy/tests/test-shaders.sh` remains the single shell-suite entry point. It delegates `crates/labs/julibrot` to Julibrot's checker and scans every other tracked package, including frozen packages under `games/`, for raw `.wgsl` files, WGSL `include_str!` calls, stage-bearing literals and untyped `ShaderSource::Wgsl` lowerings. The repository allowlist opened at forty reviewed records, including test fixtures because configuration is not an exemption; its executable ceiling is the exact opening set, so it can only shrink toward zero.

|Row|Owner|Order and evidence|
|---|-----|------------------|
|`SH-ENGINE`|`ember-engine`|First shared production proof: scene and present shaders leave the allowlist together after native naga validation, structural pins and GPU pixel readback.|
|`SH-WHAT-IS-THIS`|`what-is-this`|Migrate the diagnostic compute and progress renderer after the engine proof.|
|`SH-HEAP`|`ember-lab-heap`|Migrate heap kernel templates, generated stages and their test fixtures after what-is-this.|
|`SH-LAYER`|`ember-lab-layer`|Migrate layer's generated compute and demo renderer last, reusing the heap generator boundary where applicable.|

## Measured cost

|Commit|Configuration|arena release wasm bytes|Change|Interpretation|
|------|-------------|-----------------------:|-----:|--------------|
|`317681d1`|Shared metadata present; no shared renderer/validator|44,934,485|baseline|Measured on sokol.|
|`37512931`|Shared rendering and native-only validation present; engine still does not depend on `ember-shader`|44,898,595|\-35,890|Measured on osprey. Because arena cannot reach the new crate at either commit and the hosts differ, this is build-host or toolchain variance rather than attributable shader cost; the merge candidate is remeasured on one gate host.|
