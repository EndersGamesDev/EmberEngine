# End Game sword motion and guard

V8 adapts historical cutting directions and connected recoveries to End Game's oversized fantasy sword. The reference is a guide to readable movement, not a claim of historical reconstruction or simulated weapon dynamics.

## Reference and application

[Fiore de'i Liberi's sword in two hands, Getty manuscript folio 23r](https://wiktenauer.com/wiki/Fiore_de%27i_Liberi/Sword_in_Two_Hands), in the transcriptions and translations collected by Wiktenauer, describes descending, rising and crosswise blows, their edge choices and their return into guards. V8 uses those directional families for its descending cut, backhand, finishing cut, overhead and rising cut. The model's blade runs along +X, its sharp edges lie on ±Z and its broad faces on ±Y; the orientation follows each cut's plane so the edge leads through contact.

[Joachim Meyer's sword teaching, part three, Rebecca Garber translation](https://wiktenauer.com/wiki/Joachim_Meyer/Garber_Sword_3P_2023) describes connected redirections after contact and receiving opposing cuts on the strong near the hilt. V8 connects a queued strike's recovery to the next prepared position and uses an angled chest guard with the strong near the hands. Timings, stamina, damage, input windows and feedback remain authored game rules.

## Player rules

One press performs one cut. Three quick presses select Wolf's Fang; press, pause, press selects Gravebreaker; press, press, pause, press selects Rising Wolf. A quick interval is at most 0.30 seconds, and a delayed interval is greater than 0.30 and at most 0.90 seconds. Buffered strikes connect without returning to the resting pose. If stamina cannot fund a queued strike, the weapon returns smoothly without damage or an extra swing event.

Hold right mouse, F, controller L2 or the touch Guard button to block after equipping the sword. The guard takes 0.18 seconds to raise and 0.12 seconds to lower. Existing committed attacks finish before it rises; holding guard rejects new attack presses. Guarding slows movement, prevents sprinting and pauses stamina regeneration. Releasing it restores normal movement and regeneration.

A raised guard covers a 120-degree cone around the player's view. A knife contact must already pass the normal range, wall, gate, dodge and height checks. A successful block costs 28 stamina instead of health and produces a brief impact pause, sparks, recoil, metallic sound and rumble. Insufficient stamina breaks the guard for 0.90 seconds and lets the normal knife damage through. There is no timed-parry bonus. Before the sword pickup, while airborne, during interactions or during a dodge, guard is unavailable.

## Validation and limits

Fixed-step tests cover edge orientation and contact velocity, early/late combo transitions, failed follow-ups, both hand grips, directional block coverage, readiness, stamina, one-contact behavior, input release and pause. Passive native scene captures inspect the authored poses without machine input. Browser gameplay, physical devices, audible output and measured 5K frame pacing require separate validation. Sword damage still uses the existing contact-time target range/cone; full swept-blade collision is a separate extension.
