# r/webassembly

Title:

```
An online FPS in a wasm bundle: deterministic 60 Hz lockstep, WebGL2-only, no runtime fetch — here is the architecture
```

Body:

```
I built a game engine in Rust that targets wasm for the web (the same crates run natively), and the current build is an 8-player online FPS that runs entirely in a tab. The web story is what I want to talk about, because every decision was forced by the browser.

- One bundle per game, no runtime fetch. Assets are baked in at compile time (include_bytes!), so the wasm file plus the page is the whole product. It deploys to a static host — GitHub Pages in this case — with zero backend on the client side. The FPS bundle is about 39 MB, which I am not proud of; the size levers are listed in the backlog.
- The floor is WebGL2 + EXT_color_buffer_float, and that is the whole floor. No WebGPU, no cross-origin isolation (concurrency is Web Workers with message passing, not shared memory), no float-blend extension. The reason for that exact floor — including which real devices it excludes (almost none, because the devices missing the extension are overwhelmingly the devices missing WebGL2) — is written down in docs/minimum-requirements.md. A device below the floor gets a typed refusal naming the missing capability; it never gets a blank canvas.
- The multiplayer is the strange part. There is no central game server: any machine runs a host behind a Cloudflare tunnel, and the page carries an address book (a JSON file on the static host) that lists every host with its build, its commit, and the protocol version each of its servers speaks. The page asks every host in one round trip and picks the newest build that speaks its protocol, the emptiest among those, the nearest among those. The full model, including how a frozen old version stays playable on an old host, is docs/hosts.md.
- The sim is a deterministic 60 Hz lockstep — fixed update order, two seeded world-generation LCGs and nothing else random — so replays, save states and rollback are available for free, and the client predicts its own movement against the same sim the server runs.
- A side project that came out of the floor: "what is this?", a playful browser and hardware diagnostic that runs wasm CPU and timing kernels and can optionally upload a JSON report. It doubles as the support tool — the bug template asks for its output.

Repo: https://github.com/EndersGamesDev/EmberEngine
Play: https://endersgamesdev.github.io/EmberEngine/

What I want to know from people who have done wasm at this scale: is the no-runtime-fetch tradeoff defensible at 39 MB, and is the address-book host discovery a pattern you have seen work, or a clever way to debug your own fleet?
```

Notes: post the day after HN so the two threads do not cannibalise each other.
