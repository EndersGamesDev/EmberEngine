# End Game 4.0.0 — Weight of the Blade

Version-local Ember browser shell for heavy two-handed sword combat. Build with `tools/end-game/build.ps1`, serve `web/`, and open this directory. Keyboard/mouse, standard gamepad and touch feed the same fixed-step combat and interaction state machines. Tap left click, R2/R1 or the touch Strike button; holding attack does not repeat.

| Input rhythm | Sequence |
|---|---|
| One tap | Cut |
| Wolf's Fang: tap, tap, tap | Cut → Backhand → Finisher |
| Gravebreaker: tap, pause, tap | Cut → Overhead |
| Rising Wolf: tap, tap, pause, tap | Cut → Backhand → Rising |

Quick gaps are at most 0.30 seconds; delayed gaps are greater than 0.30 and at most 0.90 seconds, measured between consecutive presses. Valid follow-up taps queue while the current strike completes its windup, contact, follow-through and recovery. Damage is evaluated at the authored contact time against the target's current position, range and facing. A confirmed hit produces hitstop and impact feedback; a miss completes its recovery without impact feedback.

V4 retains the V3 articulated gauntlets, five-finger grip presets and shared item sockets. The character approaches and crouches for floor items, reaches to the object, closes individual finger joints and carries it at a shared grip socket. The board lifts aside, the key is pinched and pocketed, the lock turns before the gate moves, and both hands draw the greatsword. Pickups temporarily use first-person presentation and restore the selected camera afterward. Release assembly preserves the existing V1, V2 and V3 pages and bundles.

For an unattended native combat frame, set `EMBER_CAPTURE_PATH`, `END_GAME_SCENE=combat`, and `END_GAME_STRIKE=cut`, `backhand`, `finisher`, `overhead` or `rising`. `END_GAME_ACTION_TIME` selects elapsed strike time in seconds and defaults to that strike's contact time. Optional `END_GAME_IMPACT=1` adds the staged impact presentation for inspection; it does not prove a gameplay collision. Capture mode renders the Ember scene without desktop capture, focus activation or machine input and exits after one frame.
