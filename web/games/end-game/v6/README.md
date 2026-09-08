# End Game 6.0.0 — The Warden’s Voice

Four prerecorded warden reactions accompany audible footsteps in the cell, unlocking the gate, claiming the greatsword and the warden’s death. Crouching is quiet; movement barks have a twelve-second simulation cooldown. Story lines occur once per life. The fixed-step simulation exports a bounded event history with a life ID, so a skipped display frame cannot lose or repeat a reaction.

`dialogue.js` decodes the four mono PCM clips after the same user gesture that enables game audio. One voice plays at a time; a story reaction interrupts a movement bark and death interrupts all live speech. The pause menu saves the audio offset and caption time. Respawning clears the old voice and queue. Subtitles remain available when muted, audio loading fails or the browser blocks audio. Ambience drops during speech. The death line may finish over chapter completion.

Build with `tools/end-game/build.ps1`, serve `web/`, and open this directory. The build copies the four versioned voice assets beside the browser shell; the scoped publisher hashes every clip and preserves V1–V5. V3 pickup hands, V4 sword combos and the V5 knife-fighting warden remain. Existing mouse, standard gamepad and touch controls trigger the same simulation events.

Automated playback tests use a controlled audio adapter and cover queue priority, pause offsets, mute, failed/blocked audio, skipped snapshots, respawn and completion. Audio generation and offline signal/transcription evidence are in `assets/end-game/v6/README.md`. Browser playback, physical devices and listening require their own review.
