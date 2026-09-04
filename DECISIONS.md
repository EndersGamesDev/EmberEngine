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

## Phase 2: Fragen

### Fünf Block-Dateien à 20 Fragen, `questions.ts` aggregiert

Die 100 Fragen liegen nicht in einer Datei, sondern in
`src/data/blocks/block-1.ts` bis `block-5.ts` mit je 20 Fragen und den Exports
`block1` bis `block5`. `src/data/questions.ts` importiert die fünf Blöcke, hängt
sie in ID-Reihenfolge aneinander und schickt das Ergebnis durch
`questionsSchema.parse`. Diese Validierung läuft beim Import und damit auch beim
Build: Fehlerhafte Daten brechen `pnpm build` ab, statt erst im Browser
aufzufallen. Der Zuschnitt hat zwei Gründe: Eine Datei mit 100 Einträgen wäre
über 1.400 Zeilen lang und in der Bearbeitung unhandlich, und fünf getrennte
Dateien lassen sich parallel schreiben, ohne dass sich die Agents gegenseitig
überschreiben. Die im Prompt vorgesehene Erweiterung auf 350 Fragen bedeutet
folgerichtig weitere Blöcke (`block-6.ts` und folgende) mit denselben 20er-Paketen
plus je eine Zeile Import und Spread in `questions.ts` – die Blockgrenzen sind
reine Dateiaufteilung und im Datenmodell nirgends sichtbar.

Jeder Block hat zusätzlich einen eigenen Test unter `tests/unit/data/`, der die
20 Fragen für sich prüft (IDs, Schema, zwei Fragen je Kategorie). Der
Gesamtbestand wird davon getrennt in `tests/unit/questions.test.ts` geprüft. So
zeigt ein roter Test sofort, ob ein einzelner Block kaputt ist oder eine Regel
erst über den ganzen Bestand verletzt wird.

### Redaktionsprozess: fünf Agents mit getrennten Themen-Lanes

Die Blöcke wurden parallel von fünf Agents geschrieben. Damit sich die Fragen
nicht doppeln, bekam jeder Block dieselbe feste Kategorie-Reihenfolge (zwei
Fragen je Kategorie), aber innerhalb der Kategorie eine eigene Themen-Lane – im
Block 1 zum Beispiel Anatomie und Grundlagen, im Block 3 Rekorde und Extremwerte.
Die ID-Bereiche waren vorab vergeben (q001–q020, q021–q040 …), sodass keine
Kollisionen entstehen konnten. Jede Zahl wurde vor dem Schreiben durch Abruf der
Quelle verifiziert, nicht aus dem Gedächtnis übernommen; unsichere Fakten wurden
verworfen statt geschätzt. Als Quelle dienen vorrangig die deutschen
Wikipedia-Artikel zum Thema, bei englischsprachigen Spezialthemen die englische
Wikipedia, dazu einzelne Primärquellen (WHO, Water Footprint Network, lotto.de,
scinexx).

### Abweichungen von den ursprünglichen Themenvorschlägen

- **Block 2, „Tweet-Limit 140 Zeichen“ → 24-Bit-Farbtiefe 16.777.216 Farben
  (q032):** Der Block brauchte nach der Regel „mindestens zwei Antworten über
  100.000 pro Block“ eine zweite sehr große Zahl. Das Tweet-Limit ist außerdem
  seit 2017 nicht mehr 140, wäre also ein instabiler Fakt.
- **Block 2, „Vatikanstadt 44 ha“ → Grönland 2.166.086 km² (q028):** dieselbe
  Regel; 44 ha ist eine sehr kleine Zahl in einem Block, der ohnehin viele
  kleine Antworten hatte.
- **Block 2, A7-Länge (q040):** auf die belegten 962,2 km korrigiert statt der
  gerundeten Zahl aus dem Vorschlag.
- **Block 3, Elefantenrüssel 150.000 statt 40.000 Muskeln (q041):** Die häufig
  zitierten 40.000 stehen so nicht in der Quelle; der Wikipedia-Artikel „Rüssel“
  nennt „schätzungsweise rund 150.000“ Einzelmuskeln.
- **Block 3, Hyperion 115,85 m (q042):** mit Bezugsjahr 2017 in der Frage, weil
  der Baum weiterwächst und die Angabe sonst still veralten würde.
- **Block 4, „Staaten Afrikas 54“ → Mitgliedstaaten der Afrikanischen Union 55
  (q067):** Die Zahl der Staaten Afrikas ist wegen der Westsahara strittig und
  hätte keine eindeutige Antwort. Die Mitgliederzahl der AU ist dagegen ein
  belegbarer, eindeutiger Wert.
- **Block 4, „Blutmenge circa 5 Liter“ → Vollblutspende 500 ml (q063):** Die
  Quelle nennt für das Blutvolumen nur eine Spanne (fünf bis sechs Liter), womit
  die Regel „genau eine sinnvolle numerische Antwort“ verletzt wäre. Die
  Spendenmenge ist ein fester Wert.
- **Block 4, „Zauberwürfel-Stellungen“ → Lotto-Tippreihen 13.983.816 (q080):**
  43.252.003.274.489.856.000 lässt sich als JavaScript-`number` nicht exakt
  darstellen (jenseits von `Number.MAX_SAFE_INTEGER`), die Antwort wäre also
  schon im Datenmodell falsch.
- **Block 4, Ironman-Schwimmstrecke in Metern statt Kilometern (q074):** 3.860 m
  statt 3,86 km, damit die Eingabe eine Ganzzahl bleibt und nicht an der
  Nachkommastelle scheitert.
- **Block 5, „Zellen des Körpers 37,2 Billionen“ → Nervenzellen im Gehirn
  86 Milliarden (q084):** Die Gesamtzahl der Körperzellen ist in der Literatur
  uneindeutig (Angaben zwischen 30 und 40 Billionen) und damit keine belegbare
  Einzelzahl. Die 86 Milliarden Neuronen sind dagegen ein etablierter Messwert.
- **Block 1:** keine Abweichungen.

### Ergebnis von `pnpm check:sources`

Die 100 Fragen tragen zusammen 104 Quellenangaben – vier Fragen (q017, q020,
q040, q080) führen je zwei Belege. Das Skript prüft dedupliziert und kommt damit
auf 103 verschiedene URLs; der Artikel „Physiologischer Brennwert“ wird von zwei
Fragen genutzt. Beim Lauf über den fertigen Bestand waren 103 von 103
Quellen erreichbar, 0 tot – es musste kein Link ersetzt werden. Das ist kein
Zufall: Die Blöcke wurden beim Schreiben je einzeln gegen das Skript geprüft, und
weil jede Zahl ohnehin durch Abruf der Quelle belegt werden musste, konnte gar
keine tote oder erfundene URL in die Daten geraten.

## Phase 3: UI

### Hydration: `useHydrated()` statt `setState` im Effekt

Alles, was aus `localStorage` oder `sessionStorage` kommt (laufende Runde,
Einstellungen, Bestleistungen, Highscore-Merker), darf erst nach der Hydration
sichtbar werden – sonst weicht der erste Client-Render von dem ab, was der
Server gerendert hat. Der naheliegende Weg (`useState` + `useEffect` mit
`setState` im Effekt) scheitert an der Lint-Regel `react-hooks/set-state-in-effect`,
die in `eslint-config-next` aktiv ist – zu Recht, denn er erzwingt einen
zusätzlichen Renderdurchlauf.

Stattdessen gibt `src/components/useHydrated.ts` über `useSyncExternalStore`
auf dem Server `false` und im Browser `true` zurück; React kennt den Unterschied
zwischen Server- und Client-Schnappschuss und plant die zweite Darstellung
selbst ein. Jeder Bildschirm zeigt bis dahin denselben neutralen Platzhalter mit
derselben Kartenhülle und Mindesthöhe, damit beim Umschalten nichts springt.

### Provider: Lazy-Initializer statt `RESTORE`-Aktion

`GameProvider` nutzt `useReducer(gameReducer, initialGameState, restoreState)`.
Der Initializer liest die laufende Runde direkt aus dem `sessionStorage`, sodass
`src/lib/game-reducer.ts` unverändert bleibt und weiterhin nichts von Persistenz
weiß – ein zusätzlicher Wrapper-Reducer mit `RESTORE`-Aktion wäre nur ein Umweg
zum selben Ergebnis gewesen. Auf dem Server gibt es kein `sessionStorage`, dort
bleibt es beim Startzustand; sichtbar wird der wiederhergestellte Zustand
ohnehin erst mit `hydrated === true`.

Geschrieben wird in einem Effekt, der auf jede Zustandsänderung reagiert: in der
Phase `idle` `clearGameSession()`, sonst `saveGameSession(state)`.

### Der Merker „neuer Highscore“ braucht eigenen Speicher

`saveHighscore` überschreibt den alten Bestwert. Wer danach auf `/ergebnis`
neu lädt, könnte nicht mehr feststellen, ob die Runde ein Bestwert _war_ – der
Vergleich ginge gegen den eigenen, gerade gespeicherten Wert. Deshalb drei neue,
rein additive Funktionen in `src/lib/storage.ts`: `loadNewHighscoreFlag`,
`saveNewHighscoreFlag`, `clearNewHighscoreFlag`, gespeichert im `sessionStorage`
neben der Runde und mit ihr zusammen gelöscht (Unit-Tests in
`tests/unit/storage.test.ts`, Abschnitt „Merker ‚neuer Highscore‘“). Die
Reihenfolge beim Rundenende ist entscheidend: erst `isNewHighscore(...)`
auswerten, dann `saveHighscore(...)` rufen.

### Farben und Kontraste

- **Kategorien und Bewertungsstufen** bekommen helle, kräftige Füllfarben mit
  ink-farbener Schrift und schwarzer Kontur. Alle zehn Kategoriefarben und alle
  sechs Label-Farben (Grün `#57D175` bis Rot `#F2705B`) liegen mit `#1B1B1B`
  deutlich über den 4,5:1 der Stufe AA – der dunkelste Ton, „Naja …“ `#ED7A21`,
  kommt auf rund 6,4:1.
- **`--color-success` (`#1F7A3D`) und `--color-error` (`#C02718`)** wurden auf
  Creme `#FFF8E7` nachgerechnet: 5,1:1 und 5,7:1, beide AA-tauglich. Die Tokens
  bleiben deshalb, wie Phase 1 sie gesetzt hat. Auf dem gelben Grund `#FFC83D`
  reichen sie dagegen nicht (3,5:1 bzw. 3,9:1) – sie werden ausschließlich auf
  Kartenflächen eingesetzt.
- **Akzent `#F2542D` mit ink-Text** liegt bei rund 5:1 und trägt damit auch
  kleine Beschriftungen; weißer Text auf Akzent kommt nirgends vor.
- **`text-ink/70`** (Nebentexte) ergibt auf Creme rund 6,2:1 und wird nur dort
  verwendet.

### Enter-Behandlung in der Auflösung

Zwei Wege führen weiter, und sie dürfen sich nicht überschneiden: Die
Schaltfläche „Weiter“ bekommt beim Erscheinen den Fokus, dort löst Enter ganz
normal den Klick aus. Zusätzlich hängt ein `keydown`-Handler am `document`, der
aber aussteigt, sobald das Ereignisziel in einem `a`, `button`, `input`,
`textarea`, `select` oder `contenteditable` liegt – sonst würde ein Enter auf
der fokussierten Schaltfläche doppelt zählen.

Der Fokus wird mit `focus({ preventScroll: true })` gesetzt. Ohne das scrollt
der Browser die Schaltfläche ins Bild und schiebt auf schmalen Geräten
Wortmarke und Fortschrittsbalken aus dem sichtbaren Bereich.

### Count-up mit `requestAnimationFrame` statt Motion-Value

Der Endwert muss zeichengenau `formatWithUnit(answer, unit)` sein – eine
interpolierte Motion-Value liefert das nicht zuverlässig. `CountUp` hält
deshalb nur den Fortschritt 0…1 als Zustand (gesetzt ausschließlich im
rAF-Callback, nie synchron im Effekt) und rechnet daraus `value * easeOutCubic(p)`.
`easeOutCubic(1)` ist exakt 1, der Endwert also exakt `value`. Ganzzahlige
Antworten zählen ganzzahlig hoch, sonst flackerten unterwegs Nachkommastellen
auf, die es in der Antwort gar nicht gibt. Bei `prefers-reduced-motion` steht
der Endwert vom ersten Render an.

### Konfetti ohne Zusatzbibliothek

`Confetti` erzeugt 40 `motion.span` mit zufälliger Position, Größe, Farbe,
Drehung und Verzögerung in einem `fixed`, `aria-hidden`, `pointer-events-none`
Container und entfernt sich nach 2,5 Sekunden selbst. Die Fallstrecke ist in
`vh` angegeben statt in Pixeln aus `window.innerHeight` – so kommen die Teilchen
unabhängig von der Fenstergröße unten an, ohne dass die Komponente die
Fenstermaße kennen muss. Bei `prefers-reduced-motion` entsteht kein einziges
Teilchen. Die Zufallswerte fallen erst beim ersten Render an, und der findet
immer im Browser statt (die Auflösung wird bis zur Hydration gar nicht
gerendert) – ein Hydration-Unterschied ist damit ausgeschlossen.

### `prefers-reduced-motion` an zwei Stellen

`globals.css` dreht per Media Query alle CSS-Animationen und Übergänge auf
faktisch null (Karten-Einblendung, Badge-Pop, Fortschrittsbalken). Count-up und
Konfetti fragen die Einstellung zusätzlich in JavaScript ab
(`useReducedMotionSafe`, ebenfalls über `useSyncExternalStore`) und laufen dann
gar nicht erst an, statt nur schnell abzulaufen.

### Eingabefeld: `key` statt Zustands-Reset

`EstimateInput` hält seinen Eingabetext selbst. Statt ihn beim Fragenwechsel per
Effekt zurückzusetzen, bekommt die Komponente in `PlayScreen` die Fragen-ID als
React-`key`: Jede neue Frage erzeugt eine frische Instanz – leeres Feld, Fokus
im Feld, kein `setState` im Effekt. `RevealCard` bekommt denselben `key`, damit
der Count-up bei jeder Frage neu losläuft.

### Fallbacks

- **Zwischenablage:** Fehlt `navigator.clipboard` (unsicherer Kontext, alter
  Browser) oder schlägt `writeText` fehl, erscheint der Share-Text in einem
  `readonly`-Feld zum Selbstkopieren. Der `catch` versteckt hier nichts, er
  übersetzt den Fehler in eine sichtbare Alternative.
- **Leerer Kategoriefilter:** „Los geht's“ ist deaktiviert, dazu ein Hinweis in
  einer `aria-live`-Region. `startRound` meldet zusätzlich `false`, wenn die
  Auswahl keine Frage liefert – dann wird gar nicht erst navigiert.
- **Direktaufruf:** `/spielen` ohne laufende Runde leitet auf `/` um,
  `/ergebnis` ohne beendete Runde ebenfalls. Beide Wächter warten auf
  `hydrated`, sonst würde ein Reload mitten in der Runde fälschlich umleiten.
- **Alle Kategorien aktiv** wird als leere Liste gespeichert („kein Filter“),
  damit später ergänzte Kategorien automatisch mitspielen.

### Layout und Screenshots

Der Fragebildschirm hält den Kopfbereich bewusst klein (kleine Wortmarke,
Fortschritt, kein großer Titel). Auf 375 × 667 sind Fortschritt, Kategorie,
Fragetext, Eingabefeld und beide Schaltflächen ohne Scrollen sichtbar
(Unterkante der Schaltflächen bei 462 px), und
`document.documentElement.scrollWidth` bleibt auf allen drei Routen bei 375.
Frage- und Auflösungskarte teilen sich dieselbe Hülle mit `min-h-[24rem]`,
damit beim Wechsel nichts springt.

Die Ergebnistabelle liegt auf `table-fixed` mit festen Spaltenanteilen
(36/23/23/18 %) und `break-words`: Ohne das drücken lange Einheiten wie
„Chromosomen“ die Punktespalte auf schmalen Geräten aus dem Bild.

`devIndicators: false` in `next.config.ts` schaltet das schwebende Dev-Abzeichen
ab – es lag sonst über dem Inhalt und landete in jedem Screenshot. Der
Produktionsbuild ist davon nicht betroffen.

### Test-Setup: `matchMedia` und Aufräumen

jsdom bringt `window.matchMedia` nicht mit. `tests/support/reduced-motion.ts`
stellt einen echten `MediaQueryList`-Ersatz bereit (Unterklasse von
`EventTarget`, damit die Ereignis-Signaturen ohne Casts stimmen);
`tests/setup.ts` hängt ihn vor jedem Test frisch an und setzt ihn auf „keine
reduzierte Bewegung“ zurück. Tests schalten mit `setReducedMotion(true)` um.
Weil `globals: false` gesetzt ist, räumt Testing Library nicht von allein auf –
`cleanup()` läuft deshalb ebenfalls im globalen `afterEach`.

## Phase 4: E2E

### Aufteilung der Specs

Fünf Dateien unter `tests/e2e/`, zehn Tests, dazu `helpers.ts` mit den
gemeinsamen Schritten:

- `smoke.spec.ts` (1): Titel, Claim und Startknopf sind da.
- `runde.spec.ts` (3): komplette Runde mit fünf Fragen, „Keine Ahnung“,
  Tastaturbedienung.
- `persistenz.spec.ts` (4): Reload in der Frage und in der Auflösung,
  Highscore über zwei Runden hinweg, Direktaufruf von `/ergebnis` und von
  `/spielen`.
- `mobile.spec.ts` (1): Layoutprüfung auf 375 × 667, nur im Mobile-Projekt.
- `screenshots.spec.ts` (1): die vier Bildschirme nach `screenshots/`.

Beide Projekte führen alle Tests aus; der Mobile-Layouttest überspringt sich im
Desktop-Projekt selbst. Macht 19 ausgeführte und einen übersprungenen Lauf.

### Determinismus trotz zufälliger Fragen

Die Runde zieht ihre Fragen zufällig – eine feste Erwartung wie „10 Punkte“
wäre damit nicht formulierbar. Die Tests lesen deshalb den Fragetext aus dem
DOM und schlagen die Frage in `src/data/questions.ts` nach; Playwright läuft in
Node und kann die Datei direkt importieren. Die 100 Fragetexte sind eindeutig
(Datentest in `tests/unit/questions.test.ts`) und taugen darum als Schlüssel.

Aus der so bekannten Antwort ergibt sich die Schätzung als Faktor: 1 gibt einen
Volltreffer, 1,1 / 1,25 / 1,4 treffen die mittleren Stufen, 1000 ist eine
sichere Niete. Die komplette Runde landet dadurch in jedem Lauf bei 25 von 50
Punkten, ohne dass eine einzige Frage vorgegeben wäre.

`toInputText` schreibt die Zahl so, wie ein Mensch sie eintippt: ganze Zahlen
unverändert, gebrochene mit Dezimalkomma. Das Komma ist Absicht – ein Punkt
wäre bei genau drei Nachkommastellen („1.234“) ein Tausenderpunkt und damit
mehrdeutig.

Wo die Auswahl selbst festliegen muss, setzt `seedUnplayedQuestions` vor dem
ersten Laden die Liste der bereits gespielten IDs so, dass genau fünf Fragen
übrig bleiben. `selectQuestions` bedient sich zuerst bei den ungespielten, also
steht die Auswahl fest; zufällig bleibt nur ihre Reihenfolge. Der Mobile-Test
bekommt darüber die fünf längsten Fragen des Bestands – den ungünstigsten Fall
für „ohne Scrollen sichtbar“ – und die Screenshots ein festes Fünferpack aus
fünf Kategorien.

### Klicken vor der Hydration

Der Startbildschirm wird auch auf dem Server gerendert. Ein Klick auf
„5 Fragen“, der vor der Hydration ankommt, verpufft wirkungslos: Der Knopf ist
da, der Handler noch nicht. `chooseRoundSize` klickt deshalb in einer
`expect(...).toPass()`-Schleife, bis `aria-pressed="true"` steht. Das ist
zugleich der Beweis, dass die Seite interaktiv ist – ein fester Sleep wäre
entweder zu kurz oder Zeitverschwendung. Alle weiteren Bildschirme brauchen den
Kniff nicht: Frage-, Auflösungs- und Ergebnisinhalt entstehen erst nach der
Hydration (`useHydrated`), vorher steht dort nur der Platzhalter.

### Eigenes Ausgabeverzeichnis für den Testserver

Next 16 lässt pro `distDir` nur einen `next dev` laufen und legt dafür die
Sperrdatei `<distDir>/dev/lock` an. Ein offener Entwicklungsserver – auch auf
einem ganz anderen Port – brachte `pnpm test:e2e` deshalb mit „Another next dev
server is already running“ zum Abbruch, noch bevor ein Test lief.

`next.config.ts` liest den Ausgabeort jetzt aus `NEXT_DIST_DIR` (Standard
bleibt `.next`), und der `webServer` in `playwright.config.ts` setzt die
Variable auf `.next-e2e`. Damit stören sich Entwicklungs- und Testserver nicht
mehr, und der Testlauf klaut dem laufenden `next dev` auch nicht den Cache.
`.next-e2e` steht in `.gitignore`, `.prettierignore`, den ESLint-Ignores und
der `exclude`-Liste der tsconfig. Die beiden `.next-e2e/…`-Einträge unter
`include` schreibt `next dev` selbst hinein; sie stehen nur deshalb in der
Datei, damit der Arbeitsbaum nach einem Testlauf sauber bleibt. Wirkung haben
sie keine – `exclude` sticht.

Der Produktionsbuild ist von alldem nicht betroffen: `next build` schreibt in
`<distDir>` selbst, der Entwicklungsserver in `<distDir>/dev`.

### Zeitlimit 90 Sekunden

Eine komplette Runde mit fünf Fragen ist ein langer Test, und `next dev`
übersetzt jede Route beim ersten Aufruf frisch. Die 30 Sekunden der
Voreinstellung sind dafür knapp; 90 Sekunden lassen Luft, ohne einen echten
Hänger zu verschleiern. Gemessen dauert der komplette Lauf rund 17 Sekunden.

### Warten statt schlafen

Gewartet wird durchweg über die Auto-Wait-Zusicherungen (`toHaveText`,
`toBeVisible`, `toBeFocused`, `waitForURL`). Feste Pausen gibt es nur im
Screenshot-Spec und nur für Animationen: 900 ms für den Count-up (läuft
800 ms) und 400 ms für Karten- und Abzeichen-Einblendung (240 bzw. 360 ms).
Das Konfetti lebt 2,5 Sekunden – beide Aufnahmen der Auflösung müssen in
dieses Fenster passen, deshalb stehen sie direkt hintereinander.

### Screenshots

Zwölf Dateien in `screenshots/`: je Bildschirm und Projekt eine Aufnahme des
Viewports (`start-`, `frage-`, `aufloesung-`, `ergebnis-` mit `-desktop` bzw.
`-mobile`), dazu `aufloesung-…-full.png` und `ergebnis-…-full.png` mit
`fullPage: true`. Die Frage-Aufnahme bleibt bewusst auf den Viewport begrenzt:
Auf dem Handy ist gerade die Frage interessant, ob alles ohne Scrollen zu sehen
ist. Die Auflösung zeigt einen Volltreffer, damit das Konfetti im Bild ist.

Der Zielordner wird über `__dirname` bestimmt, nicht über
`testInfo.config.rootDir` – letzteres zeigt auf `testDir`, die Bilder wären
sonst in `tests/e2e/screenshots/` gelandet.

### Zusatzprüfung gegen `overflow-x: hidden`

`document.documentElement.scrollWidth <= 375` ist die in Abschnitt 8.4
geforderte Prüfung, für sich genommen aber schwach: `body { overflow-x: hidden }`
(nötig für die Kritzeleien) wird auf den Viewport übertragen und deckelt den
Wert ohnehin. `expectNoHorizontalScroll` prüft deshalb zusätzlich, dass der
Inhaltsbereich `<main>` vollständig in den Viewport passt. Über einzelne
Elemente lässt sich das nicht sagen – die Doodles ragen absichtlich hinaus.

### Gefundene Fehler

Keine. Alle sieben Punkte aus Abschnitt 8.4 waren auf Anhieb grün; an `src/`
musste nichts geändert werden, auch keine fehlende `data-testid` (der Vertrag
war vollständig umgesetzt). Die einzige Änderung außerhalb von `tests/`
betrifft das Ausgabeverzeichnis des Testservers und ist eine Frage der
Testumgebung, nicht der App.
