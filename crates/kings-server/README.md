# Four Kings server

`kings-server` is the authoritative protocol-1 table host: per-connection threads own bounded WebSocket I/O, and one hub owns every lobby, board, seat assignment, and turn clock.

Clients submit formation, start, and move intents stamped against a turn; the server validates them with `kings-core`, broadcasts a full state after accepted actions, and applies timeout or disconnect elimination itself.

The library exposes `ServerConfig` and `run`, while the production binary supplies the plain-TCP deployment shell and build identity carried in welcomes.

Game rules, timing, architecture, and the server test plan are recorded in [`../../docs/kings-design.md`](../../docs/kings-design.md); host naming follows `docs/hosts.md` and public-input discipline follows [`../../CLAUDE.md`](../../CLAUDE.md).
