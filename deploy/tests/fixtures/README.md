# Deploy test fixtures

These inputs are copied into ignored build staging by shell suites. The missing-variant template deliberately omits one generated protocol case so the pinned compiler must reject it; it contains no Jinja and uses the template suffix only so no bare TypeScript source is tracked. The wasm-bindgen symbol template reproduces the disposal member emitted by the pinned Rust binding tool so the ES2022 compatibility declaration remains necessary and exercised.
