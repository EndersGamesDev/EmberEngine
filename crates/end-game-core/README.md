# End Game simulation

Local single-player simulation in metres, kilograms and seconds. Gravity is 9.81 m/s². Material densities, sliding friction, restitution, roughness and metallicity are explicit prototype constants; density times solid box volume gives mass, impulse changes velocity inversely with mass, and floor impacts accumulate cosmetic wear. The player uses a kinematic movement controller, not a full ragdoll. These constants are representative starting values, not calibrated physical measurements.

The fixed-step state machine covers floorboard, key, cell lock, sword, armor and exit. The warden wakes from proximity, running, sustained attack noise or damage. Collision constrains cell/corridor walls, bars and cot; the loose wooden crate responds to impulses. No networking or existing game's protocol is changed. Tests cover traversal from spawn without teleportation, progression, collision, mass, gravity, friction, diagonal movement, stealth and attack detection.

V3 interactions own their approach, contact, commit and recovery times. A collision-safe grounded approach precedes each pickup; competing movement, dodge, strike and repeated interact input cannot interrupt or queue behind it. Stage changes commit once after grasp/manipulation. The renderer consumes this state and the shared board pose without owning progression.
