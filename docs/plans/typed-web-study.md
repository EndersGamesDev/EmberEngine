# Typed web generation study

## Decision

The programme should be staged: Stage 1 converts every editable browser, worker and Node script to handwritten pedantic TypeScript kept as a Minijinja template, makes compiler-derived Rust boundary declarations mandatory inputs, renders only into build staging and ships only successful `tsc` output; Stage 2 moves behavior into Rust surface by surface where ownership and measured cost justify it.

Stage 1 delivers the required compile-time guarantee because the registration derive, generated catalog and host-book models, loader shapes and wasm-bindgen declarations are the only boundary types the handwritten logic may import; where the behavior is written does not determine whether a Rust variant addition makes `tsc` fail.

Stage 2 should start with surfaces that speak a protocol or own state transitions, validation or duplicated state, while DOM-only presentation moves last or never; deferring those rewrites avoids paying wasm-size, startup and browser-API costs merely to relocate behavior that Stage 1 already checks against the Rust-owned boundary.

The immutable published pages should stay byte-for-byte frozen and become an explicit historical-artifact exception to the source rule; the rule should govern every editable and newly shipped script, because changing old JavaScript bytes and preserving those same bytes are mutually exclusive requirements.

The first implementation lane is complete: `web/hosts.js`, `web/loader.js`, `deploy/check-hosts.mjs` and all three of their Node suites became legacy TypeScript templates. This is the smallest surface that crosses Rust loader enums, hand-built `JsValue` event objects, wasm-bindgen glue, `games.json`, `server.json`, a browser module, a Node consumer and the Pages deploy gate.

A Fire-first lane would prove one protocol union with fewer page lines, but it would not prove the shared catalog, host book, browser-and-Node module split or deploy gate; hosts plus loader is the smaller credible proof of the whole architecture rather than only its enum renderer.

Landing the loader lane's 528 handwritten JavaScript lines is acceptable only as the integration prerequisite for this first conversion lane: they are migration input, not accepted Stage 1 source, and no later page lane should add another naked JavaScript surface.

## Measured inventory

The stated 128-file, 28,203-line baseline does not describe one tree: `develop` at `0cc4387b` contains 127 tracked `.js`, `.mjs` and `.cjs` files outside every `/pkg/` directory and they total exactly 28,203 newline characters, while fetched `sokol/lane/loader` at `d19f70c9` adds `web/loader.js` and its independent suite. The earlier post-loader figure counted the module but omitted `web/loader.test.mjs`; the phase 1 row below therefore uses a fresh six-file measurement at its actual base rather than extending that incomplete inventory.

The line measure is `wc -l`, which reproduces the 28,203-line baseline; `web/games/fire/v1/index.html` lacks a final newline, so its reported 78 is one below its logical line count.

### Tracked script files on develop

|Role|Exhaustive paths or path classes|Files|Lines|
|----|--------------------------------|----:|----:|
|Live page source|`web/hosts.js`; Arena v31 `settings.js`; End Game v12 `castle-audio.js`, `castle-ui.js`, `dialogue.js`, `main.js`, `quality.js`, `voice-lines.js`; Fire v2 `garage.js`, `race.js`; League v4 `ui.js`|11|3,449|
|Frozen page source|Arena v28–v30 `settings.js`; all 34 End Game v1–v11 scripts; League v2–v3 `ui.js`|39|7,561|
|Game landing source|`web/games/arena/landing.js`, `web/games/league/landing.js`|2|328|
|Lab runtime source|Julibrot `lab.js`, `main.js`, `worker.js`|3|1,181|
|Browser, capture and visual tools|The 24 non-test `tools/**` files named browser, capture, gallery, preview, trailer, UI, visual or draft|24|7,682|
|Browser and network smokes|The 14 non-test `tools/**` files named bindings-smoke, landing-smoke, network, prove, public or public-release|14|2,783|
|Publishers and release helpers|The 11 non-test `tools/**` files named publish, release-book or release-scope|11|1,175|
|Tool asset/data authoring|`tools/v31/build_shotgun.cjs`, `tools/v31/shotgun-events.cjs`|2|232|
|Node tests|17 `tools/**/*.test.{mjs,cjs}` files, `web/hosts.test.mjs`, `deploy/check-hosts.test.mjs`|19|3,384|
|Deploy runtime|`deploy/check-hosts.mjs`|1|395|
|Generated asset source|`assets/end-game/v10/voice-lines.js`|1|33|
|Total|Every tracked `.js`, `.mjs` and `.cjs` outside `/pkg/`|127|28,203|
|Loader branch snapshot|`web/loader.js` and `web/loader.test.mjs` on the in-flight `sokol/lane/loader` at `d19f70c9`|2|1,222|
|Post-loader total|Develop plus that branch delta|129|29,425|

The 39 frozen external scripts break down as Arena v28–v30 at 3 files and 1,137 lines, End Game v1–v11 at 34 files and 4,651 lines, and League v2–v3 at 2 files and 1,773 lines.

The 68 files under `tools/**`, tests included, break down as `end-game` 4/511, `league` 19/3,808, `trailer` 5/832, `v22` 1/144, `v23` 1/214, `v24` 2/477, `v25` 2/607, `v28` 3/746, `v29` 8/1,753, `v30` 10/2,170 and `v31` 13/2,577, where each pair is files/lines.

### Inline scripts

There are 37 tracked HTML files with 51 inline script blocks, 19,295 `wc -l` lines of HTML and 12,842 lines between the script tags; HTML files whose scripts are all external are outside this count.

|Pages|Files|Blocks|HTML lines|Inline lines|
|-----|----:|-----:|---------:|-----------:|
|Current source copied by the full Pages assembly: root hub, Arena v31, Arena v0, Kings v1, what-is-this v1, Julibrot drive|6|7|4,862|3,814|
|Frozen page source: Arena v0 and v7–v30, Fire v1, League v1; Arena v0 is also in the preceding assembly row because the script currently rewrites it|27|40|12,947|7,798|
|Developer lab pages: Heap bench/index/spike, Julibrot whole-grid oracle, Layer index|5|5|1,566|1,248|

The overlap is intentional and exposes a policy defect: `web/games.json` marks Arena 0.0.0 `live: false`, but `deploy/deploy-pages.sh` calls it `ARENA_V0_LIVE`, removes it from the seed and recopies its source and the current Arena wasm at every release; the independent repair is tracked in the [platform web deploy backlog](backlog.md#platform-web-deploy).

The individual page measures are exhaustive below; a repeated range states the per-file count, not a combined count.

|HTML page|Blocks|HTML lines|Inline lines|
|---------|-----:|---------:|-----------:|
|Arena v0|1|80|18|
|Arena v7–v9, each|1|268|142|
|Arena v10|1|269|142|
|Arena v11|1|278|150|
|Arena v12|1|438|289|
|Arena v13|1|452|289|
|Arena v14–v17, each|1|493|309|
|Arena v18|2|565|355|
|Arena v19|2|599|370|
|Arena v20|2|618|370|
|Arena v21|2|627|370|
|Arena v22–v23, each|2|628|370|
|Arena v24|2|626|370|
|Arena v25–v27, each|2|627|370|
|Arena v28|2|557|357|
|Arena v29|2|558|357|
|Arena v30|2|620|375|
|Arena v31|2|627|381|
|Fire v1|1|78|18|
|Kings v1|1|817|517|
|League v1|1|669|456|
|what-is-this v1|1|2,438|2,262|
|Root `web/index.html`|1|772|527|
|Heap `bench.html`|1|322|236|
|Heap `index.html`|1|439|349|
|Heap `spike.html`|1|114|71|
|Julibrot `drive.html`|1|128|109|
|Julibrot `whole-grid-oracle.html`|1|333|314|
|Layer `index.html`|1|358|278|

The current tree has Arena page source for v0 and v7–v31, not every integer v0–v31: Pong v1 is materialized from commit `e7b85e8` during assembly, and other absent historical paths live only in the frozen publication seed.

### What ships

`deploy/deploy-pages.sh` starts from the frozen `gh-pages` publication record, replaces current surfaces, and retains older directories from the seed; source-tree presence alone therefore does not prove that a script ships.

|Assembly treatment|Source scripts selected from this tree|Files|Lines|Boundary|
|------------------|--------------------------------------|----:|----:|--------|
|Root|`web/index.html`, `web/hosts.js`; `games.json` and generated `server.json` data|1 external plus 527 inline lines|1,052 script lines|DOM, host book, catalog, sockets; no wasm in `hosts.js`|
|Arena current|v31 `index.html` and `settings.js`|1 external plus 381 inline lines|847 script lines|DOM, host selection, socket protocol and arena wasm glue|
|Arena v0 exception|v0 `index.html` plus the current Arena generated binding and wasm|18 inline lines|18 script lines|DOM and wasm glue; currently rewritten despite the frozen label|
|Fire current|v2 `race.js`, `garage.js` and `index.html`|2|503|DOM, host selection and Fire wasm glue; the wasm owns the game socket|
|Kings current|v1 `index.html`|517 inline lines|517 script lines|DOM, host selection, socket messages and Kings wasm glue|
|League current|every regular non-`pkg` file under v4, including `ui.js`|1|1,155|DOM, host selection, JSON command bridge, League `Cmd` action shape and wasm glue|
|what-is-this current|v1 `index.html`|2,262 inline lines|2,262 script lines|DOM, diagnostic wasm glue and optional report HTTP; no game socket|
|End Game current|v12 `main.js` and `quality.js` only|2|487|DOM and End Game wasm glue; no socket|
|Julibrot current|`main.js`, `lab.js`, `worker.js`, `index.html`, `drive.html`|3 external plus 109 inline lines|1,290 script lines|DOM, app wasm glue and worker messages; no socket|
|Generated bindings|root and per-page `pkg/*.js` from wasm-bindgen|excluded from the 127-file source inventory|generated|wasm ABI|
|Historical pages|Everything the assembly does not replace|not measurable from develop alone|seed bytes|frozen DOM, wasm and, for old online pages, direct sockets|

The explicit source-copy set is 10 external files and 4,317 lines plus 6 HTML files carrying 3,814 inline lines; generated wasm-bindgen glue and retained seed bytes are additional shipped JavaScript.

The End Game row exposes another existing deploy defect: v12 `main.js` imports `dialogue.js`, `castle-audio.js`, `castle-ui.js` and `voice-lines.js`, totaling 313 lines, while the assembly deletes v12 and copies none of those four modules or ten tracked audio files; the typed pipeline must derive its copy manifest from the module graph and tracked runtime assets or fail on an unresolved import, and the independent repair is tracked in the [platform web deploy backlog](backlog.md#platform-web-deploy).

The two game landing modules are not copied by the full Pages assembler; League has a separate scoped publisher, and the Arena landing page depends on its historical publication path.

Developer-only surfaces are `tools/**`, all `.test` modules, the asset-side voice-line file, the Heap and Layer pages, Julibrot's whole-grid oracle, and any landing publisher or smoke until a separate publication command selects it.

### Boundary map

|Group|Reads or calls|
|-----|--------------|
|Live and frozen online pages|DOM, wasm-bindgen modules, `hosts.js` or inline host discovery, JSON socket frames using `C2S`/`S2C`; League also passes the `Cmd` action shape through its JSON wasm API|
|Offline game pages|DOM and wasm-bindgen modules; End Game also uses media and Web Audio|
|Root hub and landings|DOM, `games.json`, `server.json`, `hosts.js`, story/media files and host probes; no game wasm on the hub|
|Julibrot|DOM, wasm-bindgen modules and worker messages|
|Heap and Layer HTML labs|DOM plus their lab wasm or browser primitives|
|Browser/capture tools|Browser automation against the real DOM and generated wasm, screenshots, traces and local/public HTTP surfaces|
|Network smokes and publishers|HTTP, WebSocket, `games.json`, `server.json`, release trees and scoped publication destinations|
|Node tests|Node assertions, passive DOM/socket/filesystem doubles and the modules under test|
|`deploy/check-hosts.mjs`|`games.json`, `server.json`, dynamically imported `hosts.js`, mirrors and real WebSockets; no DOM or wasm|
|Asset voice lines|Static generated data imported by End Game source; no DOM, socket or wasm call|

## What “no naked JS, then no naked TS” can mean

### Candidate A: checked handwritten TypeScript

Convert each editable `.js`, `.mjs`, `.cjs` and inline block to pedantic TypeScript, require imports from the generated Rust boundary declarations, compile it, and ship only emitted `.js`, `.mjs` and `.cjs`; keep the canonical behavior in `.ts.j2` templates, with `.mts.j2` and `.cts.j2` variants where Node output kind matters, so Minijinja renders untracked compiler inputs rather than leaving bare `.ts` files in the tree.

This is Stage 1 and it delivers the enum guarantee: a Rust variant changes a mandatory generated union in the same artifact build, and an exhaustive consumer fails `tsc` without any handwritten copy of the discriminant list.

### Candidate B: templates without generated contracts

Moving the same page and tool bodies into per-surface `.ts.j2` files and rendering them through Minijinja produces no tracked `.ts`, but without mandatory generated imports a template containing page branches, DOM mutations and messages is only handwritten TypeScript in a different suit.

This is rejected as a standalone end state: the Stage 1 source-AST gate must prevent stringly boundary replicas, unchecked casts and other escapes from the generated contracts.

### Candidate C: Rust-owned behavior and generated adapters

Move protocol handling, state transitions, validation and application behavior into Rust crates compiled either to the existing game wasm, the loader wasm or a small tool binary; describe DOM elements, events and command bindings as Rust data; let a small fixed set of generic templates emit only imports, typed adapter calls, worker startup and Node entry points.

This is Stage 2 where it pays: it removes duplicated state and validation, but it is not necessary for the enum guarantee because Candidate A already compiles every boundary use against the same derived schema.

The honest staged invariants are precise: “no naked JS” means no editable shipped script that is not `tsc` output from a source checked against generated declarations, and “no naked TS” means no handwritten TypeScript outside the explicit `.ts.j2` legacy-template class and no handwritten boundary declaration; legacy behavior templates may remain indefinitely.

Stage 2 candidates qualify when they speak a generated protocol, own consequential state transitions or runtime validation, duplicate behavior across runtimes, or have a measured maintenance defect; DOM-only glue remains a template unless a concrete size, correctness or reuse result makes the Rust move worthwhile.

## Generator design

### Feed comparison

|Feed|Tagged unions and rename rules|Defaults and option fields|Added variant|Stale output|
|----|------------------------------|--------------------------|-------------|------------|
|`serde_reflection` trace|Reject: its documented unsupported idioms include `serde(tag = ...)`, the representation used by all four `C2S`/`S2C` enums and League `Cmd`|It observes Serde formats but does not carry the directional absent-versus-null contract needed here|Every enum must be traced separately and incomplete coverage is a trace error|A registry file can still be stale unless generation and shipping are one command|
|Direct `syn` parse of `proto.rs`|Can read the literal `tag` and `rename_all` attributes, but duplicates Serde's attribute semantics and does not see type resolution, expanded macros or active `cfg` by itself|Can spot `Option` and `serde(default)` syntax, but must reimplement skip, rename, flatten and custom-default behavior|A parsed variant is visible automatically|Impossible only if deploy renders from the parsed current checkout into staging; a checked-in result can drift|
|Registration derive on each boundary type|Recommended: the compiler expands it on the same enum item and it rejects unsupported Serde shapes; use Serde's attribute parser rather than a second casing table|Records wire direction, nullable and absent `Option`, defaulted-deserialize omission for other field types and serialized omission separately; trait bounds resolve registered nested types|The derive emits the new variant descriptor in the same compile and a variant-count test forces a new wire sample|Impossible when deploy consumes only this invocation's staging output; possible if generated files are checked in or copied from `web/`|
|wasm-bindgen `.d.ts`|Exact for exported ABI functions and exported wasm-bindgen classes/enums, but JSON strings and `JsValue` remain opaque and therefore reveal none of `C2S`, `S2C`, `Cmd` or loader event fields|Exact only for types wasm-bindgen understands; it cannot infer Serde omission or a manually assembled `Reflect` object|Only an exported wasm-bindgen signature change appears|Fresh when emitted beside the same wasm invocation; stale if an old `pkg` directory is reused|

The feed should therefore be the registration derive for Serde and catalog/book types, combined with wasm-bindgen's own generated declarations for the ABI; drop `--no-typescript`, and let Minijinja join both descriptions into the adapter's compile context rather than pretending either source covers the other.

The first derive must accept named structs, unit and named enum variants, explicit wire direction, `rename_all`, `tag`, `default` and `Option`, plus the production map-like newtype case at `league-core/src/proto.rs:303`: `C2S::Cmd(Cmd)` serializes as the intersection `{ t: "cmd" } & Cmd`, where the inner union contributes its `a` discriminator and fields, so the derive must merge the two object descriptors and reject a payload whose keys collide with the outer tag.

No `flatten`, `untagged`, `skip`, per-item `rename` or multi-field tuple variant occurs in the four current protocol files; the first derive should reject those shapes, custom serializers, unregistered generic instantiations and ambiguous integer lowering at compile time, which costs no current protocol coverage while keeping unsupported Serde semantics explicit.

For JSON, a top-level `Option<X>` field accepts either absence or `X | null` on deserialization, `serde(default)` also permits absence for other field types, and serialization requires the field as `X | null` unless an explicit non-Serde script lowering omits `None`, so the generator needs distinct input and output views.

The loader event descriptor owns the exceptional output omission rule for non-applicable fields, while one native lowering shared by tests and `wasm.rs` walks the pinned camel-case key table, converts byte counts to JavaScript numbers with `as_f64` and omits absent values; generated declarations must preserve that actual object shape.

### Shared shader machinery

The script generator should share `ember-shader`'s pinned Minijinja 2.24.0, embedded production-template registry, strict undefined behavior, Rust-owned metadata/context, deterministic render, stable source hash, no global mutable cache, exact rendered-byte tests, per-emission provenance and the production-template test pattern.

It should not share WGSL-specific type-layout metadata, binding records, emission syntax, naga parsing or shader validation; TypeScript validation belongs to `tsc`, and JSON schema validation belongs to a JSON-schema checker generated from the same boundary description.

### Execution point

|Place|What it can guarantee|Why it is or is not sufficient|
|-----|---------------------|------------------------------|
|Package `build.rs` into `OUT_DIR`|Cargo reruns on declared Rust/template inputs and the package cannot compile without generation|A build script runs before its own crate and cannot call that crate's derived registrations; another crate can own the types, but deploy still has to find the correct opaque `OUT_DIR` and can copy old `web/*.js` instead|
|Workspace `xtask`|One explicit command can build schemas, render all targets, run `tsc` and compare checked-in output|It is not compile time unless every CI, test and deploy caller invokes it; checked-in output admits stale commits between regeneration and checking|
|Workspace generator crate invoked by `deploy/deploy-pages.sh`|It compiles against current core crates, renders into a fresh release staging directory, consumes `.d.ts` from the just-built wasm and hands only successful `tsc` output to assembly|Recommended: the shipped JavaScript has no source path except the current Rust/template compilation, and a failed generator or compiler leaves no publishable tree|

“At compile time” should mean release-artifact compile time, not a source-tree rewrite: `deploy/deploy-pages.sh` builds wasm, runs wasm-bindgen with TypeScript enabled, runs the Rust generator into `target/web-generated/$SOURCE_SHA/ts`, runs pinned `tsc` into the sibling `js` directory, verifies its manifest and copies only that `js` directory into the Pages staging tree.

The generator should emit type-only discriminated unions for every `C2S`, `S2C` and League `Cmd`; directional input/output structs; loader `Phase`, `Status`, event and method-return shapes; wasm glue declaration refinements; `GameCatalog`, `GameRelease`, `HostBook`, `HostEntry` and mirror types; and JSON Schema for `games.json`, `server.json` and mirrors.

Dynamic per-game host keys should be template-literal key types derived from the `GameId` union, with bracket access required by TypeScript, while a generated runtime decoder narrows third-party JSON and WebSocket data from `unknown` before code uses it.

### Proof from Rust to rendered TypeScript

Each derive should emit a stable descriptor and variant count; the registration table names every top-level direction, and compilation fails if a nested named type lacks the descriptor trait.

A Rust test should construct one explicit wire sample per enum variant, assert the sample count equals the derived variant count, serialize each sample through `serde_json`, and compare tag name, tag key, present fields, omitted default fields and null options against the descriptor; a new variant therefore breaks the sample-count test before it can be absent from the proof.

A second Rust test renders every production template twice, requires byte equality and the stable hash, parses the generated declarations with the pinned TypeScript compiler in the server gate, and checks a compile-fail fixture in which an exhaustive consumer omits one generated variant.

Catalog tests should deserialize the real `web/games.json` through the Rust model, validate the generated JSON Schema against that file, and require the deploy manifest's live path and protocol fields to come from the same model; host-book tests should run the current hostile-input fixtures through the generated decoder before `hosts` logic.

## Pedantic TypeScript contract

The browser base configuration should be exact and checked in as follows; generated paths are illustrative build-staging paths, not tracked outputs.

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ES2022",
    "moduleResolution": "Bundler",
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "types": [],
    "strict": true,
    "noUncheckedIndexedAccess": true,
    "exactOptionalPropertyTypes": true,
    "noImplicitOverride": true,
    "noPropertyAccessFromIndexSignature": true,
    "noFallthroughCasesInSwitch": true,
    "useUnknownInCatchVariables": true,
    "verbatimModuleSyntax": true,
    "isolatedModules": true,
    "noImplicitReturns": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "allowUnreachableCode": false,
    "allowUnusedLabels": false,
    "forceConsistentCasingInFileNames": true,
    "skipLibCheck": false,
    "noEmitOnError": true,
    "sourceMap": false,
    "declaration": false,
    "removeComments": true,
    "newLine": "lf",
    "rootDir": "target/web-generated/ts",
    "outDir": "target/web-generated/js"
  },
  "include": ["target/web-generated/ts/**/*.ts"]
}
```

Worker projects extend that file but replace `lib` with `ES2022` and `WebWorker`; browser and worker libraries must not be combined because their ambient globals conflict.

```json
{
  "extends": "./tsconfig.web.json",
  "compilerOptions": {
    "lib": ["ES2022", "WebWorker"],
    "rootDir": "target/web-generated/worker-ts",
    "outDir": "target/web-generated/worker-js"
  },
  "include": ["target/web-generated/worker-ts/**/*.ts"]
}
```

Node ESM sources use `.mts` and emit `.mjs`; CommonJS sources use `.cts` and emit `.cjs`; their project replaces `lib` with `ES2022`, selects `module` and `moduleResolution` `Node16`, and loads a Node 24 declaration package pinned in the lockfile.

```json
{
  "extends": "./tsconfig.web.json",
  "compilerOptions": {
    "module": "Node16",
    "moduleResolution": "Node16",
    "lib": ["ES2022"],
    "types": ["node"],
    "rootDir": "target/web-generated/node-ts",
    "outDir": "target/web-generated/node-js"
  },
  "include": ["target/web-generated/node-ts/**/*.mts", "target/web-generated/node-ts/**/*.cts"]
}
```

With `verbatimModuleSyntax`, generated `.mts` files use ESM imports and exports while generated `.cts` files use TypeScript's `import name = require("module")` and `export = name` forms rather than ESM syntax that cannot be emitted as CommonJS.

Every relative import specifier names the emitted `.js`, `.mjs` or `.cjs` path, `verbatimModuleSyntax` makes type-only imports explicit, and the tracked HTML shells contain external module tags rather than inline bodies.

`strict` does not forbid an explicit `any`, and `moduleResolution: "Bundler"` accepts relative specifiers that a native browser cannot resolve, so a source-AST gate using the pinned TypeScript compiler must reject `AnyKeyword`, `@ts-ignore`, unchecked double assertions, extensionless or TypeScript-suffixed relative imports, and declarations imported from generated wasm glue that expose `any`; JSON parsing, `postMessage`, errors and wasm `JsValue` enter as `unknown` and pass a generated decoder or local type guard.

DOM lookup uses checked helpers that accept an element constructor and return its instance only after `instanceof`; nullable selectors stay nullable until tested, event targets are narrowed before property access, dataset and index-signature fields use bracket notation, and vendor-only APIs live in narrow declared interfaces rather than casts to `any`.

### Emitted JavaScript and size

`tsc` should perform syntax-preserving type erasure at ES2022 with no bundle, polyfill, down-level helper, source map or runtime TypeScript enum; protocol declarations are union types, not `enum` values, and their emitted stub is excluded from the deploy copy graph.

The sokol spike rendered 1,239 bytes and 27 lines of type-only Fire protocol TypeScript into a 63-byte, 2-line module stub; that stub carries no runtime value and should not ship.

A pedantic conversion of the real 24-line, 848-byte Julibrot worker emitted 34 lines and 1,299 bytes, a 451-byte or 53.2% increase, because honest narrowing added a runtime guard; this disproves a blanket promise that typing never changes bytes even though TypeScript annotations themselves erase.

Each migration lane must therefore record raw and gzip bytes for every emitted module and the wasm before/after; pure type generation has a zero-shipped-byte budget, adapters may not introduce a helper or bundler runtime, and logic moved into Rust must stay within a separately approved wasm and total-page budget rather than being hidden as TypeScript overhead.

No current `include_bytes!` site includes web scripts, so `tsc` output does not directly enlarge today's wasm assets; moving page logic into wasm can enlarge the bundle, and existing embedded-asset ceilings remain unchanged unless a measured lane explicitly re-baselines them.

## Toolchain and pipeline

Phase 0 added the tracked package and lock files, three pedantic TypeScript projects, Node 24 declarations, wasm-bindgen declarations, the `typed web` CI job and the generated-boundary release gate. Phase 1 adds the three emitted Node suites, the rendered-source AST gate and the Pages rebuild of the emitted host tool under Node 24.20.0.

The implementation pin is Node 24.20.0 and TypeScript 5.9.3, the versions measured by the spike: the lockfile is authoritative, `npm ci` is the only install path, and the host SDK TypeScript extension, sokol/osprey build pods and CI must print and compare their active versions with the lock before compiling.

The spike used Node v24.20.0 and TypeScript 5.9.3; `npx -y typescript@5.9.3 tsc` could not select a binary under npm 11.19.0, while `npx -y --package typescript@5.9.3 tsc` worked, so release automation should use the lockfile's `npx --no-install tsc` rather than a network install.

Phase 0 removed `--no-typescript` from the executable and header recipes, made the wasm-bindgen shim emit the expected declarations and taught assembly to stage them as compiler inputs.

The `typed web` CI job sets up exact Node 24.20.0, runs `npm ci`, the generator check, every TypeScript project, the three suites converted so far and the generated-tree manifest. Later phases add their remaining suites to the same emitted runner as they convert them.

`deploy/tests/test-typescript.sh` pins the configs, rejects forbidden handwritten outputs and AST escape hatches, checks generated hashes and compile-fail fixtures and runs each emitted suite by explicit path. `test-pages.sh` proves the assembler copies only the two named emitted browser modules. `deploy/tests/run.sh` lists the TypeScript suite; callers select required execution with `EMBER_TYPED_WEB_REQUIRED=1`.

On sokol and osprey, the same release command runs Rust/wasm builds and `tsc`; the workstation remains an editor and transport endpoint, and CI becomes the independent Node owner rather than trusting a developer's extension.

The template wrapper earns its place over tracked `.ts` plus the same AST gate by giving every shipped script one render path and provenance manifest, prepending generated boundary imports outside the template-controlled body so they cannot be omitted, registering every production template in the same embedded registry and stable-hash tests as `ember-shader`, and leaving no bare `.ts` for a stray compiler invocation to accept outside the gate. The generator emits an unshipped rendered-to-template line map beside each staged input; `test-typescript.sh` and `deploy-pages.sh` pipe compiler output through its error printer so diagnostics report the canonical template path, line and column while retaining the rendered coordinate.

For a live page at `web/games/ID/vN/`, Stage 1 tracks its handwritten `.ts.j2` behavior template, a tracked HTML shell with only external module tags, styles and media; generated declarations are mandatory template inputs, `.ts` and `.js` appear only under `target/web-generated/$SOURCE_SHA/games/ID/vN/`, the assembler copies that generated `.js`, and a gate rejects a tracked editable `.js` or bare `.ts` beside the page.

Stage 1 tracks no bare `.ts`, `.mts`, `.cts` or emitted JavaScript: handwritten behavior lives in `.ts.j2`, `.mts.j2` or `.cts.j2` legacy templates, generated declarations and rendered compiler inputs live under `target`, and immutable frozen JavaScript remains under the governing historical-artifact exception.

The 4,202 lines in the 20 current Node suites cannot be generated from the product definitions they test without destroying their independence, so all remain handwritten tests under the legacy-template rule: 17 suites under `tools/**` plus the `web/hosts`, `web/loader` and `deploy/check-hosts` suites become `.mts.j2` or `.cts.j2`, import generated types wherever they touch a boundary, render to the Node TypeScript staging tree, compile under Node16 rules and run against emitted modules.

The existing `/target` ignore already covers the recommended generation directory and `/web/pkg/` covers generated wasm bindings, so no broad new ignore is needed; if any generator writes under `web/`, add one exact generated-directory rule and make deploy refuse output whose source manifest does not name `HEAD`.

## Frozen versions

`docs/branching.md` preserves the `gh-pages` publication record, `docs/versioning.md` treats historical version slots as stable release identities, and `deploy/deploy-pages.sh` says older version directories stay untouched in the seed; byte identity, not semantic equivalence, is the relevant record.

|Policy|Result|Publication consequence|
|------|------|-----------------------|
|Convert frozen source in place|The repository can report fewer JavaScript sources, but generated formatting and guards change bytes|The tracked reconstruction no longer matches the published page even when behavior is intended to match; copying it later overwrites the record|
|Leave frozen bytes and convert only editable/live versions|Historical JavaScript remains as an explicit artifact exception|Existing publication bytes and hashes remain valid; the literal claim “no JavaScript anywhere” must be narrowed to authoritative editable source|
|Regenerate every frozen page from templates plus frozen protocol definitions|Old pages gain the new pipeline only if every historical Rust/wasm ABI, template and toolchain is recoverable|Even a correct regeneration is a new byte sequence and therefore a republication, not preservation; missing v1–v6 Arena source makes complete reconstruction impossible from develop alone|

The second policy is already required: `docs/branching.md` freezes the `gh-pages` publication record and the release ledger identifies publications by those bytes, so the assembler must leave every frozen directory in the seed and record its hashes; the Arena v0 rewrite is a deploy bug, frozen pages must never enter `tsc`, and a security repair must create a new release slot rather than silently rewrite an old one.

This recommendation intentionally does not satisfy the unqualified phrase “every script in the repo”; satisfying that phrase requires abandoning byte identity or disguising archived JavaScript with another extension, and the latter would improve neither typing nor provenance.

## Ordered staged phases

Lines below are measured current script or protocol lines; Stage 1 is a sequence of conversion lanes, and Stage 2 admits one surface at a time only after a measured case for moving its behavior.

### Stage 1: generated contracts and legacy TypeScript templates

|Phase|Bite|Size|What it proves|Required verification|
|----:|----|----|--------------|---------------------|
|0|Generator foundation complete (`0fc21b6a`, `a9464c1b`, `264273a3`, `240554ab`): registration derive, descriptors, declaration templates, catalog/book Rust models, wasm-bindgen declarations and configs, plus the wasm-bindgen flag, shim, copy and documentation path|4 protocol inputs/3,252 lines plus loader Rust 3/1,028 at `d19f70c9`; at most 1,300 new lines|One compiler-owned schema renders protocol, ABI and JSON artifacts without checked-in output, including League's intersected `t` and `a` discriminators|Rust unit/oracle tests, deterministic render hashes, real catalog schema validation, `.d.ts` Pages fixture, clean and deliberate-fail `tsc` fixtures|
|1|Complete (`7d5b4692`, `45e0a3ff`, `1e289ef6`, `9ab9cb91`): hosts, loader, emitted host gate and all three independent suites are checked legacy TypeScript templates|6 files/3,721 lines at `1b620127`; templates 4,511 lines, +790 total (+422 production, +368 tests)|Rust loader events and catalog/book data reach browser and Node through mandatory generated types; hostile inputs narrow from `unknown`; the assembler ships only two named emitted modules|Pedantic `tsc`, three rendered suites, source-AST fixtures, `test-pages`, module graph, raw/gzip JavaScript and byte-identical loader wasm|
|2|Fire current page and protocol|2 files/503 script lines; Fire protocol input 521 lines|The smallest live game's templates compile against generated `C2S`, `S2C` and wasm declarations even though socket ownership stays in wasm|Fire core/client tests, generated union oracle, browser race/garage smoke, Pages fixture, size comparison|
|3|Kings current page|1 HTML block/517 script lines; Kings protocol input 883 lines|The inline body becomes an external `.ts.j2` module whose direct socket messages, DOM and wasm glue use generated unions|Kings protocol/server tests, rendered Node/browser page smoke, no-inline check, Pages fixture, size comparison|
|4|League current page|1 file/1,155 lines; League protocol input 491 lines|`C2S`, `S2C` and tag-`a` `Cmd` action exhaustiveness reaches the largest live external UI through mandatory imports|League core/client tests, UI smoke, command compile-fail fixture, Pages fixture, size comparison|
|5|Arena current controls and inline page|1 external/466 plus 381 inline lines; Arena protocol input 1,357 lines|The largest protocol and dynamic settings DOM compile without unchecked indexed access while the body remains a template|Arena protocol/client tests, rendered settings tests, browser controls/network smoke, no-inline check, Pages fixture, size comparison|
|6|End Game current modules and tests|6 files/800 plus 4 tool tests/511 lines|Audio, media, DOM and wasm glue become checked templates, the assertions stay independent templates, and the generated module graph catches the four files today's deploy omits|All 4 rendered Node suites, client tests, browser boot, import closure, Pages fixture, size comparison|
|7|what-is-this inline application|1 HTML block/2,262 script lines|The largest inline surface becomes an external checked template without losing progressive failure handling|Rendered diagnostic tests, browser stages, optional-upload doubles, no-inline check, size comparison|
|8|Julibrot current graph|3 files/1,181 plus drive 109 inline lines|Main-thread, worker and wasm message shapes compile under separate DOM and WebWorker projects|Julibrot suites, worker ABI tests, drive browser proof, no-inline check, size comparison|
|9|Root hub, two game landings and developer lab inline pages|Hub 527 inline, landings 2/328, developer labs 5/1,248 inline: 8 surfaces/2,103 script lines|DOM-only templates, catalog narrowing and separate scoped publishers work without inventing protocol types|Hub and landing browser smokes, lab oracles, catalog schema check, no-inline check, scoped publish fixtures|
|10|League tools, including their Node tests|19 files/3,808 lines|A mixed browser, network, CommonJS and publication family converts intact to `.ts.j2`, `.mts.j2` or `.cts.j2` and shares mandatory generated boundary types|Rendered Node suites and isolated dry-runs, Node16 `.cts` to `.cjs` and `.mts` to `.mjs` checks, no tracked emitted files|
|11|Trailer and v22–v25 tools|11 files/2,274 lines|Browser capture and mixed public/network automation retain behavior while using the Node/browser project split and generated types|Rendered capture dry-runs and network fixtures, emitted-extension and size checks|
|12|v28–v29 tools|11 files/2,499 lines|Historical automation consumes generated catalog and host-book models without a handwritten boundary copy|Rendered Node suites, browser/network smokes and publisher preservation fixtures|
|13|v30 tools|10 files/2,170 lines|A self-contained historical tool family converts without retiring entry points or generating its own assertions|Rendered Node suites and smokes, publisher preservation, no emitted tracked files|
|14|v31 tools and asset-side voice data|13 tool files/2,577 lines plus 1 asset file/33 lines|Current Arena automation and generated static data leave no editable naked JavaScript while preserving every tool|Rendered Node suites and smokes, asset generation equality, publisher preservation|
|15|Node test closure|20 suites/4,202 lines already counted in phases 1, 6 and 10–14|Every independent assertion suite is a checked legacy template and CI runs the complete emitted test set; this audit adds no second conversion count|Central Node runner, source-AST gate, generated-type import audit, intentional product defect proving a test still fails|
|16|Frozen enforcement|39 external files/7,561 plus 27 HTML pages/7,798 inline lines, with Arena v0 counted in both policy sets|The existing publication policy is mechanically enforced without pretending archived bytes are editable source|Seed-directory hashes, Arena v0 preservation fixture and frozen allowlist exactness|

All 68 `tools/**` files and 13,839 lines are converted, none are retired: the 4 End Game tool tests/511 lines are in phase 6 and the remaining 64 files/13,328 lines are in phases 10–14, where one mechanical conversion rule preserves their executable verification and publication record more cheaply than 64 separate retirement reviews; the 17 tool test suites plus the three non-tool Node suites make the independently checked 20-suite total.

Phase 1 emitted sizes are `hosts.js` 24,069 to 15,494 raw bytes and 8,781 to 4,235 gzip bytes, `loader.js` 21,259 to 18,145 raw and 7,228 to 4,788 gzip, and `check-hosts.mjs` 18,048 to 11,778 raw and 6,714 to 3,583 gzip. The production budget count is 785 of 1,500 non-blank lines: 380 is the converted-module delta after excluding doc comments, and 405 is other production code; tests, fixtures, goldens, lockfiles and documentation are excluded. No phase 1 deliverable is deferred; later Stage 1 rows remain the standing backlog, and the release workflow deliberately does not duplicate the Pages-owned live host gate.

Each Stage 1 phase ends with `git diff --check`, LF/UTF-8 and forbidden-token scans, the TypeScript source-AST gate, relevant `deploy/tests` suites and current-tree file/line/byte counts; “no naked JS” becomes true only when every editable shipped script is compiler output, while “no naked TS” always carries the declared legacy-template exception.

### Stage 2: selective Rust ownership

Stage 2 is not a second repository-wide rewrite: each candidate below is one separately measured lane, admitted first when it speaks a protocol or owns state transitions, validation or duplicated state, and admitted only when the resulting wasm/page size, startup cost, browser API surface and maintenance result beat the Stage 1 template.

|Candidate surface|Stage 1 input|Why and when behavior should move|
|-----------------|------------:|---------------------------------|
|Hosts plus loader|6 JavaScript files/3,721 lines at `1b620127`, now 6 templates/4,423 lines|First candidate because it owns loader phases, event lowering, catalog/book validation and browser/Node state transitions; retain only generated adapters once the loader ABI is stable|
|Fire current|2 files/503 lines|Move only state duplicated outside the socket-owning wasm; keep garage and presentation DOM in templates if the wasm boundary already owns the protocol|
|Kings current|1 inline block/517 lines|High priority because the page speaks the socket protocol directly and owns message-driven state transitions|
|League current|1 file/1,155 lines|High priority because the page joins `C2S`, `S2C`, nested `Cmd` actions and UI state|
|Arena current|847 external-plus-inline lines|High priority where settings and network state cross the protocol; leave direct control and presentation DOM in templates until a measured defect justifies moving it|
|End Game current|6 files/800 lines|Move durable game, audio and transition state when Rust tests or reuse pay for it; leave DOM-only rendering last|
|what-is-this current|1 inline block/2,262 lines|Move progressive diagnostic and report state if it removes duplicated recovery logic; keep incidental diagnostic DOM in its template|
|Julibrot current|3 external files plus drive inline/1,290 lines|Move worker messages and computation state when a shared Rust model replaces runtime guards; presentation remains a template|
|Hub, landings and labs|8 surfaces/2,103 lines|These are predominantly DOM-only and therefore last or never unless catalog logic, reuse or a recurring correctness defect supplies a measured case|
|All tools|68 files/13,839 lines|Remain checked legacy templates indefinitely by default; extract only reusable protocol or state logic whose duplication or defect rate justifies a Rust owner|

## Sokol spike

The uncommitted spike temporarily attached a registration derive to the actual Fire `C2S` and `S2C` items, recorded input versus output direction and `serde(default)`, rendered both enums through Minijinja 2.24.0, and compiled one exhaustive consumer under the exact pedantic flags above with Node v24.20.0 and TypeScript 5.9.3.

The clean warm generator run took 4.1 s; its initial cold dependency/build run took 8.6 s; the successful TypeScript compile took 1.2 s.

The baseline output contained 20 discriminated variants in 27 lines and 1,239 bytes; because it contained only types, `tsc` emitted a 2-line, 63-byte module stub that should be omitted from the shipped graph.

Adding `S2C::Maintenance { reason: String }` in Rust, without editing the consumer, regenerated 21 variants in 28 lines and 1,280 bytes in 0.9 s; `tsc` then failed in 1.2 s with `TS2345` because `{ t: "maintenance"; reason: string }` reached the consumer's `never` assertion.

The first pedantic worker conversion also demonstrated the value and cost of the flags: `noPropertyAccessFromIndexSignature` rejected two dotted accesses on a `Record<string, unknown>` in 2.5 s, the corrected build passed, and the emitted worker grew from 848 to 1,299 bytes because it performs runtime narrowing.

The command `npx -y typescript@5.9.3 tsc` failed in 0.9 s before compilation because npm could not determine which executable to run; `npx -y --package typescript@5.9.3 tsc` succeeded, and no unpinned compiler was used.

All server-command wall times used by the spike sum to 20.1 s, excluding SSH and file-transfer overhead, comfortably below the 90-minute cap; the temporary tracked edits were restored and the spike remains only in ignored `target/ts-study-spike` on both machines.

## Risks

- A derive that silently accepts a Serde attribute it does not model can be more dangerous than handwritten types, so unsupported forms must be compile errors and serialized samples must test the mapping.
- TypeScript structural typing does not validate hostile JSON at runtime, so generated decoders are part of the boundary rather than an optional validation layer.
- Moving thousands of DOM and audio lines into wasm can increase wasm size, startup time and browser API surface even while emitted JavaScript shrinks.
- A deploy path that can still copy source-tree JavaScript can bypass the generator, so the assembly manifest and fresh staging directory are security boundaries.
- Frozen bytes and a repository-wide no-JavaScript slogan are incompatible; an unspoken exception would make the final gate dishonest.
- The current End Game copy omission and Arena v0 rewrite can invalidate migration measurements unless phase 1 makes the module graph and frozen manifest executable checks.
- Browser and worker ambient libraries collide, while Node ESM and CommonJS choose output extensions from source suffixes; one catch-all TypeScript project will give misleading success.
- Tool automation can carry credentials or publish externally, so migration verification must use existing dry-run and fixture modes and must not turn a typecheck into a publication.

## Decisions

The staged path holds on the merits: mandatory Rust-generated declarations give Stage 1 its compile-time boundary guarantee without waiting for a behavior rewrite, while Stage 2 moves protocol and state-transition ownership into Rust only where correctness, reuse and measured delivery costs justify it; `.ts.j2`, `.mts.j2` and `.cts.j2` are the explicit indefinite legacy-template exception to “no naked TS.”

Every `tools/**` script migrates without retirement because a version-pinned smoke or publisher can be the only executable record of how its release was verified or published, so deleting it would delete evidence rather than merely remove old code; converting the 64 files assigned to phases 10–14 under one mechanical rule is cheaper than adjudicating 64 retirement cases, while any tool can still be retired later as a separate change with its own evidence and reason, and all 19 Node suites remain independent handwritten template tests rather than generated echoes of the product they test.

## Measurement commands

All commands ran from the worktree except the commands explicitly wrapped by the sanctioned sokol worker invocation.

```text
git ls-tree -r --name-only develop | grep -E '\.(js|mjs|cjs)$' | grep -v /pkg/ | wc -l
git ls-tree -r --name-only develop | grep -E '\.(js|mjs|cjs)$' | grep -v /pkg/ | xargs wc -l | tail -1
git ls-tree -r --name-only develop | rg '\.(js|mjs|cjs)$' | rg -v '/pkg/' | while read -r f; do wc -l "$f"; done
git grep -l '<script' develop -- '*.html' | sed 's/^develop://' | while read -r f; do awk 'inline block and line counter' "$f"; done
rg -n -B4 -A2 '"live": true' web/games.json
sed -n '1,620p' deploy/deploy-pages.sh
rg -n '^pub enum (C2S|S2C|Cmd)|#\[serde\(tag' crates/{arena,fire,kings,league}-core/src/proto.rs
wc -l crates/{arena,fire,kings,league}-core/src/proto.rs
git fetch --no-tags sokol lane/loader:refs/remotes/sokol/lane/loader
git rev-parse refs/remotes/sokol/lane/loader
git show sokol/lane/loader:crates/ember-loader/src/{phase,event,wasm}.rs
for path in crates/ember-loader/src/phase.rs crates/ember-loader/src/event.rs crates/ember-loader/src/wasm.rs; do git show "sokol/lane/loader:$path"; done | wc -l
git show sokol/lane/loader:web/loader.js | wc -lc
wc -l web/hosts.js web/loader.js deploy/check-hosts.mjs web/hosts.test.mjs web/loader.test.mjs deploy/check-hosts.test.mjs
rg -n '#\[serde\([^]]*(flatten|untagged|skip|rename\s*=)|^[[:space:]]+[A-Z][A-Za-z0-9_]*\([^)]*\),' crates/{arena,fire,kings,league}-core/src/proto.rs
nl -ba deploy/deploy-pages.sh | sed -n '245,380p'
nl -ba web/games/end-game/v12/main.js | sed -n '1,12p'
wc -l web/games/end-game/v12/{dialogue,castle-audio,castle-ui,voice-lines}.js
find web/games/end-game/v12 -maxdepth 1 -type f -printf '%f\n' | sort
rg -n -- '--no-typescript' CONTRIBUTING.md docs/hosts.md docs/julibrot/refactor-survey.md deploy/deploy-pages.sh
nl -ba deploy/tests/shims/wasm-bindgen | sed -n '1,80p'
npx -y --package typescript@5.9.3 tsc -p target/ts-study-spike/tsconfig.json
wc -lc target/ts-study-spike/out/protocol.ts target/ts-study-spike/js/out/protocol.js web/labs/julibrot/worker.js target/ts-study-spike/converted-worker.ts target/ts-study-spike/js/converted-worker.js
```

Upstream constraints checked for the feed comparison are the [`serde_reflection` feature and limitation list](https://docs.rs/serde-reflection/0.6.0/serde_reflection/#features-and-limitations) and the [wasm-bindgen CLI TypeScript flags](https://rustwasm.github.io/docs/wasm-bindgen/reference/cli.html).
