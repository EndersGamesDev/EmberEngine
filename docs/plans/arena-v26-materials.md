# Arena v26: first material realism pass

## Scope and coordination

Branch `codex/graphics-polish`, based on main `00982ed`. Client/renderer presentation only; protocol 20, movement, level collision and the running v25 server stay unchanged. Root owns the renderer API/shader and release; workers own CPU mip filtering, Arena material presets, and independent headless capture/review. Keep the original dirty checkout untouched. No shared-target Cargo builds run concurrently.

## First changes

Add opt-in scalar roughness/metallic controls per instance, preserving legacy lighting when unspecified or the outdoor environment is disabled. Use camera-dependent GGX/Smith/Schlick direct specular and a bounded analytic sky-gradient reflection, with matte concrete, painted dielectric steel and exposed metal authored separately. Harbor water should reflect in dry weather. Preserve existing color palette, lighting and texture data for the first A/B. No additional render pass, sampled texture, asset download or WebGL extension is required. This is not full PBR texture support, scene reflection, ray tracing or a photorealism claim. Reference for the BRDF equations: [Filament design](https://google.github.io/filament/main/filament.html).

Correct the CPU mip chain: average sRGB RGB samples in linear light, then encode to sRGB; alpha stays linear. Use cached exact transfer tables instead of per-pixel powers. Equal black/white samples should become 188, not 128. The final color-output pipeline already performs the correct sRGB conversions and must not acquire an extra gamma transform.

## Gates before release

Native unit tests, strict Clippy, explicit production-shader GPU pixel checks, all five published WASM builds and deployment fixtures. Inspect matched headless WebGL2 harbor before/after views, plus both older maps; preserve shadows, rain, HUD readability and draw-call budgets. Record elapsed time, actual adapter and what remains unverified. Test roughness, metallic, view dependence, disabled-environment compatibility and malformed scalar inputs. No server restart for this compatible visual-only change. Publish only after gates, preserving frozen v25.

## Deliberately deferred

Contact occlusion, normal maps, per-texel material maps, improved character geometry, local reflection probes and higher-detail asset replacement require their own measured passes. Do not hide flat materials with blanket contrast, saturation, sharpening or a gamma filter.

## Executed verification — 2026-09-05

Implemented scalar surfaces, linear-light mip filtering, authored Arena presets and a lower-crane-column paint match. The latter derives a cached linear-space tint from the actual upper palette and lower neutral tile, not an independently guessed blue. Geometry, texture source files, weather, simulation and protocol remain unchanged. First-person hands/weapons and HUD intentionally retain their existing lighting.

The final native suite passed 704 tests across Arena/core/server, engine, editor, Fire, Kings, the diagnostic game and Julibrot (90.98 s, 9 opt-in tests ignored). Strict all-target Clippy passed for engine/Arena/core/server/Fire/Kings/editor in 4.10 s. The initial full run failed a Julibrot source-text assertion because Windows automatic checkout conversion had changed an included file from LF to CRLF: its exact three-newline pattern matched zero instead of two. Serial execution confirmed it was not shared test state. Normalizing the isolated checkout to the repository's existing LF rule fixed it; no lab source or assertion was changed or disabled. No source diffs outside this release were staged.

Five explicit production-pipeline GPU tests passed in 5.48 s: the existing sky/cloud/shadow/wetness/particle and eight isolated-receiver checks, plus material roughness, metallic/instance isolation and camera-dependence tests. Review caught an over-large GGX denominator safety floor at minimum roughness; the corrected stable denominator now passes a low-light 0.08-versus-0.10 peak regression. Disabled-environment images remain byte-identical for all material endpoints.

All five release WASM bundles and bindings built in 10.93 s. The 15 final Harbor views used the actual unmodified shader at 1600×900 on WebGL2/ANGLE Intel UHD 770: all 101 draws per checkpoint, no GL errors or warnings (18.55 s harness, 18.78 s command). Matched v25 baseline captures used the previously published bundle; central stacks, quay/ship and warehouse were compared, with final paint-seam improvement independently reviewed. Final Freight Yard and Trench City browser smoke passed in 11.98 s (12.19 s command), joining a private local test server and drawing moving sky/weather; Freight Yard reported one 102 ms startup stall, so no frame-rate target or low-end performance certification is claimed. Pages assembly fixtures passed 41/41 in 6.59 s; shell syntax passed 91/91 in 2.50 s.

The optional release-mode CPU benchmark measured 1024² full mip-chain preparation at 5.776 ms versus 2.617 ms for the old encoded-byte average (16 warm runs; 42.47 s including a cold release-test build). This is a startup CPU cost, not GPU upload or per-frame timing. Texture memory and texture count remain unchanged; each instance upload grows by eight bytes. Reports and images are machine-local under ignored `target/graphics-before`, `graphics-gpu`, `graphics-release` and `graphics-release-old-maps`; reuse `tools/v25/browser-harbor.cjs` and `tools/v22/browser-environment-smoke.cjs` with `EMBER_QA_OUTPUT` to reproduce.

The public server's read-only Welcome still reports protocol 20, host `dusky-osprey`, r1397/00982ed; no restart is needed and its player is not disconnected. Source branch and Barza carry the coordination update; no Fable acknowledgement or PR review is claimed. Publication and public artifact-hash verification follow these pre-release gates.

## Remaining visual limits

This is a modest material-fidelity pass, not a photorealistic transformation. Existing thin-edge/contact-shadow aliasing, weak local occlusion, low-detail meshes and mixed-material base-color atlases remain visible. Local/contact occlusion and authored normal/material detail are the next recommended passes; they are recorded in `docs/plans/backlog.md`. No new optional GPU feature, asset download or toolkit was necessary for this pass.
