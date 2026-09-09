# End Game V11 — Weight and Impact

The versioned browser shell loads the Ember Rust/WASM game and provides controller, keyboard/mouse and touch controls. The castle contains sword and spear soldiers, hollow axe knights and the one-eyed Castellan. Read inscriptions, recover the boss's crown seal, awaken the three vows and leave through the west garden sally-port.

`main.js` owns input, pause, settings, sound and the HUD. `castle-ui.js` renders discovered journal entries and enemy warnings from simulation state; `castle-audio.js` queues the generated castle voice clips with subtitles, pause offsets and duplicate-event protection. `dialogue.js` retains the prison warden dialogue and shared audio output. `quality.js` selects resolution within the display and frame-time budget.

The journal is inside Menu, with Earlier/Later controls for controller navigation. Healing shrines use the same Interact action as clues. The castle entry checkpoint retains defeated enemies, discovered clues, shrine usage and seal progress until the page is restarted. A browser reload begins a new run.

Build and release entry points are `tools/end-game/build.ps1` and `tools/end-game/publish.py`. The publisher preserves V1–V10 and all unrelated launcher entries. Generated bindings and WASM under `pkg/` are build output.

Normal strikes use left click, R1, or Strike. Dedicated heavy attacks use nonrepeating R, middle click, R2, or Heavy; jumping first with Space, Cross, or Jump produces a downward airborne cut that holds its follow-through until landing. The touch Heavy button shares the existing three-row action area, with Guard beside it. Guard, pause and the ending suppress new attacks.

The crosshair follows `combat.aimProjection` from the actual frame camera: viewport x is `0.5 + 0.5*x/aspect` and y is `0.5 - 0.5*y`. Invalid or offscreen projections hide the armed reticle; interaction before the sword keeps the central reticle. Only the text is clamped for readability. Its label shows the simulation's aimed head/torso/leg zone. A separate brief label appears only for confirmed impacts and clears with the simulation timer; aiming alone is not treated as a hit. The combo HUD distinguishes connected cuts, windup/follow-through and waiting for Jump Heavy to land. Authored WebAudio cues distinguish body, stone/paving, iron, timber and grass through the existing pause and mute gates. No new generated sound assets are used.

`tools/end-game/guard.test.mjs` executes the actual shell with passive DOM, game API and audio graph doubles. It covers heavy key/mouse/touch input, repeats, guard/pause/ending exclusion, aim/impact timers, aspect-aware projected aim, invalid/offscreen reticles and surface sound routing. These checks do not drive a browser or verify device controls, audible playback or responsive rendering.
