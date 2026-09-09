# End Game V10 — The Last Seal

The versioned browser shell loads the Ember Rust/WASM game and provides controller, keyboard/mouse and touch controls. The castle contains sword and spear soldiers, hollow axe knights and the one-eyed Castellan. Read inscriptions, recover the boss's crown seal, awaken the three vows and leave through the west garden sally-port.

`main.js` owns input, pause, settings, sound and the HUD. `castle-ui.js` renders discovered journal entries and enemy warnings from simulation state; `castle-audio.js` queues the generated castle voice clips with subtitles, pause offsets and duplicate-event protection. `dialogue.js` retains the prison warden dialogue and shared audio output. `quality.js` selects resolution within the display and frame-time budget.

The journal is inside Menu, with Earlier/Later controls for controller navigation. Healing shrines use the same Interact action as clues. The castle entry checkpoint retains defeated enemies, discovered clues, shrine usage and seal progress until the page is restarted. A browser reload begins a new run.

Build and release entry points are `tools/end-game/build.ps1` and `tools/end-game/publish.py`. The publisher preserves V1–V9 and all unrelated launcher entries. Generated bindings and WASM under `pkg/` are build output.
