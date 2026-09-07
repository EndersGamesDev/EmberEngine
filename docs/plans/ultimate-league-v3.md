# UltimateLegue V3 — controls, Crystalforge stories and launch trailer

The V3 release adds conventional right-click orders, a bindable attack-move
command, a complete Escape menu and a standalone world page with a real video
trailer. It is coordinated through Barza with Fable (fiction), OpenCode (landing
page) and asset-forge (LAN image, sound and video workers).

## Player controls and combat

- Right-click ground to move; right-click an enemy to acquire and automatically
  attack it within the champion's range and cooldown. Left-click operates the UI.
- `A` is the default attack-move binding. It orders movement toward the cursor,
  acquires nearby hostile champions, minions or the enemy core, and resumes the
  destination after a target dies or leaves acquisition range. It does not
  accidentally select allied units, holograms or neutral Courts.
- Escape opens fullscreen, key bindings, a complete tutorial, and resume/leave
  controls. The match keeps running while a menu is open. Menu input does not
  send world orders. All fifteen actions have saved bindings with conflict
  validation; a settings edit commits the entire valid map atomically.
- The tutorial explains draft, practice and online matches, every champion and
  ability, skill points, spells, runes, stats, gold/assists/last hits, the eighteen
  items and six slots, Courts, cores, waves, healing and death. Descriptions are
  based on the actual simulation rather than inferred MOBA conventions.
- An accepted ability takes over the attack recovery animation. Paid attack
  cooldowns and already released projectiles remain intact. Attacks in this
  simulation have no separate cancellable damage windup. Failed casts do not
  cancel a visual. Authoritative source IDs bind cast/attack effects to the exact
  actor, including a caster that just blinked or lunged; overlapping mirrors and
  clones cannot borrow another unit's pose.

SW4RM recoils through its rotor and drone assembly; EmberKnight uses an amber
blade sweep; the Hallow One sends an ivory/mint petal pulse with a hovering turn;
Bog Maw slashes with three claws and a low lunge; Tessera winds and releases a
violet clockwork gear. These are authored game
animations, distinct from the generated illustrations on the world page.

## Fiction and media

The landing route is `games/league/`; all Play links lead to `games/league/v3/`.
The hub gains a plain **Story & trailer** link in the League card. Reading the
page does not create an account. The five origins, motives, flaws and bonds and
three connected chapters come from `story.json`; the accompanying
`league-launch-story.md` is the fiction bible. Chronicles are editorial fiction,
not a playable campaign or a promise of quests.

The trailer is a 44-second H.264/AAC MP4 with native controls, an English WebVTT
track and a poster. It never autoplays. Story illustrations alternate with
clearly marked genuine V3 practice footage from all five champions. The raw
captures use normal input, earned level-one skill points and the actual game
clock; no fabricated snapshots or modified health, damage, mana or cooldowns.
The soundtrack combines MMAudio crystal/flame cues with an original soft
harmonic bed. The reproducible edit is `tools/league/trailer-edit.py`.

LAN generation is serial within each worker. `launch-assets.py` records the job
ID before polling, resumes the same job, refuses an occupied queue, and never
automatically resubmits an uncertain request. Video output receives a decoded
per-frame content check; a successful job status alone is insufficient. Job
`b4fed053c988` was rejected because Wan's temporal VAE GPU decode produced black
frames. Asset-forge owns the worker repair and rerun; no rejected clip is part
of the release. Selected output hashes, prompts, seeds and job IDs are recorded
in `media/manifest.json`; identity-inconsistent image candidates are excluded.

## Compatibility and publication

V3 uses protocol 2 for the new authoritative `AttackMove` command. V1 and V2
remain frozen on protocol 1, with their existing server, tunnel and task
configuration. V3 uses an independent clean runtime, port 7784, task suffix
`v3`, state directory `.ember/league-local-v3-7784`, and host name
`dusky-osprey-league-v3`. No shared cluster model profiles are changed by this
release. No Killshot, Fire Racer or other game's service is restarted.

The source worktree is `C:/Users/end/dev/ember-league-launch`, branch
`codex/league-landing-trailer`. Final publication requires clean source, exact
main CI, tested bundle hashes and a proven public V3 server. The selected-game
publisher changes only V3 and the League catalog entry. A second standalone
landing publisher changes only the four landing files, `media/`, and an exact
League-only link patch applied to the current public hub. It proves unchanged
V1/V2/V3 trees, catalog, host book and all non-League pages.

## Verification and release record

Pre-release gates include simulation/client/server tests, strict core/client
Clippy, the V3 UI/fullscreen suite, broad practice and authoritative browser
checks, isolated Windows deployment fixtures and publication fixtures. The
landing suite checks desktop and 390px layouts, complete real story data,
keyboard readers, images, no-JavaScript/reduced-motion fallbacks and native video
playback, decoded frames, captions and seeking. Final release evidence records
the exact tested hashes and public revisions separately; this plan is not proof
that publication has completed.

The GitHub Wiki URL was checked and redirected to the repository; the Wiki Git
endpoint returned 404. No unavailable Wiki content was inferred. The repository
simulation, kit definitions and documentation are the source of truth.
