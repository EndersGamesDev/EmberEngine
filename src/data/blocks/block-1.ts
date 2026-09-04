import type { Question } from "../schema";

export const block1: Question[] = [
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
    sources: [
      { title: "Wikipedia: Venus (Planet)", url: "https://de.wikipedia.org/wiki/Venus_(Planet)" },
    ],
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
    sources: [
      { title: "Wikipedia: Jeanne Calment", url: "https://de.wikipedia.org/wiki/Jeanne_Calment" },
    ],
    difficulty: 2,
  },
  {
    id: "q004",
    category: "Tiere & Natur",
    question: "Wie viele Halswirbel stecken im meterlangen Hals einer ausgewachsenen Giraffe?",
    answer: 7,
    unit: "Halswirbel",
    answerFormat: "integer",
    explanation:
      "Die Giraffe kommt mit genau sieben Halswirbeln aus, also mit exakt so vielen wie du, nur sind ihre bis zu einem Viertelmeter lang. Fast alle Säugetiere haben sieben Halswirbel, vom Blauwal bis zur Spitzmaus. Damit der Hals trotzdem beweglich bleibt, hilft bei der Giraffe der erste Brustwirbel wie ein achter Halswirbel mit.",
    sources: [{ title: "Wikipedia: Giraffe", url: "https://de.wikipedia.org/wiki/Giraffe" }],
    difficulty: 2,
  },
  {
    id: "q005",
    category: "Körper & Gesundheit",
    question:
      "Wie viele Knochen hat das Skelett eines erwachsenen Menschen nach gängiger Zählweise?",
    answer: 206,
    unit: "Knochen",
    answerFormat: "integer",
    explanation:
      "Als Baby startest du mit über 300 Knochen, von denen später viele miteinander verschmelzen. Beim Erwachsenen bleiben 206 übrig, je nach Zählweise der kleinen Fuß- und Wirbelknochen kommen Lehrbücher auch auf bis zu 212. Mehr als die Hälfte davon steckt allein in deinen Händen und Füßen.",
    sources: [
      {
        title: "Wikipedia: Skelett des Menschen",
        url: "https://de.wikipedia.org/wiki/Skelett_des_Menschen",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q006",
    category: "Geschichte",
    question: "Wie viele Jahre stand die Berliner Mauer, vom Bau 1961 bis zur Öffnung 1989?",
    answer: 28,
    unit: "Jahre",
    answerFormat: "integer",
    explanation:
      "Zwischen dem Baubeginn im August 1961 und der Öffnung im November 1989 lagen 28 Jahre, 2 Monate und 27 Tage. Hochgezogen wurde sie im Wesentlichen in einer einzigen Nacht, geöffnet dann nach einer verpatzten Pressekonferenz. Wer als Kind vor der frischen Mauer stand, war beim Mauerfall längst erwachsen.",
    sources: [
      { title: "Wikipedia: Berliner Mauer", url: "https://de.wikipedia.org/wiki/Berliner_Mauer" },
    ],
    difficulty: 2,
  },
  {
    id: "q007",
    category: "Geschichte",
    question:
      "Wie viele Jahre dauerte der Hundertjährige Krieg zwischen England und Frankreich tatsächlich?",
    answer: 116,
    unit: "Jahre",
    answerFormat: "integer",
    explanation:
      "Von 1337 bis 1453 zog sich der Konflikt über 116 Jahre, den griffigen Namen verpassten ihm erst spätere Historiker. Durchgekämpft wurde dabei keineswegs: Zwischen 1386 und 1415 herrschte fast drei Jahrzehnte lang weitgehend Ruhe. Beim Namen wurde also großzügig abgerundet.",
    sources: [
      {
        title: "Wikipedia: Hundertjähriger Krieg",
        url: "https://de.wikipedia.org/wiki/Hundertj%C3%A4hriger_Krieg",
      },
    ],
    difficulty: 3,
  },
  {
    id: "q008",
    category: "Geografie & Länder",
    question: "An wie viele Nachbarstaaten grenzt Deutschland zu Lande?",
    answer: 9,
    unit: "Nachbarstaaten",
    answerFormat: "integer",
    explanation:
      "Dänemark, Polen, Tschechien, Österreich, die Schweiz, Frankreich, Luxemburg, Belgien und die Niederlande legen sich rundherum um Deutschland. Damit gehört das Land zu den bestvernetzten Staaten Europas. Wer eine Rundreise entlang aller neun Grenzen plant, fährt einmal quer durch Mitteleuropa.",
    sources: [
      { title: "Wikipedia: Deutschland", url: "https://de.wikipedia.org/wiki/Deutschland" },
    ],
    difficulty: 1,
  },
  {
    id: "q009",
    category: "Geografie & Länder",
    question: "Wie hoch ist der Mount Everest nach der Neuvermessung von 2020, in ganzen Metern?",
    answer: 8849,
    unit: "m",
    answerFormat: "integer",
    explanation:
      "China und Nepal haben den Gipfel 2020 gemeinsam neu vermessen und sich auf 8848,86 Meter geeinigt, gerundet also 8849. Das bleibt ein Näherungswert, denn die Schneehaube auf dem Gipfel schwankt und das Gebirge wächst durch die Plattentektonik weiter. Davor galt jahrzehntelang der Wert 8848 Meter.",
    sources: [
      { title: "Wikipedia: Mount Everest", url: "https://de.wikipedia.org/wiki/Mount_Everest" },
    ],
    difficulty: 3,
  },
  {
    id: "q010",
    category: "Essen & Trinken",
    question: "Wie viele Kilokalorien liefert ein einziges Gramm reines Fett?",
    answer: 9,
    unit: "kcal",
    answerFormat: "integer",
    explanation:
      "Mit 9 Kilokalorien pro Gramm ist Fett der dichteste Energieträger auf deinem Teller, Kohlenhydrate und Eiweiß bringen es auf jeweils nur 4. Diese Werte sind in der EU für die Nährwerttabelle fest vorgeschrieben, damit alle gleich rechnen. Ein großzügiger Schuss Öl kippt deshalb die Bilanz jedes Salats.",
    sources: [
      {
        title: "Wikipedia: Physiologischer Brennwert",
        url: "https://de.wikipedia.org/wiki/Physiologischer_Brennwert",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q011",
    category: "Essen & Trinken",
    question: "Wie viele Monate muss ein Laib Parmigiano Reggiano mindestens reifen?",
    answer: 12,
    unit: "Monate",
    answerFormat: "integer",
    explanation:
      "Erst nach zwölf Monaten wird jeder Laib geprüft und darf bei bestandener Kontrolle das Siegel tragen. Wer länger wartet, bekommt mehr Titel: mit 24 Monaten heißt der Käse vecchio, mit 36 stravecchio und mit 72 extra stravecchione. Die letzte Stufe siehst du fast nie, weil kaum ein Erzeuger sechs Jahre lang stillhält.",
    sources: [
      {
        title: "Wikipedia: Parmigiano Reggiano",
        url: "https://de.wikipedia.org/wiki/Parmigiano_Reggiano",
      },
    ],
    difficulty: 2,
  },
  {
    id: "q012",
    category: "Technik & Internet",
    question: "Wie viele Zeichen passen in eine klassische SMS, bevor sie geteilt wird?",
    answer: 160,
    unit: "Zeichen",
    answerFormat: "integer",
    explanation:
      "Das Limit entstand, weil in ein Datenpaket der Mobilfunktechnik genau 1120 Bit passten und jedes Zeichen davon 7 Bit belegt. Sobald du Emojis oder exotische Sonderzeichen tippst, wird auf 16 Bit pro Zeichen umgestellt und es bleiben nur noch 70 Zeichen übrig. Sich kurz zu fassen war also nie eine Stilfrage, sondern reine Technik.",
    sources: [
      {
        title: "Wikipedia: Short Message Service",
        url: "https://de.wikipedia.org/wiki/Short_Message_Service",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q013",
    category: "Technik & Internet",
    question:
      "Aus wie vielen einzelnen Bildpunkten besteht ein Full-HD-Bild mit 1920 mal 1080 Pixeln?",
    answer: 2073600,
    unit: "Pixel",
    answerFormat: "integer",
    explanation:
      "1920 mal 1080 ergibt 2.073.600 Bildpunkte, also gut 2 Megapixel. Diese Menge färbt dein Bildschirm für jedes einzelne Bild komplett neu ein, und das viele Male pro Sekunde. Ein Bild in 4K besteht noch einmal aus der vierfachen Anzahl.",
    sources: [{ title: "Wikipedia: 1080p", url: "https://de.wikipedia.org/wiki/1080p" }],
    difficulty: 3,
  },
  {
    id: "q014",
    category: "Sport",
    question: "Wie viele Kilometer misst die offizielle Marathondistanz auf drei Nachkommastellen?",
    answer: 42.195,
    unit: "km",
    answerFormat: "decimal",
    explanation:
      "Die krumme Zahl geht auf die Olympischen Spiele 1908 in London zurück: Gestartet wurde an Schloss Windsor, ins Ziel ging es im Stadion vor der königlichen Loge. Aus dieser Zugabe wurden 26 Meilen und 385 Yards, seit 1921 gilt die Strecke als verbindlich. Ohne Königsfamilie liefen wir heute vermutlich glatte 40 Kilometer.",
    sources: [
      { title: "Wikipedia: Marathonlauf", url: "https://de.wikipedia.org/wiki/Marathonlauf" },
    ],
    difficulty: 1,
  },
  {
    id: "q015",
    category: "Sport",
    question:
      "Wie viele Spieler einer Mannschaft stehen beim Rugby Union gleichzeitig auf dem Feld?",
    answer: 15,
    unit: "Spieler",
    answerFormat: "integer",
    explanation:
      "Acht Stürmer erobern den Ball, sieben Spieler der Hintermannschaft machen daraus Punkte. Die Trikotnummer verrät deshalb immer die Position, von der Nummer eins vorn im Gedränge bis zur Nummer 15 hinten als Schlussmann. Beim olympischen Siebener-Rugby stehen dagegen nur sieben Leute pro Team auf dem Rasen.",
    sources: [
      { title: "Wikipedia: Rugby Union", url: "https://de.wikipedia.org/wiki/Rugby_Union" },
    ],
    difficulty: 3,
  },
  {
    id: "q016",
    category: "Weltraum & Wissenschaft",
    question: "Wie viele Kilometer ist der Mond im Mittel von der Erde entfernt?",
    answer: 384400,
    unit: "km",
    answerFormat: "integer",
    explanation:
      "Weil die Mondbahn eine Ellipse ist, schwankt der Abstand zwischen rund 363.300 und 405.500 Kilometern, im Mittel sind es 384.400. Ein Funkspruch braucht für diese Strecke etwa 1,3 Sekunden, weshalb Gespräche zum Mond immer leicht verzögert klangen. Die Apollo-Raumschiffe waren für dieselbe Strecke rund drei Tage unterwegs.",
    sources: [{ title: "Wikipedia: Mond", url: "https://de.wikipedia.org/wiki/Mond" }],
    difficulty: 3,
  },
  {
    id: "q017",
    category: "Popkultur & Musik",
    question: "Wie viele Oscars gewann James Camerons Film „Titanic“ bei der Verleihung 1998?",
    answer: 11,
    unit: "Oscars",
    answerFormat: "integer",
    explanation:
      "Aus 14 Nominierungen wurden elf Auszeichnungen, darunter bester Film und beste Regie. Damit zog Titanic mit Ben Hur aus dem Jahr 1959 gleich, der ebenfalls elf Oscars gewonnen hatte. Den Rekord bei den Nominierungen teilt sich der Film mit Alles über Eva.",
    sources: [
      { title: "Wikipedia: Titanic (1997)", url: "https://de.wikipedia.org/wiki/Titanic_(1997)" },
      {
        title: "Wikipedia: Titanic (1997 film)",
        url: "https://en.wikipedia.org/wiki/Titanic_(1997_film)",
      },
    ],
    difficulty: 2,
  },
  {
    id: "q018",
    category: "Popkultur & Musik",
    question: "Wie viele Sekunden dauert Queens „Bohemian Rhapsody“ in der Albumfassung von 1975?",
    answer: 355,
    unit: "Sekunden",
    answerFormat: "integer",
    explanation:
      "Fünf Minuten und 55 Sekunden, aufgeteilt in sechs völlig verschiedene Abschnitte von der Ballade über das Gitarrensolo und den Opernteil bis zum Hardrock. Für eine Single war das damals viel zu lang, das Radio spielte sie trotzdem rauf und runter. Ganz am Ende steht ein einzelner Gongschlag.",
    sources: [
      {
        title: "Wikipedia: Bohemian Rhapsody",
        url: "https://de.wikipedia.org/wiki/Bohemian_Rhapsody",
      },
    ],
    difficulty: 3,
  },
  {
    id: "q019",
    category: "Alltag & Kurioses",
    question: "Wie viele Karten hat ein deutsches Skatblatt, mit dem am Stammtisch gereizt wird?",
    answer: 32,
    unit: "Karten",
    answerFormat: "integer",
    explanation:
      "Vier Farben mit je acht Karten von der Sieben bis zum Ass ergeben zusammen 32 Blatt. Alle Zweien bis Sechsen fehlen, weil sie beim Skat schlicht keine Augen zählen. Im ganzen Spiel stecken genau 120 Augen, weshalb 61 zum Sieg reichen.",
    sources: [{ title: "Wikipedia: Skatblatt", url: "https://de.wikipedia.org/wiki/Skatblatt" }],
    difficulty: 1,
  },
  {
    id: "q020",
    category: "Alltag & Kurioses",
    question: "Wie viele Felder hat ein klassisches Monopoly-Brett auf einer kompletten Runde?",
    answer: 40,
    unit: "Felder",
    answerFormat: "integer",
    explanation:
      "Von den 40 Feldern sind 28 kaufbar, nämlich 22 Straßen, vier Bahnhöfe und zwei Werke. Dazu kommen Ereignis- und Gemeinschaftsfelder, zwei Steuerfelder und die vier Ecken mit Los, Gefängnis, Frei Parken und Gehe ins Gefängnis. Mit zwei Würfeln brauchst du im Schnitt knapp sechs Würfe für eine volle Runde.",
    sources: [
      { title: "Wikipedia: Monopoly", url: "https://de.wikipedia.org/wiki/Monopoly" },
      { title: "Wikipedia: Monopoly (game)", url: "https://en.wikipedia.org/wiki/Monopoly_(game)" },
    ],
    difficulty: 2,
  },
];
