# End Game 5.0.0 — The Warden’s Knife

Version-local Ember browser shell for an articulated prison warden. He wakes, plants his feet, rises from his chair and draws a knife before pursuing the player. Knife attacks visibly anticipate, make one contact check and recover; stepping away or dodging avoids the contact. Heavy greatsword hits can interrupt an upright warden. The chair stays behind when he walks.

The V4 greatsword rhythms remain: one tap for Cut, three quick taps for Wolf’s Fang, tap-pause-tap for Gravebreaker, and tap-tap-pause-tap for Rising Wolf. Quick gaps are at most 0.30 seconds; delayed gaps are greater than 0.30 and at most 0.90 seconds. Left click, gamepad R2/R1 and touch Strike use the same input rules. The enemy status and health bar accompany the visible knife telegraph; damage produces camera, sound and haptic feedback.

Build with `tools/end-game/build.ps1`, serve `web/`, and open this directory. This release retains V3 pickup hands and V4 sword combos; scoped publication preserves the complete V1–V4 pages and bundles. Browser gameplay and physical devices require their own playtesting.
