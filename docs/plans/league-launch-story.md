# UltimateLegue launch story — the Crystalforge bible, five origins, three chapters, trailer beats

Editorial fiction for the standalone landing page at `web/games/league/index.html` (Barza 188/191, 2026-09-07). The page reads `web/games/league/story.json` at runtime; this document is the bible behind that file, so anyone extending the fiction (a sixth champion, a fourth chapter, a season) starts from the same world and does not contradict the game.

The fiction is not a feature. UltimateLegue has no quests, no campaign, no cutscenes and no narration; the story lives on the landing page and nowhere in the client. Nothing below may be advertised as playable. Where the fiction leans on the game, it leans on things the sim actually does, listed per chapter so a rule change can be checked against the prose.

## Ownership and contract

Fable owns `web/games/league/story.json` and this document on branch `fable/league-launch-story` from `7f41e04b`. OpenCode owns the landing page files, root (codex) owns media, the game and integration. The file follows the shared `schemaVersion: 1` contract from Barza 191 exactly, with no extra keys: `campaign {eyebrow, headline, tagline, description}`, `world {title, subtitle, paragraphs[]}`, `champions[] {id, name, epithet, role, quote, hook, origin[], motivation, flaw, bond {champion, text}, portrait, storyImage}`, `chapters[] {id, number, title, subtitle, body[], image}`, `features[] {title, text}`. Champion ids are `swarm`, `emberknight`, `hallow`, `bogmaw`, `tessera`, matching the portrait files under `web/games/league/v2/art/`. `storyImage` points at the portrait until root swaps in `./media/<id>-story.webp` (in production, Barza 201); chapter images are `./media/chapter-1.webp` to `chapter-3.webp`, root's to supply, and the page is expected to degrade to text when they are absent.

## The world in one breath

The Crystalforge was never a forge of metal. The Masons grew crystal the way gardeners grow trees: two seeds at either end of a ravine, fed with light until they became the two hearths, with an ivory bridge running the length of the ravine between them. The Masons left four hundred years ago; nobody agrees why. The garden took the ruin back and kept its colours. Whoever puts out the other hearth inherits the forge and everything it ever made. Five have come to collect, and none of them wants the same thing.

Campaign line, from the owner: **One lane. Five reasons to fight.**

## Rules of the world

- **The Masons** are the vanished builders. They made soldiers first (the minions), then one suit of armour with a hearth-spark sealed in it (EmberKnight's) and a census construct (SW4RM), and they had a healer (The Hallow One). They are never on stage and never explained; every champion has a different theory about them, and the theories are the characters.
- **The two hearths** are the cores: blue at the west end, red at the east. In the fiction they are the Masons' two seeds, still burning turquoise. The fountain is the warmth of your own hearth. Putting out the other hearth is the win condition and the legend in one sentence; the campaign description says "the enemy core, the other side's hearth" once so a reader can map the word onto the page's core bars.
- **The bridge** is the lane: ivory slabs running the length of an emerald ravine, arches at both plazas, jade trees along the edges. The ravine below is the fen, and the fen is Bog Maw's.
- **The Courts** are the two smaller crystals off the bridge on their ritual medallions. The North Court sharpens whoever takes it (the damage boon); the South Court quickens (the haste and gold boon). The fiction only ever says "sharpens" and "quickens", so the numbers can move without touching the prose.
- **The beat** is the wave cadence: the hearths march out soldiers in fours every thirty seconds. Chapter 3 turns that into the moment the forge wakes.
- **Time**: the Masons left four hundred years ago. SW4RM counted the whole time. The Hallow One sat down when they left and did not stand up until the forge woke (chapter 3). EmberKnight has been on the bridge "a century" by his own reckoning, which may be wrong. The fen and its mouth predate the bridge; Bog Maw grew out of them to guard the ravine after the Masons came, and was young when the slabs it wears went under. Tessera arrived recently, over the ridge, with the plans.
- **Palette**: ivory (stone, the saint's robe), bronze (fittings, fins, SW4RM's shell, the broken halo, the hook), turquoise (hearth light, SW4RM's lenses, the Court crystals), amber (sap in the cracks, the Knight's visor, Tessera's goggles, the Maw's fungus), jade (the garden, the Maw's hide). On top of that every champion owns one accent, matching the game's own champion colours: SW4RM turquoise (the forge's colour, which is the point), the Knight red and orange (the cloak, the ember cracks), the saint gold and mint (the light under the hood, the ribbons), the Maw green, Tessera violet. A new character should own one accent the way Tessera owns violet.

## Five reasons

| id | hook | motive | flaw | bond | kit in the origin | visual anchors (art-provenance and the selected portrait) |
|---|---|---|---|---|---|---|
| swarm | To close the count. | a finished ledger | each copy keeps the number and loses the reason | → tessera, whom it logs as maker | Q drones "built to count stones, not bite"; W beams; E hologram; R four of itself | bronze sphere, engraved, bronze fins, one great turquoise lens and lesser ones, two drone pods |
| emberknight | To put the fire out. Starting with his own. | be the last thing the forge burns | the demon works, and less knight comes back | → hallow, who keeps standing him back up | Q tornado; W two untouchable seconds; E four seconds of burning blade; R double damage, three steps | black plate with bronze edges, amber slit visor, ember cracks, short red cloak, greatsword |
| hallow | To keep the hearth lit and everyone standing. | nothing in their keeping ends | cannot tell saving from refusing to let go | → emberknight, their worst patient | Q mend; W hymn that quickens for three seconds; E aegis; R the second breath | ivory robe, faceless hood with gold light, broken bronze halo, staff with pale green crystal, mint ribbons |
| bogmaw | To take the ravine back. | open water, the two dam-stones gone | everything it kept, it kept by swallowing | → swarm, whose drones it eats | Q hook; W shroud that feeds half back; E silt lunge that slows on landing; R quake that roots a plaza | moss-green hide, yellow eyes, ivory plates grown into the back, amber fungus, rusted bronze hook |
| tessera | To wind it up again. | restart the forge, give back its time | nobody asked to be rewound, and she has not noticed | → hallow, the other answer to death | Q gear shot that hits the first foe in its path; W three traps; E chrono step; R the Grand Mechanism | plum coat, ivory and bronze segments, silver hair, amber goggles, clockwork pack, violet hourglass on the chest |

Pronouns: SW4RM and Bog Maw are "it", EmberKnight "he", Tessera "she", The Hallow One "they" (a faceless robe with a light in it; the art never gendered it and the fiction keeps that). Because singular "they" blurs after any plural noun, the saint's origin names "the saint" wherever a pronoun would sit next to the Masons or the years. Every origin is three paragraphs and 120–170 words: setting and body, the kit in the world's own words, the wound. The third paragraph must not restate the `motivation` and `flaw` fields, which the card shows beside it.

The bonds form one chain into one loop: bogmaw → swarm → tessera → hallow ↔ emberknight. Every champion is inside somebody else's story, which is what lets three short chapters carry all five.

## Three chapters

Each chapter is one scene, one point of view, under 240 words, ending on a turn. The in-game facts they rest on are listed so a sim change can be checked against the prose.

1. **The Count** (SW4RM). A slab filed as ruin has moved; three drones go down, two come back; the hook goes straight through the hologram and bites stone while the sphere climbs, and the copy fades once its time is up. Rests on: Q sends three drones; E leaves a hologram that no projectile, hook included, can touch or target, and that lasts five seconds; Bog Maw's Q drags the first champion it catches to its feet and roots it.
2. **The Breath** (the South Court). EmberKnight holds the Court too long, the Maw hooks him, the guard buys two seconds and not a third, he falls against the crystal and stands again, hurt but breathing, with the saint's mark on him; Tessera times it from the arch. Rests on: W immunity lasts two seconds; Bog Maw's W shroud; The Hallow One's R, cast before the blow, turns the killing strike into a revive at part health; the South Court quickens.
3. **The Beat** (the bridge). Tessera's three traps at dawn; SW4RM sweeps the reeds with one beam and splits four ways at the knight when he shows; the saint has not moved from the South Court; the North Court's crystal beats, and thirty seconds later both hearths march out ivory soldiers in fours. Rests on: W holds three traps; SW4RM's W is a free skillshot but its R needs an enemy champion under the cursor, so the four copies fire at the knight and never at empty ground; waves of four (three melee, one caster) every thirty seconds from both fountains. The last line lands on the campaign hook.

The hook the chapters leave open is in-world only: the forge is awake and it is making soldiers. It is not a promise of a story mode, a sixth champion or a season; if one of those ever ships, the fiction has room for it, and until then the page must not say so.

## Voice

Short declaratives, concrete nouns, one image per sentence, dialogue in single quotes, no em-dashes, no sentence past about 35 words. British spelling (colour, armour) to match the repo. Numbers in the prose are the game's numbers or none (two seconds, four seconds, three traps, thirty seconds, four per rank). The jokes are dry and belong to the characters (the census, the stopwatch, "both consider this a conversation"). Nothing is grimdark: the world is a garden in the afternoon, and the worst thing in it is that nobody can let anything go.

## Features copy

`features[]` lists shipped gameplay only, as recorded in `docs/plans/ultimate-league.md`: 1v1 and 3v3 with bots filling empty seats; five kits on QWER ranked from level 1 to 12 with per-champion auto attacks; the North and South Court boons (100 s, then the crystal regrows); gold from last hits, kills, assists and captures, eighteen items in six slots; three-of-eight rune pages saved in the browser and two summoner spells; browser play with no install. The V3 controls (right-click move, attack-move), the Escape menu, keybindings and the new-player guide are not listed because Barza 191 keeps them out of story.json, whether or not a version already has them (the live v2 page already has an Escape menu and saved keybinds); root appends them in the same shape if it wants them here.

## Trailer beat sheet (40 s)

Constraints from the fleet brief: Wan 2.2 TI2V-5B on falke64 renders 25 frames at 640×384 in a few minutes, one job at a time, and its first clips came out black (Barza 194–208, fixed by decoding on a CPU copy of the VAE), so a "motion shot" is one to two seconds of gentle motion on an existing plate, palindrome-looped on the page to three or four seconds, and the edit must survive with none of them. It is therefore still-first: SDXL plates with slow scale and drift in the page's own player, three real motion shots as accents, and one clearly labelled segment of real gameplay. No spoken narration; captions carry the words and go into `trailer.vtt`.

| t (s) | beat | picture | source | caption | sound |
|---|---|---|---|---|---|
| 0–3 | black, one turquoise pulse | dark frame, a single hearth pulse | motion shot 2, first 20 frames, dimmed | — | low crystal pulse |
| 3–9 | the bridge | establishing plate, mist crossing, light shifting | motion shot 1 (looped) or `arena.webp` with a slow push | The Crystalforge was never a forge of metal. | wind, distant water |
| 9–13 | the hearth | the obelisk crystal swelling from dim to bright | motion shot 2 (looped) | Two hearths. One bridge between them. | pulse rises |
| 13–28 | five reasons, 3 s each | portrait, slow drift; accent colour per champion | `v2/art/<id>.webp` in order swarm, emberknight, hallow, bogmaw, tessera | SW4RM — to close the count. / EmberKnight — to put the fire out. / The Hallow One — to keep it lit. / Bog Maw — to take the ravine back. / Tessera — to wind it up again. | one short cue per card: drone whine, ember crackle, choir breath, wet hook, clock tick |
| 28–34 | the lane | real gameplay, labelled "gameplay" in the corner | engine frames (see below) | Real gameplay. | game audio or the crackle bed |
| 34–37 | the blade | ember sparks rising off the greatsword | motion shot 3 (looped) | — | crackle swell |
| 37–40 | title | logo card, Play in the browser | static | One lane. Five reasons to fight. | pulse, then silence |

The five-card block and the title card are page-side stills, which keeps the whole edit deliverable even if no motion shot renders.

## Three motion-shot prompts (Wan 2.2 TI2V-5B, image-to-video)

All three: 25 frames, 640×384, seed fixed and recorded, plate pre-cropped to 5:3 before upload (the model should not be asked to outpaint), negative prompt on every job: `text, watermark, logo, camera shake, fast motion, morphing, new objects, extra limbs, people, flicker, cut, zoom out`. Play at 12 fps and loop as a palindrome; the page should show the poster frame until the clip is loaded, and keep the poster if the clip never arrives. Judge every clip by per-frame pixel statistics, not by file size: the first Wan output on this fleet was 25 black frames in a valid MP4.

1. **Establishing mist** — plate `web/games/league/v2/art/arena.webp`, centre crop. Prompt: `Slow cinematic push-in over an ancient ivory stone bridge crossing an emerald ravine, thin mist drifting slowly from left to right across the bridge, shafts of afternoon light shifting gently through the haze, luminous turquoise crystals softly pulsing, hanging vines swaying slightly, hand-painted fantasy concept art, fixed composition, no characters, no text.`
2. **Hearth pulse** — plate `web/games/league/v2/art/obelisk.webp`, crop to the crystal and pedestal. Prompt: `A massive faceted turquoise crystal held in a bronze and ivory pedestal, its inner light swelling slowly from dim to bright in one long breath, tiny glowing motes rising, soft moss at the base, fixed camera, painterly game art, no text.`
3. **Ember blade** — plate `web/games/league/v2/art/emberknight.webp`, crop to the upper body and sword. Prompt: `A flame knight in black and bronze plate armour standing still, ember cracks in the armour glowing brighter, small embers and sparks rising from the edge of the greatsword, red cloak stirring in a gentle wind, subtle heat shimmer, fixed camera, painterly game art, no text.`

If a fourth slot opens: the saint's halo, `hallow.webp`, `warm gold light under the hood breathing brighter, mint ribbons lifting in a slow wind, motes rising around a broken bronze halo, fixed camera`.

## Sound cues (MMAudio)

Three short cues cover the whole edit: a low crystal pulse with a long tail (the bed under 0–13 s and the title), a dry ember crackle with occasional sparks (the Knight card and the blade shot), and wind over water with reeds (the bridge and the Maw card). The per-card cues in the table are optional decoration; the edit stands on the three.

## Real gameplay without touching the operator's machine

`tools/league/review.ps1` already photographs a hands-off native client (TOPMOST and NOACTIVATE, its own pid, no input). Its `-Shots` list is a string of seconds, so a dense list such as `"70,70.2,70.4,…,76"` yields thirty frames of a live bot match at five frames a second, which assembles into a stop-motion clip that is honestly labelled real gameplay. `LEAGUE_CAM=slot:N` frames one champion, `-Mode squad` gives a 3v3. Root decides whether to use it; it costs no GPU and no fleet time.

## Review record

The first commit (5c1d0341) was reviewed by six independent lenses (kit accuracy against `data.rs`, `kits.rs` and `sim.rs`; visual identity against `art-provenance.json` and the selected portraits; feature and honesty claims; the 191 contract; internal lore consistency; prose), each finding re-checked by a second sceptical reader, plus root's own core audit (Barza 204). Of 83 findings 51 were upheld, and the refinement commit applies them: the hook cannot touch a hologram (projectiles skip clones), so it passes through and bites stone; Hive Overload needs an enemy champion under the cursor, so the four copies fire at the knight and never at empty reeds; Second Breath revives at part health, so the knight stands "hurt but breathing"; Gear Shot hits the first foe in its path; Silt Lunge slows what it lands among and leaves no zone; SW4RM's selected portrait is a bronze orb with many lenses, and the Knight's helm is dark, so the prose follows the pictures; the saint's pronoun is anchored after every plural; the two overlapping four-hundred-year spans became one; the ravine runs the length of the bridge; the third paragraphs no longer repeat the motive and flaw fields. Declined: nothing material; a handful of wording alternatives were merged rather than taken verbatim.

## Verification

Verified on this branch: `story.json` parses with Python's `json` module; the top-level keys and every champion, chapter and feature field match the Barza 191 contract with no additions; the five origins count 156 (swarm), 168 (emberknight), 162 (hallow), 146 (bogmaw) and 168 (tessera) words against the 120–170 band, the premise 158, the chapters 216, 213 and 232 (counted as runs of letters, digits, apostrophes and hyphens); no sentence in the file is longer than 36 words and the JSON prose contains no em-dash; the file is UTF-8 without BOM with LF line endings; the five portrait paths exist under `web/games/league/v2/art/`.

Not verified: the landing page rendering this file (OpenCode's page and root's media are on other branches; OpenCode's first shell d47a4c42 still read the superseded 190 shape, reported on the board), and any trailer render; the beat sheet and prompts are a plan, not a result.

## Backlog

- Story: a fourth chapter when a sixth champion or a season exists, not before.
- Story: when root's five `storyImage` plates land (Barza 201), re-read each origin's first paragraph against its plate the way the portraits already forced two changes (SW4RM bronze, the Knight's helm).
- Trailer: if Wan's clips are proven by pixel statistics, re-render the three shots at 49 frames for smoother loops.
