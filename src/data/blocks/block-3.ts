import type { Question } from "../schema";

export const block3: Question[] = [
  {
    id: "q041",
    category: "Tiere & Natur",
    question: "Wie viele einzelne Muskeln stecken schätzungsweise im Rüssel eines Elefanten?",
    answer: 150000,
    unit: "Muskeln",
    answerFormat: "integer",
    explanation:
      "Der Rüssel kommt vollkommen ohne Knochen aus und wird allein von einem gewaltigen Muskelgeflecht bewegt. Die Zahl ist ein Näherungswert, doch damit gehört der Rüssel zu den beweglichsten Organen des ganzen Tierreichs. Er hebt Baumstämme und pflückt trotzdem einzelne Grashalme.",
    sources: [{ title: "Wikipedia: Rüssel", url: "https://de.wikipedia.org/wiki/R%C3%BCssel" }],
    difficulty: 3,
  },
  {
    id: "q042",
    category: "Tiere & Natur",
    question:
      "Wie hoch ragte Hyperion, der höchste bekannte Baum der Welt, bei seiner Vermessung 2017 in den Himmel?",
    answer: 115.85,
    unit: "m",
    answerFormat: "decimal",
    explanation:
      "Hyperion ist ein Küstenmammutbaum im Redwood-Nationalpark in Kalifornien und überragt die Freiheitsstatue damit deutlich. Sein genauer Standort wird geheim gehalten, weil Besucher den empfindlichen Wurzelbereich zertrampelt haben. Wer ihn trotzdem sucht, riskiert ein Bußgeld.",
    sources: [
      { title: "Wikipedia: Hyperion (Baum)", url: "https://de.wikipedia.org/wiki/Hyperion_(Baum)" },
    ],
    difficulty: 3,
  },
  {
    id: "q043",
    category: "Körper & Gesundheit",
    question: "Wie viele Chromosomen stecken in einer normalen Körperzelle des Menschen?",
    answer: 46,
    unit: "Chromosomen",
    answerFormat: "integer",
    explanation:
      "Die 46 Chromosomen bilden 23 Paare, je eines von der Mutter und eines vom Vater. Bis Mitte der 1950er-Jahre hielt die Wissenschaft hartnäckig 48 für die richtige Zahl, weil die Präparate zu unscharf waren. Ein Schimpanse hat tatsächlich 48.",
    sources: [{ title: "Wikipedia: Chromosom", url: "https://de.wikipedia.org/wiki/Chromosom" }],
    difficulty: 1,
  },
  {
    id: "q044",
    category: "Körper & Gesundheit",
    question:
      "Aus wie vielen Wirbeln besteht die menschliche Wirbelsäule, wenn man Kreuzbein und Steißbein mitzählt?",
    answer: 33,
    unit: "Wirbel",
    answerFormat: "integer",
    explanation:
      "Sieben Halswirbel, zwölf Brustwirbel und fünf Lendenwirbel bleiben beweglich, die restlichen neun verwachsen zu Kreuzbein und Steißbein. Manche Menschen bringen es auf 34, weil das Steißbein einen Wirbelrest mehr enthält. Frei beweglich sind also nur 24 Stück.",
    sources: [
      { title: "Wikipedia: Wirbelsäule", url: "https://de.wikipedia.org/wiki/Wirbels%C3%A4ule" },
    ],
    difficulty: 2,
  },
  {
    id: "q045",
    category: "Geschichte",
    question:
      "Wie viele Jahre lagen zwischen der Grundsteinlegung und der Vollendung des Kölner Doms?",
    answer: 632,
    unit: "Jahre",
    answerFormat: "integer",
    explanation:
      "Begonnen wurde 1248, fertig war der Dom erst 1880 – dazwischen ruhte die Baustelle über drei Jahrhunderte lang fast völlig. Der mittelalterliche Baukran blieb so lange auf dem Südturm stehen, dass er selbst zum Wahrzeichen der Stadt wurde. Seitdem wird ununterbrochen weiter restauriert.",
    sources: [
      { title: "Wikipedia: Kölner Dom", url: "https://de.wikipedia.org/wiki/K%C3%B6lner_Dom" },
    ],
    difficulty: 2,
  },
  {
    id: "q046",
    category: "Geschichte",
    question:
      "Wie viele Minuten dauerte der Britisch-Sansibarische Krieg von 1896, der kürzeste Krieg der Geschichte?",
    answer: 38,
    unit: "Minuten",
    answerFormat: "integer",
    explanation:
      "Um neun Uhr morgens eröffnete die britische Flotte das Feuer, um 9:38 Uhr war alles vorbei. Guinness World Records führt den Konflikt bis heute als kürzesten Krieg der Weltgeschichte. In dieser Zeit schaffst du nicht einmal eine Folge deiner Lieblingsserie.",
    sources: [
      {
        title: "Wikipedia: Britisch-Sansibarischer Krieg",
        url: "https://de.wikipedia.org/wiki/Britisch-Sansibarischer_Krieg",
      },
    ],
    difficulty: 3,
  },
  {
    id: "q047",
    category: "Geografie & Länder",
    question: "Wie tief ist der Baikalsee in Sibirien an seiner tiefsten Stelle?",
    answer: 1642,
    unit: "m",
    answerFormat: "integer",
    explanation:
      "Der Baikalsee ist so tief, dass in ihm rund ein Fünftel des flüssigen Süßwassers der Erde steckt. Der Eiffelturm würde fast fünfmal übereinander hineinpassen, ohne die Oberfläche zu durchstoßen. Im Winter trägt seine Eisdecke sogar ganze Lastwagen.",
    sources: [{ title: "Wikipedia: Baikalsee", url: "https://de.wikipedia.org/wiki/Baikalsee" }],
    difficulty: 2,
  },
  {
    id: "q048",
    category: "Geografie & Länder",
    question: "Aus wie vielen Kantonen besteht die Schweizerische Eidgenossenschaft heute?",
    answer: 26,
    unit: "Kantone",
    answerFormat: "integer",
    explanation:
      "Sechs davon galten früher als Halbkantone und haben bis heute nur eine halbe Standesstimme. Der jüngste Kanton ist der Jura, der sich 1979 von Bern löste. Gezählt werden trotzdem alle als vollwertige Kantone.",
    sources: [
      {
        title: "Wikipedia: Kanton (Schweiz)",
        url: "https://de.wikipedia.org/wiki/Kanton_(Schweiz)",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q049",
    category: "Essen & Trinken",
    question:
      "Wie viele Scoville-Einheiten erreicht die Chilisorte Pepper X, seit 2023 die schärfste der Welt?",
    answer: 2693000,
    unit: "Scoville",
    answerFormat: "integer",
    explanation:
      "Eine Jalapeño bringt es auf ein paar Tausend Scoville, Pepper X spielt damit in einer völlig eigenen Liga. Der Züchter Ed Currie berichtete nach seiner Kostprobe von stundenlangem Nachbrennen. Guinness World Records nahm die Sorte 2023 offiziell in die Rekordbücher auf.",
    sources: [{ title: "Wikipedia: Pepper X", url: "https://en.wikipedia.org/wiki/Pepper_X" }],
    difficulty: 3,
  },
  {
    id: "q050",
    category: "Essen & Trinken",
    question:
      "Wie viele Kilokalorien liefert ein Gramm reiner Alkohol nach der EU-Nährwertkennzeichnung?",
    answer: 7,
    unit: "kcal",
    answerFormat: "integer",
    explanation:
      "Damit liegt Alkohol genau zwischen Fett mit neun und Zucker mit vier Kilokalorien pro Gramm. Der Körper baut Alkohol bevorzugt ab und legt die Fettverbrennung so lange auf Eis. Deshalb heißt Bier auch spöttisch flüssiges Brot.",
    sources: [
      {
        title: "Wikipedia: Physiologischer Brennwert",
        url: "https://de.wikipedia.org/wiki/Physiologischer_Brennwert",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q051",
    category: "Technik & Internet",
    question:
      "Wie viele Byte Arbeitsspeicher hatte der Commodore 64, der meistverkaufte Heimcomputer aller Zeiten?",
    answer: 65536,
    unit: "Byte",
    answerFormat: "integer",
    explanation:
      "Die 64 Kilobyte im Namen entsprechen genau 65.536 Byte, wovon für BASIC-Programme nur 38.911 Byte übrig blieben. Ein einziges Foto aus deinem Handy ist heute tausendfach größer. Trotzdem passten ganze Spielwelten in diesen Speicher.",
    sources: [
      { title: "Wikipedia: Commodore 64", url: "https://de.wikipedia.org/wiki/Commodore_64" },
    ],
    difficulty: 2,
  },
  {
    id: "q052",
    category: "Technik & Internet",
    question:
      "Wie viel wog das Motorola DynaTAC 8000X von 1983, das erste kommerzielle Handy der Welt?",
    answer: 790,
    unit: "g",
    answerFormat: "integer",
    explanation:
      "Der Klotz kostete bei seiner Markteinführung fast 4000 Dollar und hielt nach zehn Stunden Ladezeit ganze 30 Minuten Gespräch durch. Seinen Spitznamen Knochen hatte er sich damit redlich verdient. Ein heutiges Smartphone wiegt kaum ein Viertel davon.",
    sources: [
      {
        title: "Wikipedia: Motorola DynaTAC",
        url: "https://en.wikipedia.org/wiki/Motorola_DynaTAC",
      },
    ],
    difficulty: 3,
  },
  {
    id: "q053",
    category: "Sport",
    question:
      "Wie breit ist ein Fußballtor im Erwachsenenbereich zwischen den Innenkanten der Pfosten?",
    answer: 7.32,
    unit: "m",
    answerFormat: "decimal",
    explanation:
      "Die krumme Zahl stammt aus dem englischen Maßsystem, denn acht Yard ergeben genau 7,32 Meter. Die Höhe von 2,44 Metern entspricht exakt acht Fuß. Seit über 150 Jahren wird an diesen Maßen nicht mehr gerüttelt.",
    sources: [
      { title: "Wikipedia: Fußballtor", url: "https://de.wikipedia.org/wiki/Fu%C3%9Fballtor" },
    ],
    difficulty: 1,
  },
  {
    id: "q054",
    category: "Sport",
    question: "Wie schwer ist die Kugel beim Kugelstoßen der Männer im internationalen Wettkampf?",
    answer: 7.26,
    unit: "kg",
    answerFormat: "decimal",
    explanation:
      "Umgerechnet sind das genau 16 englische Pfund, ein Erbe der britischen Leichtathletik. Bei den Frauen wiegt die Kugel dagegen glatte vier Kilogramm. Das Männergewicht wurde bereits um 1860 festgelegt und seitdem nie wieder verändert.",
    sources: [
      { title: "Wikipedia: Kugelstoßen", url: "https://de.wikipedia.org/wiki/Kugelsto%C3%9Fen" },
    ],
    difficulty: 2,
  },
  {
    id: "q055",
    category: "Weltraum & Wissenschaft",
    question:
      "Wie viele Kilometer legt Licht im Vakuum in einer Sekunde zurück, auf volle Kilometer gerundet?",
    answer: 299792,
    unit: "km/s",
    answerFormat: "integer",
    explanation:
      "Exakt sind es 299.792.458 Meter pro Sekunde, und dieser Wert ist nicht gemessen, sondern per Definition festgelegt. Seit 1983 definiert man den Meter über die Lichtgeschwindigkeit und nicht mehr umgekehrt. Licht umrundet die Erde damit gut siebenmal pro Sekunde.",
    sources: [
      {
        title: "Wikipedia: Lichtgeschwindigkeit",
        url: "https://de.wikipedia.org/wiki/Lichtgeschwindigkeit",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q056",
    category: "Weltraum & Wissenschaft",
    question:
      "Wie viel Prozent der Gesamtmasse unseres Sonnensystems entfallen allein auf die Sonne?",
    answer: 99.86,
    unit: "%",
    answerFormat: "decimal",
    explanation:
      "Alle Planeten, Monde, Asteroiden und Kometen zusammen machen also nur gut ein Promille aus. Vom verbleibenden Rest steckt der größte Teil im Jupiter. Die Sonne wiegt rund 330.000-mal so viel wie die Erde.",
    sources: [{ title: "Wikipedia: Sonne", url: "https://de.wikipedia.org/wiki/Sonne" }],
    difficulty: 3,
  },
  {
    id: "q057",
    category: "Popkultur & Musik",
    question: "Wie viele Tasten hat die Klaviatur eines modernen Klaviers insgesamt?",
    answer: 88,
    unit: "Tasten",
    answerFormat: "integer",
    explanation:
      "Es sind 52 weiße und 36 schwarze Tasten, zusammen also gut sieben Oktaven. Ältere Instrumente kamen oft mit 85 Tasten aus, weil die Klaviatur weiter oben endete. Wer alle 88 gleichzeitig drücken will, braucht sehr lange Unterarme.",
    sources: [{ title: "Wikipedia: Klavier", url: "https://de.wikipedia.org/wiki/Klavier" }],
    difficulty: 1,
  },
  {
    id: "q058",
    category: "Popkultur & Musik",
    question: "Wie viele Folgen umfasst die Sitcom „Friends“ über alle zehn Staffeln hinweg?",
    answer: 236,
    unit: "Folgen",
    answerFormat: "integer",
    explanation:
      "Die Serie lief von 1994 bis 2004, am Ende verdiente jeder der sechs Hauptdarsteller eine Million Dollar pro Folge. Aufgezeichnet wurde vor Publikum im Studio, das Lachen ist also größtenteils echt. Der Brunnen aus dem Vorspann steht nicht in New York, sondern in Kalifornien.",
    sources: [{ title: "Wikipedia: Friends", url: "https://de.wikipedia.org/wiki/Friends" }],
    difficulty: 2,
  },
  {
    id: "q059",
    category: "Alltag & Kurioses",
    question: "Aus wie vielen Karten besteht ein vollständiges klassisches UNO-Kartenspiel?",
    answer: 108,
    unit: "Karten",
    answerFormat: "integer",
    explanation:
      "Neben den Zahlenkarten in vier Farben stecken darin Aussetzen, Retour und Zieh Zwei sowie acht schwarze Wunschkarten. Die Null gibt es nur einmal pro Farbe, alle anderen Ziffern doppelt. Erfunden hat das Spiel ein Friseur aus Ohio.",
    sources: [
      {
        title: "Wikipedia: Uno (Kartenspiel)",
        url: "https://de.wikipedia.org/wiki/Uno_(Kartenspiel)",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q060",
    category: "Alltag & Kurioses",
    question:
      "Wie viele Spielsteine enthält die deutsche Ausgabe von Scrabble inklusive der Blankosteine?",
    answer: 102,
    unit: "Steine",
    answerFormat: "integer",
    explanation:
      "Genau 100 Steine tragen Buchstaben, dazu kommen zwei leere Joker für ein beliebiges Zeichen. Das E ist mit 15 Steinen am häufigsten vertreten, weil es im Deutschen der mit Abstand beliebteste Buchstabe ist. Q, X und Y gibt es dagegen nur je einmal.",
    sources: [{ title: "Wikipedia: Scrabble", url: "https://de.wikipedia.org/wiki/Scrabble" }],
    difficulty: 2,
  },
];
