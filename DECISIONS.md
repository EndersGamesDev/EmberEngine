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

## Phase 1: Spiellogik

### Highscore je Rundengröße statt eines globalen Bestwerts

`Highscores` ist ein `Partial<Record<RoundSize, Highscore>>`, gespeichert unter
`verschaetz-dich:highscores`. Ein Bestwert über alle Rundengrößen hinweg wäre
sinnlos: 40 von 50 Punkten (80 %) und 63 von 100 Punkten (63 %) sind absolut
nicht vergleichbar, und die 20er-Runde würde jede 5er-Runde automatisch
schlagen. `saveHighscore` ersetzt einen Eintrag nur, wenn die Punktzahl echt
größer ist – bei Gleichstand bleibt der ältere Eintrag stehen, damit „neuer
Highscore“ auch wirklich etwas Neues meint.

Beim Lesen wird nach dem Feld `count` des Eintrags indiziert, nicht nach dem
JSON-Schlüssel: JSON-Schlüssel sind immer Strings, `count` ist typsicher eine
`RoundSize`.

### Reset-Regel der Fragenauswahl

`selectQuestions` zieht zuerst ausschließlich Fragen des Kategoriefilters, die
noch nicht in `playedIds` stehen. Reichen diese nicht für die gewünschte
Rundengröße, gilt der gefilterte Pool als durchgespielt und wird zurückgesetzt
(`poolReset: true`). Dabei gilt:

- Die verbliebenen ungespielten Fragen sind garantiert in der Runde – sonst
  könnten einzelne Fragen über viele Runden hinweg nie erscheinen.
- Aufgefüllt wird aus den bereits gespielten Fragen desselben Filters, nie
  doppelt innerhalb einer Runde.
- Zurückgesetzt wird nur der gefilterte Teil: `playedIds` anderer Kategorien
  bleiben unverändert erhalten. Wer nur „Sport“ durchspielt, verliert damit
  nicht den Fortschritt in allen anderen Kategorien.
- `count` wird auf die Größe des gefilterten Pools gekappt; bei einem Filter
  mit 4 Fragen ergibt eine 10er-Runde eben 4 Fragen statt einer Endlosschleife.

Die Zufallsquelle (`random`) ist injizierbar (Standard `Math.random`), damit
die Auswahl im Test deterministisch prüfbar ist. Gemischt wird mit
Fisher-Yates.

### Parse-Regeln für `parseGermanNumber`

Deutsche Eingaben sind mehrdeutig: „1.250“ kann 1250 oder 1,25 meinen. Die
Funktion entscheidet nach festen Regeln, statt zu raten:

- Erlaubt sind nur Ziffern, Punkt, Komma und Leerraum. Kein Vorzeichen, kein
  Exponent, keine Einheit – Antworten sind per Schema immer positiv, und „12
  km“ soll als Fehleingabe auffallen statt still zu 12 zu werden.
- Höchstens ein Komma. Wenn eines da ist, ist es immer das Dezimaltrennzeichen;
  Punkte davor müssen eine gültige Tausendergruppierung bilden („1.250,75“ →
  1250,75, „1,250.5“ → ungültig).
- Ohne Komma: Mehrere Punkte müssen dem Muster `\d{1,3}(\.\d{3})+` folgen
  („1.250.000“ → 1250000, „1.25.000“ → ungültig).
- Ein einzelner Punkt ist ein Tausenderpunkt, wenn er dem Muster oben
  entspricht („1.250“ → 1250), sonst ein Dezimalpunkt („12.5“ → 12,5,
  „1.2345“ → 1,2345). Die Drei-Ziffern-Regel ist die einzige, die zu dem passt,
  was Leute tatsächlich tippen; Handytastaturen bieten oft nur den Punkt an.
- Leerraum innerhalb der Zahl wird entfernt („1 250 000“ → 1250000).
- Leere oder ungültige Eingaben ergeben `null`, ebenso alles, was keine
  endliche Zahl ergibt.

### Epsilon an den Punkte-Schwellen

Die Schwellen 1,05 / 1,15 / 1,3 / 1,5 / 2 sind in IEEE-754 nicht exakt
darstellbar. Für glatte Fälle stimmt der Vergleich zwar (115/100 trifft
denselben Double wie das Literal 1.15), bei krummen Antworten aber nicht:
3,45 / 3 ergibt 1.1500000000000001 und läge ohne Toleranz eine Stufe zu tief.
`scoreGuess` vergleicht deshalb mit `ratio <= schwelle + 1e-9`. Das Epsilon ist
um Größenordnungen kleiner als jeder Punkteunterschied und größer als der
Rundungsfehler in diesem Wertebereich.

### `START` nur aus dem Zustand `idle`

Der Zustandsautomat ist bewusst eng: `START` wirkt nur in `idle`, `SUBMIT_GUESS`
und `SKIP` nur in `question`, `NEXT` nur in `reveal`. Jede Aktion im falschen
Zustand gibt dieselbe State-Referenz zurück, damit React bei einem versehentlich
doppelten Dispatch (Doppelklick, Enter-Wiederholung) nicht neu rendert und keine
zweite Bewertung entsteht. Für die UI heißt das: Nach dem Ergebnis erst `RESET`,
dann `START`.

### Große Ganzzahlen werden als `BigInt` formatiert

`Intl.NumberFormat` rundet einen Double intern auf rund 16 signifikante Stellen
und macht aus 43.252.003.274.489.856.000 (den Stellungen eines Zauberwürfels)
„43.252.003.274.489.860.000“. Als `BigInt` formatiert Intl exakt den Wert des
Doubles. `formatNumber` wechselt deshalb bei Ganzzahlen oberhalb von
`Number.MAX_SAFE_INTEGER` auf den BigInt-Pfad.

### `ratio: Infinity` überlebt JSON nicht von allein

`scoreGuess` liefert für ungültige Schätzungen (und für „Keine Ahnung“)
`ratio = Infinity`. JSON kennt kein Infinity, `JSON.stringify` macht daraus
`null`. Das Session-Schema akzeptiert deshalb `number | null` für `ratio` und
setzt `null` beim Lesen wieder auf `Infinity`. Ohne diese Sonderbehandlung
würde ein Reload jede Runde mit einer übersprungenen Frage komplett verwerfen.

### `try/catch` um die Speicherzugriffe

Abschnitt 9 verbietet, Fehler mit `try/catch` zu verstecken. In `storage.ts`
ist der Zugriffsschutz trotzdem gesetzt und auf genau drei Stellen begrenzt
(Zugriff auf das Storage-Objekt, `getItem`/`setItem`/`removeItem`, `JSON.parse`).
Grund: Safari im Privatmodus und Browser mit blockierten Website-Daten werfen
schon beim Lesen der Property `window.localStorage`, und ein volles Kontingent
lässt `setItem` werfen. Ein gespeicherter Spielstand ist Komfort, kein
kritischer Wert – die App muss ohne ihn weiterlaufen. Versteckt wird dabei
nichts: Ungültige oder fehlende Daten führen zu den dokumentierten
Standardwerten (`{}`, `[]`, `null`, Standard-Settings).

### Zusätzlicher Export `STORAGE_KEYS`

`storage.ts` exportiert die vier Schlüssel (`verschaetz-dich:highscores`,
`:played-ids`, `:settings`, `:session`) als Konstanten-Objekt. Die E2E-Tests aus
Phase 4 müssen `localStorage` gezielt vorbelegen und leeren; ohne diesen Export
stünden die Schlüssel doppelt im Code.

### Test-Fixtures unter `tests/fixtures/`

Die Fragen für die Logiktests kommen aus `tests/fixtures/questions.ts`, nicht
aus dem echten Fragenbestand: Die Logiktests sollen nicht rot werden, wenn sich
eine Frage ändert. Jede Fixture läuft durch `questionSchema.parse`, damit sie
nicht unbemerkt vom echten Datenvertrag abweicht. Der Ordner liegt bewusst
neben `tests/unit/`, weil Vitest nur `tests/unit/**/*.test.{ts,tsx}` einsammelt.

### `check-sources.ts`: erst HEAD, dann GET

Das Skript sammelt alle Quellen-URLs dedupliziert ein (mit den Fragen-IDs, die
sie benutzen) und prüft jede zuerst per `HEAD`. Viele Server – auch große –
beantworten HEAD gar nicht oder mit 405; erst bei einem Status außerhalb von
2xx/3xx folgt deshalb ein `GET`. Gesendet wird ein eigener `User-Agent`
(Wikipedia antwortet ohne einen mit 403) und `Accept-Language: de`, Timeout
15 s per `AbortSignal.timeout`, Nebenläufigkeit 6. Der Exit-Code ist 1, sobald
mindestens eine Quelle tot ist. Der Response-Body wird verworfen, sonst bleiben
Verbindungen unnötig offen.
