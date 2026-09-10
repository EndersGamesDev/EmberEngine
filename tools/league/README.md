# League tools

This tree holds Ultimate League's reproducible art preparation, private browser and multiplayer evidence harnesses, landing and trailer assembly, host compatibility proofs, and retained historical Pages preparation tools. The preparation tools never publish remotely because release assets and Pages are owned by the tag workflows.

The scripts consume committed game and web sources plus reviewed generator records, then produce artifacts or evidence used by `crates/league`, `crates/league-server`, and `web/games/league`; test-named scripts pin publisher scope, deployment isolation, and server compatibility without contacting public services.

Release intent and acceptance criteria live in `docs/plans/ultimate-league.md` and its v2–v4 successors, launch media rules live in `docs/plans/league-launch-story.md`, and shared generated-asset constraints live in `docs/asset-pipeline.md`; each operational script's header states its own isolation and input limits.
