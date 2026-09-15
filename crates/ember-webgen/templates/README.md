# Production templates

These strict Minijinja templates render type-only boundary declarations, the exhaustive TypeScript consumer used by the release gate and the compiler-only ES2022 symbol declaration required by current wasm-bindgen output. Registered behaviour templates live beside the browser and deploy sources they replace; their canonical paths and bytes also contribute to the stable template-source hash.
