# Killshot and Arena pages

This directory holds the original local Pong page, the tracked Killshot page sources from v7 through the current release, and the game's landing page.

The landing page is [`index.html`](index.html) with [`landing.css`](landing.css), [`landing.js`](landing.js) and [`media/`](media/): what Killshot is, its weapons, maps and rules, and the trailer. The launcher links to it from the game's card because [`../../games.json`](../../games.json) gives the arena entry a `landing` key; the card's link is generic and any game that grows a landing page gets one by adding that key. It is a separate page from the version directories below, which are the playable builds themselves.

**It is not published yet.** [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh) copies exactly `index.html` and `settings.js` out of the live version directory for this game and nothing else, so these files reach nobody until a publisher ships them, the way [`tools/league/publish-landing.cjs`](../../../tools/league/publish-landing.cjs) ships the league's.

The launcher uses [`../../games.json`](../../games.json) to present their notes and protocol compatibility, while Pages deployment rebuilds v0 and the declared live Killshot directory and leaves older published versions frozen.

Release ownership lives in [`docs/versioning.md`](../../../docs/versioning.md), the current assembly is defined by [`deploy/deploy-pages.sh`](../../../deploy/deploy-pages.sh), and gameplay history belongs in [`CHANGELOG.md`](../../../CHANGELOG.md).
