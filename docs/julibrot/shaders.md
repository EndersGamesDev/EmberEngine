# Julibrot shader templates

Julibrot owns a lab-local rule: WGSL submitted by its presentation and kernel crates is rendered at runtime from an embedded Minijinja `.wgsl.jinja` template by `ember-julibrot-shader`. The wasm carries templates rather than pre-rendered source, so native tests and the browser execute the same rendering before their target-specific validation steps.

This lane's scope is the Julibrot lab, its docs and `deploy/tests`, plus the root Cargo workspace manifest, lockfile and changelog metadata required to integrate the crate. Engine and game sources remain outside this implementation.

## Why Rust owns the interface

A handwritten Rust layout and a separate WGSL declaration can drift without either compiler seeing the disagreement. The first visible failure may be a rejected device pipeline, a wrong binding, or valid bytes decoded through the wrong layout. Rendering the GPU interface from the Rust type, enum and binding items used by the CPU makes that disagreement mechanically observable before wgpu receives the source.

Runtime rendering is deliberate. It keeps context-dependent shader construction in the wasm execution path, avoids generated source artifacts and build-script state, and lets a pipeline owner retain the exact validated source associated with its context. Templates are embedded with `include_str!`; rendered WGSL is not checked in as production source.

## Mechanism

`crates/labs/julibrot/shader` owns the Minijinja environment, the production template registry, `WgslType` and `WgslEnum` descriptions, `ShaderContext`, declaration filters, stable source hashing and Naga 24 parsing and validation. It has no global cache or mutable static state.

Release builds keep Minijinja's complete shader code-generation set: built-ins, deserialization, macros, multiple templates, adjacent loop items, standard collections, Serde and loop controls. Only `debug` leaves the release wasm; native shader tests enable it directly and present tests enable the crate's `template-debug` feature so diagnostics retain template source context. `json`, `urlencode`, `unicode`, `custom_syntax`, `fuel`, `loader`, `preserve_order` and `speedups` remain disabled because no shader template needs them, and each is added when a template does.

The Rust owner registers every CPU-visible shader struct, enum discriminant, bind-group and binding number, and named ABI or layout constant needed by a template. Struct metadata is compile-time type-checked against each Rust field. Scalar and explicitly aligned vector mappings carry compile-time size and alignment equalities; raw array and matrix mappings are not accepted, and three-lane vectors use an explicitly padded wrapper whose storage size is emitted as `@size(16)`. Each declaration filter records its exact rendered emission; native validation audits every registered item and rejects any CPU-visible declaration that does not overlap the corresponding emission. A pipeline creation retains one `RenderedShader`, including its stable hash, beside the pipeline. A changed context is rendered and retained as a new value.

Templates may hand-write control flow, arithmetic, entry-point bodies and shader-private stage plumbing. They may not hand-write a type, enum value, binding number or named constant that the CPU stores, uploads, decodes or uses to construct a layout; those declarations come through context filters from the same Rust items. Inline production WGSL strings and production `.wgsl` files are migration debt, never examples for new code.

Every render uses Minijinja's strict undefined behavior. Native renders then use Naga's WGSL parser and validator, so a syntactic or semantic failure names the embedded template and, when Naga supplies a span, the rendered WGSL line and its text. On wasm, rendering returns the source and stable hash before wgpu validates that source while creating the shader module; the browser therefore carries only the validator path it already needs.

Every template in the production registry must have exactly one `production_template_test!(TEMPLATE_CONST, render_fn)` invocation. The macro generates a native test that renders with the named production context, then passes that exact source through Naga parsing and validation. The deployment checker rejects a missing, duplicated or non-structural pairing. This native coverage makes deferring the browser's validation to wgpu safe.

## Anti-drift oracle

Every native render runs the anti-drift oracle against that render's complete `ShaderContext`. For every registered struct it compares Naga's member names, types, byte offsets, alignment and struct span with Rust metadata derived from the same fields, `offset_of!`, `align_of` and `size_of`; the generic test context includes explicit padding after a `vec3`. It compares every rendered signed and unsigned enum constant with the Rust discriminant, checks every binding and named constant, requires each CPU-visible declaration to come from its traced filter, and proves that an unregistered type fails with that type's name.

The first production proof applies that oracle to `PaletteUniform`, the aligned GPU wire type constructed byte-for-byte from the real `PaletteRecord`, and to the real `PaletteId` used by `ember-julibrot-present`. `present-shade.wgsl.jinja` takes their declarations, status sentinels, diagnostic colours, light limits and all three bind slots from CPU-side items. The presenter renders once while creating GPU state and retains the `RenderedShader` beside the shade pipeline. Its complete rendered FNV-1a hash is `855da4bc80e3906e`; a test proves the wire conversion preserves every palette byte, removes the context-rendered declarations, substitutes their symbolic uses with the former literals, and compares the result byte for byte, including the final newline, with an exact fixture of the former inline source.

## Measured bundle cost

Sizes are measured on the release `ember_lab_julibrot.wasm` before wasm-bindgen. The baseline is commit `1e81e358`; subsequent rows isolate the template feature set and the direct validation path.

| Build stage | Bytes | Delta from baseline | Included mechanism |
|---|---:|---:|---|
| Baseline | 6,870,688 B | 0 B | No runtime shader-template mechanism |
| Default set | 7,620,854 B | +750,166 B (+10.9%) | Minijinja's default features, including `debug`, plus direct Naga validation |
| Code-generation set without debug | 7,653,093 B | +782,405 B (+11.4%) | The full dynamic code-generation feature set, including loop controls, because generated shaders need the complete template language; `debug` is excluded from release |
| Native-only validation and declaration audit (reviewed lane tip) | 7,640,932 B | +770,244 B (+11.2%) | The same feature-full Minijinja build without `debug`; wgpu owns wasm validation, while the runtime declaration audit verifies Rust-rendered provenance on every target |

Layout hardening measured 7,615,863 B at `64a04bef`. Declaration-provenance auditing measured 7,633,778 B at `f5f0ff9e`, a 17,915-byte increase. Exact emission-byte verification adds 7,154 B; the 7,640,932 B figure was measured at the lane's reviewed tip. The merge commit message records that tip's SHA together with the merge candidate's own measured size after the series bump, making the reachable merge record the durable provenance for both measurements.

`arena.wasm` remains outside the Julibrot dependency graph. Its baseline is 44,934,492 B; the full gate supplies the post-change confirmation.

## Migration order

The allowlist used by `deploy/tests/test-shaders.sh` cites these row IDs and may only shrink as the pending rows migrate. Test configuration is not an exemption: a test-only shader source and the function that lowers it to wgpu carry a reviewed test row just like the existing present fixture.

R3-04 landed three records on main at `7d8b1cb4` between this lane's base and merge: one production source, `RECONSTRUCTION_BODY`, plus the paired-output test fixture and its lowering. The production migration ceiling therefore grows by exactly one source; the two test-only records remain explicit because test fixtures do not bypass the boundary.

| Row | Source | Crate | State and owner |
|---|---|---|---|
| `JB-PRESENT-SHADE` | `shader/templates/present-shade.wgsl.jinja` replacing `present/src/shade_shader.rs` inline source | `ember-julibrot-shader`, consumed by `ember-julibrot-present` | Complete; rendered hash `855da4bc80e3906e`, normalized source equals the legacy fixture |
| `JB-PRESENT-SCENE` | `present/src/shader.rs` scene and glitch-count inline sources | `ember-julibrot-present` | Next Julibrot presentation-template lane |
| `JB-PRESENT-WARP` | `present/src/warp_shader.rs` | `ember-julibrot-present` | Next Julibrot presentation-template lane |
| `JB-PRESENT-NATIVE-TEST` | `present/src/gpu/device/tests.rs` split-value shader fixture | `ember-julibrot-present` | Migrates with the remaining presentation templates |
| `JB-APP-PAIRED-TEST` | `app/src/frame/loop/tests.rs` paired-output readback fixture and direct wgpu lowering | `ember-julibrot-app` | Test-only R3-04 evidence; migrates with the paired reconstruction template |
| `JB-KERNEL-SHALLOW` | `kernels/src/shallow.wgsl` | `ember-julibrot-kernels` | Julibrot kernel-template follow-up |
| `JB-KERNEL-PERTURB` | `kernels/src/perturb.wgsl` | `ember-julibrot-kernels` | Julibrot kernel-template follow-up |
| `JB-KERNEL-DIALECT` | `kernels/src/dialect.rs`, including heap dialect v2 validation | `ember-julibrot-kernels` | Migrates with the two kernel templates |
| `JB-KERNEL-RECONSTRUCTION` | `kernels/src/gpu.rs` `RECONSTRUCTION_BODY` paired reconstruction pass | `ember-julibrot-kernels` | Production R3-04 source; the kernel-template follow-up owns its rendered form |

Julibrot keeps this implementation. Once the lab's shader design is complete, a later lane may re-implement the lessons as a separate shared facility for the engine, the other labs and games, with games last. That shared facility will not replace this crate, and those consumers will not depend on `ember-julibrot-shader`.
