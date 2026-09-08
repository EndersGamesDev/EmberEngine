# The Awakening browser shell

`index.html`, `style.css` and `main.js` provide the start screen, pause/settings menu, accessible status, PS5-style prompts, touch input and audio. `quality.js` adjusts backing resolution to the display, hardware texture limit and sustained frame time; 5120 pixels wide and 14.7 million total pixels are ceilings, not a guarantee of rendering speed or asset detail. The game downloads and starts only after the player enters. The prologue downloads only when requested.

`cover.png`, `prologue.mp4` and `ambience.wav` come from the user's existing End Game generation workspace. The prologue is 640×384 prerecorded footage with voice-over; it is not gameplay or 5K footage. Generated bindings in `pkg/` are built, not committed to the source branch. Publication writes `version.json` containing exact source identity and per-file hashes.
