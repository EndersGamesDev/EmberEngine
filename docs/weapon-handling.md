# Weapon handling (Arena v24)

Every weapon has its own optical magnification, sight-raising time and model-space sight reference. The engine still renders one camera and one depth buffer; the game positions the gun, posed gloves and sleeves together. Aiming is no longer just a field-of-view effect: the server progresses a timed ADS fraction and uses it to narrow each shot's cone. The shared definitions live in `arena-core::shooter::{weapon_stats, weapon_handling, advance_ads, weapon_spread}`; client presentation lives in `arena::ads` and `arena::feel`.

## Rules and tuning

The starter sidearm remains weapon 1, with an infinite reserve, but now has six rounds, a 0.32 s shot interval and a 30 m range instead of eight rounds, 0.18 s and 60 m. It retains one body-hit damage and the existing headshot rules. Pickups improve firepower without making an empty player helpless; the existing M4 mesh fallback and loot pool are unchanged.

| Weapon | Optical zoom | Full raise | Full lower |
|---|---:|---:|---:|
| Sidearm | 1.15× | 140 ms | 100 ms |
| Vityaz SMG | 1.3× | 180 ms | 120 ms |
| AK-47 | 1.5× | 240 ms | 140 ms |
| M4 | 1.6× | 220 ms | 130 ms |
| Revolver | 1.4× | 190 ms | 120 ms |
| Sniper | 6× | 450 ms | 200 ms |
| RPG-7 | 2× | 300 ms | 180 ms |

Magnification uses the tangent of half the camera field of view, not a ratio of FOV angles. The sniper's former 3.5° field was excessive for these small arenas; its new 6× optic is deliberate. Other weapons raise their actual sights, with distinct intermediate pitch/roll and an exact settled pose. Reloading, changing weapons, death and a raised shield interrupt aiming. RMB and gamepad LT share the same intent path; scripted clients remain hands-off.

All guns have a nonzero hip-fire cone, a smaller settled ADS cone, and a further reduction when grounded and crouched. Actual movement—not simply holding a direction against a wall—widens the cone. Airborne shooting has a weapon-specific multiplier and minimum cone, so an aimed sniper cannot jump with perfect accuracy; crouch does not confer a grounded accuracy bonus in the air. Sustained shots add bounded recoil spread, which recovers with time rather than staying at the magazine's worst spread until reload. Projectile speed, gravity, hit damage, headshot zones and server-only bullet stepping otherwise retain their existing rules. Spread sampling uses the existing seed/tick/player hash, not a new RNG stream.

`PState.ads_fraction`, `PState.spread` and `PState.recoil_bloom` expose authoritative handling to the client. They are not client-supplied accuracy claims. The reticle combines the last confirmed recoil bloom with the locally predicted stance using the shared cone calculation, and preserves the latest same-weapon authoritative cone as a lower bound: movement can widen it immediately, while a precision gain waits for confirmation. It does not infer recoil from interpolated body motion. It is an aiming guide, not a promise that a projectile hits its centre: recoil, travel time, gravity and the target's movement still apply. A brief shield/reload/melee interruption resets the local ADS fraction as well as the server's, so a quick tap cannot resume a mostly raised scope while authority starts from zero.

## Ground movement recovered from the other task

The owner's completed slowdown was uncommitted in the shared `lane/arena-v18` checkout and had never reached GitHub or the server. Commit `6c85c59` ports its seven-file patch into an isolated branch without modifying the original checkout. Ground walking is 2 m/s, sprinting 3.2 m/s and crouching 1.1 m/s. The legacy horizontal air speeds and vertical jump arc remain, preserving the authored jump routes. There is no additional ADS movement slowdown in this release. Server simulation, initial client prediction and reconciliation replay all call the shared ground/air speed rule.

## Protocol and publication

Arena protocol 19 includes the recovered protocol-18 movement change and the timed handling rules. Exact join gating is intentional: a cached protocol-17 client predicts the wrong speed and does not display these accuracy states. Deploy the matching server before the v24 client; older archived Arena pages remain frozen and cannot join a protocol-19 match. The launcher remains able to list hosts without joining a game.

At the start of this task, source main `9953e6a` already contained the Julibrot and diagnostic integrations, but the public Pages build was still `d045403`. There was no missing game merge: the updated nine-stage “what is this?” test and Julibrot saved-view/retained-frame work needed publication. Rebuild all four game bundles plus the Julibrot bundle from the integrated source; do not pair the new lab loader with older WASM. The standalone Next.js `verschaetz-dich` branch remains unmerged under the repository's one-engine rule.

## Verification

Run native tests for Arena, arena-core, arena-server and ember-engine, followed by all-target Clippy and release WASM builds at Idle priority. Accuracy tests must exercise actual launched rounds and handling over ticks, not just compare table constants. Old collision-only tests isolate spread when testing a geometric boundary; new handling tests cover the shipped nonzero cones. Movement tests preserve walking speed, both-map collision/reconciliation and jump reach.

The test-only `step_geometry` uses the production step order and real speed, gravity, range and hit rules with a zero-dispersion launch callback. It is limited to exact pitch/trajectory, reflection/arc, map centre-ray visibility, cover/pierce ordering, splash and event-contact fixtures. The shipped gameplay damage/headshot, two-simulation replay and new weapon-handling cloud tests still use real spread. Centre-ray spawn occlusion is not a guarantee that no scattered edge shot can pass around cover. The protocol-19 fingerprint includes ADS fraction, recoil bloom and effective cone; it was deliberately regenerated after the movement and weapon changes, not claimed bit-identical to the old game.

`tools/v24/browser-ads.cjs` runs the real WASM renderer with deterministic render steps and synthetic DOM input in a hidden browser. It captures all seven weapons at hip, mid-raise, settled ADS, crouched ADS and released. This is input/rendering evidence, not server behavior or a performance benchmark. `tools/v24/network-smoke.cjs` creates a disposable one-player lobby on a real server and verifies protocol gating, authoritative ADS/crouch accuracy, spread recovery, shots and movement from received states. Run it first on a separate loopback server before the live upgrade. Neither harness drives operating-system input.

Release-candidate evidence: 508 native tests passed, four existing tests remained ignored, and all-target Clippy passed with warnings denied. The final test/lint/Arena WASM/native-server build took 46.959 s at Idle priority; the other four WASM bundles were rebuilt in 9.723 s. Pages fixtures passed 41/41 and JavaScript syntax fixtures 91/91. The real loopback server smoke passed in 6.124 s with 182 states, three shots, measured walking/crouching speeds of 2.000/1.100 m/s and protocol-17 rejection. All 35 final browser captures passed in 49.298 s without errors or warnings; all five contact sheets were reviewed and the corrected sight poses independently inspected. The diagnostic/Julibrot smoke verified the new exports and a completed Julibrot scene, not a full benchmark. The final Arena WASM is 41,423,169 bytes with SHA256 `d39d8023a0576d1967ff5cbfef475f10c98e5ab9f1bc7fe2b00fe4bae715f117`. Local evidence is under `target/ads-final`, `target/ads-network-final-local` and `target/ads-newgame-check`; it is deliberately not included in the source release.

Limits remain: the inherited character and weapon art are not AAA assets; hands are static authored finger poses, reloads do not yet exchange magazines by hand, and nearby walls can intersect the viewmodel because it shares the world depth buffer. This release changes handling and sight placement, not the rendering pipeline or projectile prediction model.
