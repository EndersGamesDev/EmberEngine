# Entscheidungen

Kurze Begründung nicht offensichtlicher Entscheidungen, chronologisch nach Phasen.

## Phase 0: Gerüst

### Projekt-Scaffold trotz Umlaut im Pfad

`create-next-app` leitet den Paketnamen aus dem Ordnernamen ab und lehnt
„Verschätz dich“ ab (Großbuchstabe, Leerzeichen, Umlaut). Das Projekt wurde
deshalb in einem Temp-Ordner `verschaetz-dich` erzeugt und anschließend per
`rsync` in den Zielordner kopiert. `package.json` trägt den Namen
`verschaetz-dich`; der Ordnername bleibt wie vorgegeben.

### Versionen

Next.js 16.3.4 (App Router, Turbopack), React 19.2.8, Tailwind CSS 4.3.3,
Zod 4.5.4, Motion 13.2.0, Vitest 5.0.0, Playwright 1.62.1.

### Tailwind v4 statt Config-Datei

Tailwind 4 braucht keine `tailwind.config.ts`. Die Design-Tokens aus
Abschnitt 6.1 stehen als CSS-Variablen in einem `@theme`-Block in
`src/app/globals.css`. Tailwind leitet daraus automatisch die Utilities ab:
`--color-bg` wird zu `bg-bg`, `text-bg`, `border-bg` usw. Damit gibt es genau
eine Quelle für die Farben, statt Tokens doppelt in CSS und Config zu pflegen.

### Schriften

Display-Schrift „Luckiest Guy“ (Variable `--font-display`), Fließtext „Nunito“
(`--font-body`), beide über `next/font/google` im Root-Layout. „Luckiest Guy“
hat den dicken, comichaften Spieleschachtel-Look aus Abschnitt 6.2 und nur
einen Schnitt (400), was den Font-Payload klein hält. Nunito ist rund und
freundlich und passt damit zur Display-Schrift, bleibt aber gut lesbar.

### Zod 4: angepasste Schema-Syntax

Die Vorlage in der Spec ist Zod-3-Syntax. In Zod 4 gibt es `z.string().url()`
nicht mehr, der Format-Check heißt `z.url()`. `src/data/schema.ts` nutzt daher
`z.url().startsWith("https://")`. Alle Exportnamen bleiben unverändert.
`z.number().finite().positive()` und `.strict()` funktionieren in Zod 4
weiterhin und bleiben wie vorgegeben.

### Validierung beim Import

`src/data/questions.ts` ruft `questionsSchema.parse(...)` direkt beim
Modul-Import auf. Dadurch schlägt schon `next build` fehl, wenn eine Frage das
Schema verletzt — die Datenvalidierung braucht keinen separaten Schritt.

### tsconfig: `.next` ausgeschlossen, eigener Layout-Props-Typ

`exclude` enthält `.next`, damit `tsc --noEmit` nicht die generierten
Build-Artefakte mitprüft. Als Nebenwirkung ist der globale Typ
`LayoutProps<"/">` (deklariert unter `.next/types`) nicht verfügbar. Das
Root-Layout typisiert seine Props deshalb selbst mit `{ children: ReactNode }`.
`next build` schreibt die tsconfig nicht um; das wurde geprüft.

### Vitest-Config als `.mts`

Als `vitest.config.ts` warnte Vite beim Laden („ESM syntax in a file loaded as
CommonJS“). Da `pnpm check` warnungsfrei sein muss und `"type": "module"` in
der `package.json` unnötig weit in das Next.js-Setup eingreift, heißt die Datei
`vitest.config.mts`.

### Vitest greift keine Playwright-Specs auf

`include` ist auf `tests/unit/**/*.test.{ts,tsx}` eingeschränkt. Playwright
nutzt `*.spec.ts` unter `tests/e2e`; beide Muster überschneiden sich dadurch
nicht, auch wenn später weitere Testarten dazukommen.

### Playwright: `next dev` auf Port 3170

Der `webServer` startet `pnpm dev --port 3170`, mit `reuseExistingServer: !process.env.CI` und 180 s Timeout.
Der Dev-Server wurde dem Produktions-Build vorgezogen, weil er in der
Entwicklungsschleife der Phasen 3 bis 5 gebraucht wird und dort einen bereits
laufenden Server wiederverwenden kann, statt bei jedem Lauf neu zu bauen.
Port 3170 statt 3000, damit ein nebenher laufender `pnpm dev` nicht kollidiert.

Der Port ist zusätzlich über `PLAYWRIGHT_PORT` überschreibbar, Standard
ist 3170. Grund: `reuseExistingServer` prüft nur, ob auf dem Port
überhaupt etwas antwortet — belegt ein fremder Prozess den Port, testet
Playwright kommentarlos die falsche App. Genau das ist beim Einrichten
passiert (siehe nächster Abschnitt), und die Env-Variable ist der Ausweg,
ohne fremde Prozesse abzuschießen.

Beide Playwright-Projekte (`desktop`, `mobile`) laufen auf Chromium; die
Mobile-Variante setzt zusätzlich `isMobile` und `hasTouch`, was ausschließlich
Chromium unterstützt.

### `PROMPT.md` in `.prettierignore`

Die Spezifikation ist eine Vorgabe und soll unverändert bleiben, also darf
`pnpm format` sie nicht umformatieren.

### Port 3100 war beim Einrichten fremdbelegt, deshalb 3170

Auf der Entwicklungsmaschine lief während Phase 0 ein verwaister `npm run start`
eines anderen Projekts auf Port 3100. Playwright hat diesen Server wegen
`reuseExistingServer` wiederverwendet und die fremde App getestet. Der fremde
Prozess wurde bewusst nicht beendet, er gehört nicht zu diesem Projekt.
Stattdessen ist der Standard-Port des Projekts jetzt 3170 (frei), weiterhin
per `PLAYWRIGHT_PORT` überschreibbar.

### `AGENTS.md` und `CLAUDE.md` sind generiert

Next.js 16 schreibt beim ersten `next dev` selbsttätig eine `AGENTS.md` (mit
Hinweisen für Coding-Agents) und eine `CLAUDE.md`, die per `@AGENTS.md` darauf
verweist. `create-next-app` wurde zwar mit `--no-agents-md` aufgerufen, der
Dev-Server legt die Dateien aber trotzdem an und erzeugt sie nach dem Löschen
erneut (`node_modules/next/dist/server/lib/generate-agent-files.js`). Sie
bleiben deshalb im Projekt und werden mitcommittet — so bleibt der Arbeitsbaum
sauber. In `.prettierignore` stehen sie, damit ein spaeter geaenderter
Generator nicht `pnpm format:check` rot färbt.
