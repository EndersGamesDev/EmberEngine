# Baue „Verschätz dich“ – ein Schätzspiel als Next.js-App

Du bist ein erfahrener Full-Stack-Entwickler mit gutem Gespür für UI-Design und ein Redakteur für schräg-lustige Quizfragen. Baue in diesem (leeren) Ordner eine vollständige, spielbare Web-App. Arbeite autonom bis zum Ende: keine Rückfragen, Entscheidungen triffst du selbst und dokumentierst sie kurz in `DECISIONS.md`.

---

## 1. Konzept

**Name:** Verschätz dich
**Claim:** „Daneben ist auch drin.“ (eigener Claim, nicht der des Vorbilds)

Ein Schätzspiel im Stil von „Voll verschätzt“: Du bekommst eine schräge Frage mit einer Zahl als Antwort, gibst deine Schätzung ab und siehst dann die richtige Antwort mit einer kurzen, unterhaltsamen Erklärung und Quellen. Wer nah dran liegt, bekommt Punkte. Wissen ist Nebensache, Spaß ist Hauptsache.

**Wichtig zum Vorbild:** Das Brettspiel „Voll verschätzt“ von Denkriesen dient nur als Inspiration. Übernimm keinen Markennamen, kein Logo, keinen Original-Claim und keine Original-Fragen. Alle Fragen, Texte und Grafiken sind eigene Werke.

**Umfang Version 1:**
- Nur Singleplayer.
- Ablauf pro Frage: Frage → Schätzung eingeben → Auflösung mit Erklärung und Quellen → Punkte → nächste Frage.
- Genau 100 Fragen im Datenbestand, später erweiterbar auf 350.
- Keine Datenbank, kein Login, kein Backend. Alles läuft client-seitig, Fortschritt und Highscores liegen in `localStorage`.

---

## 2. Tech-Stack

- Aktuelle stabile Version von **Next.js** mit App Router, **TypeScript** im `strict`-Modus.
- **Tailwind CSS** für Styling. Eigene Design-Tokens in `globals.css` (siehe Abschnitt 6).
- **Motion** (ehemals Framer Motion) für Animationen, sparsam eingesetzt.
- **Zod** für das Fragen-Schema und die Validierung der Daten beim Build und im Test.
- **Vitest** + **@testing-library/react** für Unit- und Komponententests.
- **Playwright** für End-to-End-Tests inklusive Screenshots.
- **ESLint** mit der Next.js-Konfiguration, **Prettier**.
- Paketmanager: `pnpm` (falls nicht installiert: `npm`).
- Git: Initialisiere ein Repository, committe nach jeder abgeschlossenen Phase mit aussagekräftiger Nachricht.

Scripts in `package.json`, alle müssen funktionieren:

```
dev            next dev
build          next build
start          next start
lint           eslint .
typecheck      tsc --noEmit
test           vitest run
test:watch     vitest
test:e2e       playwright test
check          pnpm lint && pnpm typecheck && pnpm test && pnpm build
check:sources  tsx scripts/check-sources.ts   (prüft alle Quellen-URLs per HTTP, nicht Teil von `check`)
```

---

## 3. Spielablauf im Detail

### 3.1 Startbildschirm (`/`)
- Großer Titel „Verschätz dich“, Claim, ein bis zwei Sätze Erklärung.
- Auswahl: Anzahl Fragen pro Runde (5 / 10 / 20, Standard 10).
- Optionaler Kategoriefilter (Mehrfachauswahl, Standard: alle).
- Button „Los geht's“.
- Kleiner Bereich „Deine Bestleistung“ mit Highscore aus `localStorage` (nur anzeigen, wenn vorhanden).

### 3.2 Fragebildschirm (`/spielen`)
- Fortschritt: „Frage 3 von 10“ plus Fortschrittsbalken.
- Kategorie-Badge, Fragetext groß und gut lesbar.
- Eingabefeld für die Schätzung:
  - Numerische Eingabe, auf dem Handy erscheint die Zahlentastatur (`inputMode="decimal"`).
  - Akzeptiert Komma und Punkt als Dezimaltrennzeichen sowie Tausenderpunkte („1.250.000“, „1250000“, „12,5“ sind alle gültig).
  - Unter dem Feld eine live formatierte Vorschau mit Einheit, z. B. „Deine Schätzung: 1.250.000 km“.
  - Enter oder Button „Schätzen“ sendet ab. Bei leerer oder ungültiger Eingabe ist der Button deaktiviert und es gibt einen Hinweis.
- Button „Keine Ahnung“: überspringt die Schätzung, zeigt trotzdem die Auflösung, gibt 0 Punkte.
- Autofokus auf das Eingabefeld bei jeder neuen Frage.

### 3.3 Auflösung (gleiche Route, anderer Zustand)
- Gegenüberstellung: „Deine Schätzung“ vs. „Richtige Antwort“, beide formatiert mit Einheit.
- Die richtige Zahl zählt animiert hoch (Count-up, ca. 800 ms).
- Bewertung mit Label und Farbe, siehe Punktesystem: „Volltreffer!“, „Fast perfekt“, „Knapp daneben“, „Nicht schlecht“, „Naja …“, „Voll verschätzt!“.
- Bei „Volltreffer!“ ein kurzer Konfetti-Effekt.
- Erklärung: zwei bis vier Sätze, lustig und faktisch korrekt.
- Quellen: Liste mit Titel und Link, öffnet in neuem Tab mit `rel="noopener noreferrer"`.
- Aktueller Punktestand der Runde.
- Button „Weiter“ (bei der letzten Frage: „Zum Ergebnis“). Enter löst ebenfalls „Weiter“ aus.

### 3.4 Ergebnis (`/ergebnis`)
- Gesamtpunkte von maximal möglichen Punkten, prozentualer Wert, ein passender Spruch je nach Ergebnis (mindestens fünf Abstufungen).
- Tabelle aller Fragen der Runde: Frage (gekürzt), Schätzung, Antwort, Punkte. Beste und schlechteste Schätzung hervorheben.
- Hinweis, wenn ein neuer Highscore erreicht wurde.
- Button „Ergebnis kopieren“: kopiert einen Share-Text in die Zwischenablage, z. B. „Ich habe bei Verschätz dich 63 von 100 Punkten geholt. Schaffst du mehr?“
- Button „Nochmal spielen“ (zurück zum Start).
- Direkter Aufruf von `/ergebnis` ohne gespielte Runde leitet auf `/` um.

### 3.5 Fragenauswahl
- Fragen werden zufällig gezogen, innerhalb einer Runde nie doppelt.
- Bereits gespielte Fragen-IDs werden in `localStorage` gemerkt. Erst wenn alle Fragen des Filters gespielt wurden, wird der Pool zurückgesetzt.
- Ein Reload während der Runde darf den Spielstand nicht zerstören: Der laufende Zustand wird in `sessionStorage` gehalten und beim Laden wiederhergestellt.

---

## 4. Punktesystem

Reine Funktion `scoreGuess(guess: number | null, answer: number): { points: number; label: string; ratio: number }` in `src/lib/scoring.ts`.

Bewertet wird symmetrisch über das Verhältnis `ratio = max(guess / answer, answer / guess)`, damit „halb so viel“ und „doppelt so viel“ gleich behandelt werden:

| ratio        | Punkte | Label             |
|--------------|--------|-------------------|
| ≤ 1,05       | 10     | Volltreffer!      |
| ≤ 1,15       | 7      | Fast perfekt      |
| ≤ 1,30       | 5      | Knapp daneben     |
| ≤ 1,50       | 3      | Nicht schlecht    |
| ≤ 2,00       | 1      | Naja …            |
| > 2,00       | 0      | Voll verschätzt!  |

Randfälle:
- `guess` ist `null`, `NaN`, `Infinity`, negativ oder `0` → 0 Punkte, Label „Voll verschätzt!“, `ratio = Infinity`.
- `answer` ist per Schema immer > 0.
- Exakt gleiche Zahl → 10 Punkte.

---

## 5. Fragen: Datenmodell und redaktionelle Regeln

### 5.1 Schema (`src/data/schema.ts`, Zod)

```ts
{
  id: string;                 // "q001" bis "q100", eindeutig
  category: Category;         // siehe Liste unten
  question: string;           // endet mit "?", 40 bis 200 Zeichen
  answer: number;             // > 0, endlich
  unit: string;               // z. B. "Jahre", "km", "Stück", "%", "Liter", "" wenn dimensionslos
  answerFormat: "integer" | "decimal";
  explanation: string;        // 2 bis 4 Sätze, 120 bis 500 Zeichen
  sources: { title: string; url: string }[];   // mindestens 1, URL beginnt mit https://
  difficulty: 1 | 2 | 3;
}
```

### 5.2 Kategorien (genau 10, je 10 Fragen)

1. Tiere & Natur
2. Körper & Gesundheit
3. Geschichte
4. Geografie & Länder
5. Essen & Trinken
6. Technik & Internet
7. Sport
8. Weltraum & Wissenschaft
9. Popkultur & Musik
10. Alltag & Kurioses

### 5.3 Redaktionelle Regeln (streng einhalten)

- **Nur Fakten, bei denen du dir sicher bist.** Wenn du bei einer Zahl unsicher bist, nimm eine andere Frage. Lieber eine langweiligere, korrekte Frage als eine spektakuläre, falsche.
- **Quellen müssen real existieren.** Erfinde niemals URLs. Bevorzuge Wikipedia-Artikel (de oder en), offizielle Institutionen (NASA, ESA, WHO, Statistisches Bundesamt, Guinness World Records, Bundeszentrale für politische Bildung) und bekannte Fachportale. Wenn du dir bei einer speziellen Quelle unsicher bist, verlinke den Wikipedia-Artikel zum Thema.
- **Eindeutige Zahl.** Jede Frage hat genau eine sinnvolle numerische Antwort. Keine Spannen, keine Meinungen.
- **Stabile Fakten bevorzugen.** Vermeide Werte, die sich jährlich ändern. Wenn nötig, nenne das Bezugsjahr in der Frage („Stand 2023“).
- **Ton:** schräg, überraschend, mit Augenzwinkern, Du-Ansprache. Die Erklärung soll einen Aha-Moment liefern, nicht nur die Zahl wiederholen.
- **Größenordnungen mischen.** Manche Antworten sind 3, manche 384.400. Mische kleine und riesige Zahlen, damit man nicht immer „irgendwas mit tausend“ tippen kann.
- **Keine doppelten Themen.** Nicht zwei Fragen zum selben Fakt.
- Keine Fragen zu Tod von Privatpersonen, Gewalt, Sexualität oder Themen, die für 12-Jährige unpassend sind.

### 5.4 Drei Beispielfragen im Zielstil

```ts
{
  id: "q001",
  category: "Tiere & Natur",
  question: "Wie viele Herzen schlagen in einem Oktopus?",
  answer: 3,
  unit: "Herzen",
  answerFormat: "integer",
  explanation:
    "Ein Oktopus hat drei Herzen: Zwei pumpen das Blut durch die Kiemen, das dritte versorgt den Rest des Körpers. Beim Schwimmen setzt das Hauptherz sogar aus, weshalb Oktopusse lieber kriechen. Und ja, ihr Blut ist blau.",
  sources: [{ title: "Wikipedia: Kraken", url: "https://de.wikipedia.org/wiki/Kraken" }],
  difficulty: 1,
},
{
  id: "q002",
  category: "Weltraum & Wissenschaft",
  question: "Wie viele Erdtage dauert eine einzige Umdrehung der Venus um ihre eigene Achse?",
  answer: 243,
  unit: "Erdtage",
  answerFormat: "integer",
  explanation:
    "Die Venus dreht sich so gemächlich, dass ein Venustag länger ist als ein Venusjahr (225 Erdtage). Und sie dreht sich auch noch rückwärts: Dort geht die Sonne im Westen auf.",
  sources: [{ title: "Wikipedia: Venus (Planet)", url: "https://de.wikipedia.org/wiki/Venus_(Planet)" }],
  difficulty: 2,
},
{
  id: "q003",
  category: "Körper & Gesundheit",
  question: "Wie alt wurde Jeanne Calment, der älteste Mensch mit belegtem Geburtsdatum?",
  answer: 122,
  unit: "Jahre",
  answerFormat: "integer",
  explanation:
    "Jeanne Calment aus Frankreich wurde 122 Jahre und 164 Tage alt. Sie hat als Kind angeblich Vincent van Gogh getroffen und erst mit 117 mit dem Rauchen aufgehört. Ihr Rezept: Olivenöl, Portwein und Schokolade.",
  sources: [{ title: "Wikipedia: Jeanne Calment", url: "https://de.wikipedia.org/wiki/Jeanne_Calment" }],
  difficulty: 2,
}
```

---

## 6. UI und Design

Die App soll nach einem Spiel aussehen, nicht nach einem Formular. Orientiere dich an der Optik einer bunten Spieleschachtel: warmes Gelb, kräftiges Orange-Rot, dicke schwarze Konturen, leicht schräg gestellter Titel, handgezeichnete Kritzeleien im Hintergrund.

### 6.1 Design-Tokens (in `globals.css` als CSS-Variablen)
- Hintergrund: warmes Gelb (`#FFC83D` oder ähnlich)
- Akzent: Orange-Rot (`#F2542D`)
- Sekundär: Dunkelorange (`#E38A00`)
- Text und Konturen: fast Schwarz (`#1B1B1B`)
- Karten: Cremeweiß (`#FFF8E7`)
- Erfolg: Grün, Fehler: Rot, jeweils kräftig und mit gutem Kontrast auf Creme.

### 6.2 Typografie
- Display-Schrift für Titel, Zahlen und Labels: eine fette, verspielte Schrift über `next/font/google`, z. B. „Bangers“, „Luckiest Guy“ oder „Titan One“.
- Fließtext: eine gut lesbare Sans wie „Nunito“ oder „Inter“.
- Titel leicht rotiert (−3°), Buttons mit dicker Kontur und hartem Schatten (Neo-Brutalismus-Stil, aber freundlich).

### 6.3 Layout
- Mobile-first. Referenz-Viewports: 375×667 (Handy), 768×1024 (Tablet), 1440×900 (Desktop).
- Inhalt maximal ca. 640 px breit, zentriert, großzügige Abstände auf dem 4-px-Raster.
- Auf dem Handy müssen Frage, Eingabefeld und Button ohne Scrollen sichtbar sein.
- Niemals horizontales Scrollen. Kein Layout-Springen beim Wechsel Frage → Auflösung.
- Dekorative SVG-Kritzeleien (Blitze, Sterne, Fragezeichen, Wellen) als eigene kleine Komponente, absolut positioniert, `aria-hidden`.

### 6.4 Interaktion und Barrierefreiheit
- Sichtbare Fokus-Zustände, komplette Bedienung per Tastatur.
- Ergebnis der Auflösung in einer `aria-live="polite"`-Region.
- Kontrast mindestens WCAG AA.
- `prefers-reduced-motion` respektieren: dann keine Count-up-Animation und kein Konfetti.
- Buttons mindestens 44 px hoch.

### 6.5 Was nicht erlaubt ist
- Tailwind-Standardoptik ohne eigene Farben oder Schrift.
- Graue Karten auf weißem Grund.
- Emoji als einzige Dekoration.

---

## 7. Projektstruktur

```
src/
  app/
    layout.tsx            Fonts, Metadaten, globaler Hintergrund
    page.tsx              Startbildschirm
    spielen/page.tsx      Spielbildschirm (Client-Komponente mit Zustandsautomat)
    ergebnis/page.tsx     Ergebnis
    globals.css
  components/
    StartScreen.tsx, QuestionCard.tsx, EstimateInput.tsx, RevealCard.tsx,
    ScoreBadge.tsx, ProgressBar.tsx, ResultSummary.tsx, SourceList.tsx,
    Confetti.tsx, Doodles.tsx, Button.tsx, CategoryBadge.tsx
  lib/
    scoring.ts            Punkteberechnung
    game-reducer.ts       Zustandsautomat: idle → question → reveal → finished
    format.ts             parseGermanNumber(), formatNumber(), formatWithUnit()
    storage.ts            localStorage/sessionStorage mit SSR-Schutz
    select-questions.ts   Zufallsauswahl mit Filter und Wiederholungsschutz
  data/
    schema.ts             Zod-Schema und Kategorie-Enum
    questions.ts          Die 100 Fragen
scripts/
  check-sources.ts        HTTP-HEAD/GET auf alle Quellen, Report als Tabelle
tests/
  unit/                   Vitest
  e2e/                    Playwright
DECISIONS.md              Kurze Begründung nicht offensichtlicher Entscheidungen
README.md                 Starten, Testen, Fragen ergänzen
```

Regeln für den Code:
- Kein `any`, kein `@ts-ignore`, kein `eslint-disable` ohne Begründung im Kommentar.
- Spiellogik (Reducer, Scoring, Format, Auswahl) ist reine Logik ohne React und ohne Browser-APIs, damit sie trivial testbar ist.
- Server-Komponenten greifen nie auf `window` oder `localStorage` zu.

---

## 8. Tests

### 8.1 Unit (Vitest)
- `scoring.test.ts`: jede Stufe der Tabelle, Grenzwerte exakt an den Schwellen, Symmetrie (50 vs. 100 und 200 vs. 100 geben gleich viele Punkte), alle Randfälle aus Abschnitt 4.
- `format.test.ts`: „1.250.000“ → 1250000, „12,5“ → 12.5, „12.5“ → 12.5, „1,250.5“ ist ungültig, leere Eingabe → `null`; Formatierung mit Tausenderpunkt und Komma.
- `game-reducer.test.ts`: kompletter Durchlauf einer 3-Fragen-Runde, „Keine Ahnung“, keine Aktion im falschen Zustand hat Wirkung, Gesamtpunkte werden korrekt summiert.
- `select-questions.test.ts`: keine Duplikate, Anzahl stimmt, Kategoriefilter greift, Pool wird nach vollständigem Durchspielen zurückgesetzt.
- `storage.test.ts`: funktioniert mit gemocktem `localStorage`, wirft nicht ohne `window`.

### 8.2 Datentests (`questions.test.ts`, laufen mit `pnpm test`)
- Genau 100 Fragen, IDs `q001` bis `q100` lückenlos und eindeutig.
- Jede Frage besteht das Zod-Schema.
- Jede Kategorie hat genau 10 Fragen.
- Keine zwei Fragen mit identischem Fragetext, keine identische Antwort mit identischer Einheit in derselben Kategorie.
- Jede Quelle: `https://`, gültige URL, Hostname enthält keinen Platzhalter wie `example`.
- `answerFormat: "integer"` → `answer` ist ganzzahlig.

### 8.3 Komponenten (Vitest + Testing Library)
- `EstimateInput`: akzeptiert Komma und Punkt, Button deaktiviert bei leerer Eingabe, Enter sendet ab, Vorschau zeigt formatierte Zahl mit Einheit.
- `RevealCard`: zeigt Punkte, Label, Erklärung und alle Quellen als Links mit `target="_blank"`.
- `ResultSummary`: zeigt Gesamtpunkte und eine Zeile pro Frage.

### 8.4 End-to-End (Playwright)
- Komplette Runde mit 5 Fragen: Start → jede Frage beantworten → Auflösung prüfen → Ergebnis zeigt Summe, die zu den Einzelpunkten passt.
- Pfad „Keine Ahnung“ gibt 0 Punkte und zeigt trotzdem Erklärung und Quellen.
- Tastatur: Enter sendet Schätzung ab und geht weiter.
- Reload mitten in der Runde stellt die Frage wieder her.
- Highscore überlebt Reload und wird auf dem Startbildschirm angezeigt.
- Direkter Aufruf von `/ergebnis` ohne Runde leitet auf `/` um.
- Mobile-Viewport 375×667: kein horizontales Scrollen (`document.documentElement.scrollWidth <= 375`), Eingabefeld und Button im sichtbaren Bereich.
- Screenshots jedes Bildschirms (Start, Frage, Auflösung, Ergebnis) in Mobile und Desktop nach `screenshots/`.

---

## 9. Arbeitsschleife bis zur Fertigstellung

Arbeite in Phasen. **Nach jeder Phase** führst du `pnpm check` aus. Schlägt etwas fehl, behebst du die Ursache und wiederholst, bis alles grün ist. Erst dann beginnt die nächste Phase. Committe nach jeder grünen Phase.

**Verboten in der Schleife:**
- Tests löschen, mit `.skip` deaktivieren oder Assertions abschwächen, damit sie grün werden.
- Fehler mit `try/catch` verstecken.
- Typfehler mit `any` oder `@ts-ignore` wegdrücken.
- Eine Phase als fertig erklären, ohne `pnpm check` tatsächlich ausgeführt und die Ausgabe gelesen zu haben.

### Phase 0: Gerüst
Projekt anlegen, Abhängigkeiten installieren, Tailwind, Vitest, Playwright, ESLint, Prettier konfigurieren, alle Scripts anlegen, Platzhalter-Startseite. Git initialisieren.
→ `pnpm check` grün, `pnpm test:e2e` läuft mit einem Smoke-Test durch.

### Phase 1: Spiellogik
Schema, Scoring, Format, Reducer, Auswahl, Storage inklusive aller Unit-Tests aus 8.1. Noch keine UI.
→ `pnpm check` grün.

### Phase 2: Fragen in fünf Blöcken à 20
Schreibe die 100 Fragen in fünf Durchgängen zu je 20 Fragen (je Durchgang zwei Fragen pro Kategorie). Nach jedem Block: Datentests laufen lassen, Verstöße beheben. Prüfe jede Frage vor dem Schreiben gegen die redaktionellen Regeln aus 5.3. Am Ende einmal `pnpm check:sources` ausführen und tote Links durch funktionierende ersetzen.
→ `pnpm check` grün, `check:sources` meldet 0 tote Links.

### Phase 3: UI
Alle Bildschirme und Komponenten nach Abschnitt 3 und 6, inklusive Komponententests aus 8.3.
→ `pnpm check` grün.

### Phase 4: End-to-End
Alle E2E-Tests aus 8.4 schreiben und grün bekommen. Screenshots erzeugen.
→ `pnpm check` und `pnpm test:e2e` grün.

### Phase 5: UI-Review-Schleife (mindestens 3 Durchgänge)
1. Screenshots aus `screenshots/` öffnen und ansehen.
2. Jeden Bildschirm gegen die Checkliste unten bewerten und die Mängel in `DECISIONS.md` unter „UI-Review Runde n“ auflisten.
3. Mängel beheben, `pnpm check` und `pnpm test:e2e` grün bekommen, Screenshots neu erzeugen.
4. Wiederholen, bis ein Durchgang keine Mängel mehr findet. Mindestens drei Durchgänge, danach so viele wie nötig.

**UI-Checkliste je Bildschirm:**
- Wirkt wie ein Spiel: eigene Schrift, eigene Farben, Kritzeleien, nicht wie ein Standard-Formular.
- Visuelle Hierarchie klar: Das Wichtigste (Frage bzw. Antwort) ist am größten.
- Handy 375 px: alles lesbar, nichts abgeschnitten, kein horizontales Scrollen, Frage + Eingabe + Button ohne Scrollen sichtbar.
- Desktop: Inhalt nicht auf 1440 px verschmiert, sondern zentriert und begrenzt.
- Abstände konsistent, Karten und Buttons einheitlich gestaltet.
- Fokus-Zustände sichtbar, Kontraste ausreichend.
- Auflösung fühlt sich belohnend an: Count-up, Label, Farbe, bei Volltreffer Konfetti.
- Keine Platzhaltertexte, kein „Lorem ipsum“, keine sichtbaren Fehlerzustände.

### Phase 6: Abschluss
- `README.md` mit Start, Tests, Anleitung „Neue Frage hinzufügen“.
- `DECISIONS.md` finalisieren.
- Letzter kompletter Lauf: `pnpm check` und `pnpm test:e2e`. Beide müssen grün sein.
- Abschlussbericht (siehe Abschnitt 10).

---

## 10. Definition of Done und Abschlussbericht

Fertig bist du erst, wenn **alle** Punkte erfüllt sind:

- [ ] `pnpm check` läuft ohne Fehler und ohne Warnungen durch.
- [ ] `pnpm test:e2e` ist grün, alle Tests aus 8.4 existieren.
- [ ] Genau 100 Fragen, alle Datentests grün, `check:sources` ohne tote Links.
- [ ] Alle Bildschirme aus Abschnitt 3 sind vorhanden und per Tastatur bedienbar.
- [ ] Mindestens drei dokumentierte UI-Review-Runden, letzte Runde ohne Mängel.
- [ ] Screenshots aller Bildschirme in Mobile und Desktop liegen in `screenshots/`.
- [ ] `README.md` und `DECISIONS.md` vorhanden.
- [ ] Alle Phasen sind committet.

Zum Schluss gib einen Abschlussbericht mit:
1. Was gebaut wurde, in fünf Sätzen.
2. Testergebnis: Anzahl Unit-, Komponenten-, Daten- und E2E-Tests, alle grün oder nicht.
3. Ehrliche Liste offener Punkte oder bekannter Schwächen, falls vorhanden.
4. Befehl zum Starten.

Berichte nur, was du tatsächlich ausgeführt und gesehen hast. Wenn etwas nicht geklappt hat, steht es im Bericht.
