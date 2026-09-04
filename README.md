# Verschätz dich

> Daneben ist auch drin.

Ein Schätzspiel als Next.js-App: schräge Frage, Zahl schätzen, Auflösung mit
Erklärung und Quellen. Läuft komplett im Browser, ohne Backend.

## Loslegen

```bash
pnpm install
pnpm dev
```

Die App läuft dann auf <http://localhost:3000>.

## Wichtige Befehle

| Befehl          | Wirkung                                        |
| --------------- | ---------------------------------------------- |
| `pnpm dev`      | Entwicklungsserver                             |
| `pnpm build`    | Produktions-Build                              |
| `pnpm check`    | Lint, Typecheck, Unit-Tests und Build am Stück |
| `pnpm test`     | Unit- und Komponententests (Vitest)            |
| `pnpm test:e2e` | End-to-End-Tests (Playwright, Port 3170)       |
| `pnpm format`   | Alles mit Prettier formatieren                 |

Der Stand ist Phase 0 (Gerüst). Spiellogik, Fragenbestand und UI folgen in den
nächsten Phasen; diese Datei wächst dann mit.
