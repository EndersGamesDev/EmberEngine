# Fire Racer V2 — GT Circuit

First draft: grounded sports-car handling and readable circuit racing, with a single tactical item slot. The existing Ember engine remains the runtime. Work is isolated in `codex/fire-racer-v2`; Arena, Kings and their running services are outside this change.

## Power-up system

Drive through a rotating mystery box to fill an empty slot. Use E to activate the held item; Shift remains the driver's boost button. Each box respawns after a short cooldown. A deterministic draw based on race tick, box and driver gives trailing positions more opportunities to recover without changing their base engine power. Pickups never replace a held item.

| Item | Effect | Counterplay |
| --- | --- | --- |
| Nitro | A sustained acceleration and speed burst | Brake before the next corner; no steering automation |
| Shield | Temporary protection from one incoming attack | Bait it before spending an attack |
| Pulse | A visible guided projectile targeting the next car ahead | Shield absorbs the hit; recovery protection prevents chains |
| Oil | A temporary slick dropped behind the car | Avoid the patch or use grip boost |
| Grip | Temporary extra road holding and protection from oil | Useful through corners; no straight-line speed advantage |

Drifting builds a short charge only while moving and actually slipping. A controlled exit awards a mini boost; parked handbrake input cannot farm it. Car contacts exchange impulses and separate bodies. Attacks slow briefly rather than disabling steering or removing players from the race. Race completion and recovery must remain possible for every vehicle and AI driver.

## Vehicle and race experience

Three unbranded vehicles: balanced GT, agile lightweight, and powerful muscle car. Each needs a distinct silhouette and actual handling differences. Keyboard steering must build and return smoothly, remain stable at speed, and reverse correctly. Use the existing castle circuit with asphalt, racing markings, readable braking areas, warm lighting, lower chase camera, tyre effects and clear item visuals. The renderer has no PBR material support, so this release targets a substantial grounded visual improvement without claiming photorealism.

Loop: choose a car → grid/countdown → three laps against seven competitors → results with race time and placement → race again or return to garage. HUD: speed, gear, lap, place, held item, boost/drift feedback and race time. Practice works without a server. Online uses the same authoritative simulation, version-gated as Fire protocol 2.

## Verification and release

Check car assets before replacing or adapting them. Verify handling, collision separation, item pickup/consumption/counterplay, deterministic races and AI completion in Rust; verify client/server sync, the WebAssembly build and browser presentation. Publish only Fire's page, bundle and catalog entry, preserve the rest of the Pages branch, then update only Fire's server and address-book keys. Record measured checks and remaining limitations here before release.

## Implemented release

The garage selects GT-01, Apex R or V8-R with different acceleration, braking, steering, grip, top speed and mass. The shared 60 Hz simulation provides capsule contacts, drift exit boosts, twelve deterministic respawning boxes, all five items and recovery that preserves checkpoints. Fire protocol 2 carries vehicle choice, exact effect timers, finish times, pickups, projectiles and slicks. Online results remain available until drivers leave; local rematches reuse the existing renderer.

Asset audit: the previous 107,348-byte car GLB contained one 6,000-triangle POSITION-only primitive with no separate wheels, materials or UVs. Architecture GLBs remain active. Three proportioned curved body meshes, separate wheel/glass/light parts and generated paint/reflection textures now provide distinct vehicles. Track presentation adds asphalt, runoff, kerbs, braking boards, covered stands, trees, sky and a camera that follows the vehicle's translation without falling behind at speed. Web presentation includes the garage, item guide, HUD, results, synthesised engine/tyre/item audio and keyboard/gamepad controls.

Initial verification: 129 Rust tests passed (106.8 seconds including the first native client dependency build), covering full three-lap AI races, determinism, item counterplay, collisions, input edges, client prediction and real WebSocket integration. The first complete WebAssembly build took 28.6 seconds. A subsequent rendering pass adds a camera centering regression. Isolated headless Edge practice checks cover desktop/mobile layout, car choice, acceleration, boost charge consumption, drift/recovery input and renderer reuse on restart; first run passed in 8.9 seconds. A real WebSocket server plus WebAssembly client test verified vehicle choice, movement, boost and preserved canvas focus after the sound toggle in 6.7 seconds. Process ownership/deployment boundary tests passed in 0.11 seconds. Screenshots and machine-readable results are generated under `target/fire-v2-qa/`.

Release is isolated: `deploy/publish-fire-pages.py --push` copies only `games/fire/v2/` and replaces only the Fire object in the current remote catalog. UTF-8 reads preserve every other game's Unicode. `deploy/deploy-fire-local.ps1` checks exact executable, process creation time and port ownership; verifies a candidate's protocol and commit before replacing anything; and changes only this host's Fire address-book fields after public protocol checks. The old `sokol` alias is absent on this workstation, so V2 is hosted on its free port 7781 while the existing remote V1 and local Arena service remain untouched.

Limits and follow-ups: the driving remains a simcade model with approximate tyre grip and contact, and the Ember renderer supports a single albedo texture rather than PBR or dynamic reflections. Graphics remain stylised. Recorded engine sounds, richer vehicle assets and suspension/tyre simulation are future work. Browser audio is synthesised and was checked functionally, not by a listening panel. The workstation host is available while this machine remains awake; migrate the Fire-only service to a configured always-on host for continuous uptime. Mobile layout is verified, but driving currently requires keyboard or gamepad.

Final gate: all 130 Rust tests passed; `cargo clippy -p fire-core -p fire -p fire-server --all-targets -- -D warnings` passed; the release WebAssembly build passed. The combined final gate took 30.9 seconds. Final desktop/mobile practice browser smoke passed in 8.9 seconds and real-server online browser smoke passed in 6.7 seconds with no browser exceptions. Manual screenshot inspection confirmed the camera, smoother body finishes, sky, trees and grandstands. A prior scene test incorrectly assumed opponents could not bump a parked car; its stationary assertion was removed while retaining finite scene, AI movement and untouched driver-resource checks. Repository-wide tests and unrelated games were deliberately not run or changed.
