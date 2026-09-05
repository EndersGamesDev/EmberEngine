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

## Status

Plan only at initial push. Implementation and visual/runtime verification pending.
