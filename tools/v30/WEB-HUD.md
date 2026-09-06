# Killshot v30 web contract

`web/games/arena/v30/index.html` and `settings.js` are new frozen-release sources. Never rewrite v29 or another game's page to release v30. Keep the catalog ID `arena` for existing discovery and handover URLs; its user-facing name is now Killshot. Protocol 23 is required.

## Gameplay telemetry

Rust calls `window.emberUpdateKillshotHud(snapshot)` with either a JSON string or an object at no more than 20 Hz. The object fields are `visible`, `alive`, `hp`, `max_hp`, `weapon`, `magazine`, `reserve`, `reload_remaining`, `reload_duration`, `slots`, and `selected_slot`. Reserve `null` means infinite reserve; zero means empty. Reload fields use seconds. `slots` is a nine-element array of weapon IDs or zero for unowned slots; `selected_slot` is one-based, zero for none. Weapon IDs are 1 Sidearm, 2 Vityaz, 3 AK-47, 4 M4, 5 Revolver, 6 Sniper, and 7 RPG-7. Slots 8 and 9 are reserved and must never grant an invented weapon.

The HUD is presentation-only: LIFE, magazine/reserve, slot ownership and reload completion come from authoritative Rust state. JavaScript does not advance reload timers or grant health/ammunition. Invalid telemetry is clamped or hidden without throwing into the game loop. The entire HUD has `pointer-events: none`; it contains no gameplay click handlers. The small reload ring follows elapsed fraction with a numeric remaining-seconds label and accessible progress text. Reduced-motion users do not receive its CSS interpolation.

## Input and persistence

The settings table now has 22 actions: the existing 13 plus `slot1` through `slot9`, defaulting to physical `Digit1` through `Digit9`. Rust exposes `Action::Slot1` through `Action::Slot9` and `Settings::selected_slot(&InputState) -> u8`, returning zero for no request and the lowest-numbered held slot deterministically. The online client owns fresh-press edge detection; the server owns inventory validation. Paused settings neutralize every slot like all other actions.

New preferences use `ember-killshot-settings-v1`. When absent, the page imports the old 13-action `ember-arena-settings-v1` table without modifying that old key. Existing number-key bindings remain intact; conflicting new slots receive unused keys, announced in Settings. The hotbar displays the actual configured slot key rather than assuming the defaults. This separation prevents opening a frozen pre-v30 page from resetting a user's new settings or vice versa.

## Starting loadout

The host's Create sends `loadout: "classic" | "custom"` and `starting_weapon: 1..7`, independent of `mode: "ffa" | "tdm" | "hill"`. Classic always sends weapon 1 even if the disabled selector retains an earlier value. Custom sends the selected weapon and applies equally to every player's spawn/respawn, with a backup sidearm in slot 1. Joining does not override the lobby's choice. `LobbyInfo` and `GameJoined` report canonical loadout and starting weapon; the lobby browser displays that choice. Collected weapons persist until death, not across respawn, as the menu explicitly explains.

## Verification

Run `node --experimental-vm-modules tools/v30/settings.test.cjs` for configuration, migration, pure HUD sanitization, the actual renderer against a small DOM stand-in, inline syntax, and denied/quota-storage startup checks. The initial 129 checks passed in 11.285 ms; syntax plus this suite took 0.132 seconds at Idle priority. These are not browser layout or GPU verification.

`browser-killshot.cjs` retains the previous actual-WASM controls suite and adds a private protocol-23 custom/Classic lobby, LIFE/ammo/nine-slot HUD checks at 390 and 1600 pixels, reload cancellation and ammo conservation, all seven weapon reload captures, paused slot safety and passive second-player state agreement at matching server ticks. It accepts the same `EMBER_QA_PLAYWRIGHT`, `EMBER_QA_BROWSER`, `EMBER_QA_SERVER`, and `EMBER_QA_OUTPUT` environment variables as the v29 fixture. It refuses occupied ports 8088 and 7788, owns its disposable server, hides all browser/server windows, uses only authored DOM events, and stubs OS-facing focus/pointer/fullscreen APIs. No trusted-user-gesture, physical controller or native settings UI claim is made. The root worker must run the GPU fixture serially and record its actual result before publishing; its initial syntax check is not an execution claim.
