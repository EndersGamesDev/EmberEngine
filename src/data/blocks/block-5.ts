import type { Question } from "../schema";

export const block5: Question[] = [
  {
    id: "q081",
    category: "Tiere & Natur",
    question:
      "Wie viele Bäume wachsen laut der großen Nature-Studie von 2015 insgesamt auf der Erde?",
    answer: 3040000000000,
    unit: "Bäume",
    answerFormat: "integer",
    explanation:
      "Ein Team um Thomas Crowther wertete 2015 mehr als 400.000 Probeflächen aus und kam auf rund 3,04 Billionen Bäume, etwa achtmal mehr als alle früheren Schätzungen. Rechnerisch stehen damit rund 375 Bäume für jeden einzelnen Menschen bereit. Fast die Hälfte davon wächst in den Tropen und Subtropen.",
    sources: [{ title: "Wikipedia: Tree", url: "https://en.wikipedia.org/wiki/Tree" }],
    difficulty: 3,
  },
  {
    id: "q082",
    category: "Tiere & Natur",
    question: "Wie viele Kilometer misst das Great Barrier Reef vor Australien in seiner Länge?",
    answer: 2300,
    unit: "km",
    answerFormat: "integer",
    explanation:
      "Das Riff zieht sich über gut 2.300 Kilometer an der Küste von Queensland entlang und bedeckt dabei etwa 348.700 Quadratkilometer, also ungefähr die Fläche Deutschlands. Gebaut haben es unzählige winzige Polypen, womit es das größte von Lebewesen errichtete Bauwerk der Erde ist. Die Längenangabe bleibt ein Näherungswert, denn eine Riffkette verläuft nun einmal nicht schnurgerade.",
    sources: [
      {
        title: "Wikipedia: Great Barrier Reef",
        url: "https://de.wikipedia.org/wiki/Great_Barrier_Reef",
      },
    ],
    difficulty: 2,
  },
  {
    id: "q083",
    category: "Körper & Gesundheit",
    question: "Wie viele äußere Muskeln braucht dein Augapfel, um in alle Richtungen zu blicken?",
    answer: 6,
    unit: "Muskeln",
    answerFormat: "integer",
    explanation:
      "Vier gerade und zwei schräge Muskeln ziehen an jedem Augapfel und drehen ihn flinker als jeden anderen Körperteil. Viele Säugetiere haben sogar einen siebten, den Rückziehmuskel, der dem Menschen fehlt. Damit beide Augen exakt dasselbe anpeilen, müssen also zwölf Muskeln perfekt zusammenspielen.",
    sources: [
      { title: "Wikipedia: Augenmuskeln", url: "https://de.wikipedia.org/wiki/Augenmuskeln" },
    ],
    difficulty: 2,
  },
  {
    id: "q084",
    category: "Körper & Gesundheit",
    question:
      "Wie viele Nervenzellen stecken nach heutigen Schätzungen in einem menschlichen Gehirn?",
    answer: 86000000000,
    unit: "Nervenzellen",
    answerFormat: "integer",
    explanation:
      "Lange hielt sich die schöne runde Zahl von 100 Milliarden, bis eine neue Zählmethode auf rund 86 Milliarden Neuronen kam. Etwa 69 Milliarden davon stecken im Kleinhirn, das nur einen kleinen Teil des Gehirnvolumens einnimmt. Dazu kommt noch einmal ungefähr dieselbe Menge an Zellen, die gar keine Nervenzellen sind, sondern für Versorgung und Ordnung sorgen.",
    sources: [
      { title: "Wikipedia: Human brain", url: "https://en.wikipedia.org/wiki/Human_brain" },
    ],
    difficulty: 3,
  },
  {
    id: "q085",
    category: "Geschichte",
    question: "Wie viele Ehefrauen hatte der englische König Heinrich VIII. im Lauf seines Lebens?",
    answer: 6,
    unit: "Ehefrauen",
    answerFormat: "integer",
    explanation:
      "Englische Schulkinder merken sich die Reihenfolge mit dem Reim geschieden, geköpft, gestorben, geschieden, geköpft, überlebt. Weil der Papst die erste Ehe nicht auflösen wollte, machte sich Heinrich kurzerhand selbst zum Oberhaupt der englischen Kirche. Nur die letzte Ehefrau, Catherine Parr, überlebte ihren königlichen Gatten.",
    sources: [
      {
        title: "Wikipedia: Heinrich VIII. (England)",
        url: "https://de.wikipedia.org/wiki/Heinrich_VIII._(England)",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q086",
    category: "Geschichte",
    question: "Wie viele Meter hoch war die Cheops-Pyramide, als sie noch ihre Verkleidung trug?",
    answer: 146.6,
    unit: "m",
    answerFormat: "decimal",
    explanation:
      "Berechnet werden 146,59 Meter, was genau 280 altägyptischen Ellen entspricht. Heute sind es nur noch 138,75 Meter, weil die glatte Kalksteinverkleidung später als bequemer Steinbruch für Kairo diente. Trotzdem blieb die Pyramide fast vier Jahrtausende lang das höchste Bauwerk der Welt.",
    sources: [
      { title: "Wikipedia: Cheops-Pyramide", url: "https://de.wikipedia.org/wiki/Cheops-Pyramide" },
    ],
    difficulty: 2,
  },
  {
    id: "q087",
    category: "Geografie & Länder",
    question: "Über wie viele Zeitzonen erstreckt sich Russland, das flächengrößte Land der Erde?",
    answer: 11,
    unit: "Zeitzonen",
    answerFormat: "integer",
    explanation:
      "Wenn auf der Halbinsel Kamtschatka der neue Tag längst läuft, sitzt Kaliningrad noch beim Nachmittagskaffee des Vortages, denn zwischen beiden Enden liegen zehn Stunden. Damit hat Russland mehr Zeitzonen als jedes andere Land mit zusammenhängendem Staatsgebiet. Frankreich kommt nur deshalb auf eine höhere Zahl, weil seine Überseegebiete über den halben Globus verteilt sind.",
    sources: [{ title: "Wikipedia: Zeitzone", url: "https://de.wikipedia.org/wiki/Zeitzone" }],
    difficulty: 2,
  },
  {
    id: "q088",
    category: "Geografie & Länder",
    question:
      "Wie viele Meter über Normalhöhennull liegt der Gipfel der Zugspitze, Deutschlands höchstem Berg?",
    answer: 2962,
    unit: "m",
    answerFormat: "integer",
    explanation:
      "Der Gipfel misst genau 2.962,06 Meter über Normalhöhennull und markiert zugleich die Grenze zu Österreich. Deutschlands höchster Punkt schafft es damit nicht einmal über die Dreitausendermarke. Der Mont Blanc als höchster Alpengipfel überragt die Zugspitze um mehr als 1.800 Meter.",
    sources: [{ title: "Wikipedia: Zugspitze", url: "https://de.wikipedia.org/wiki/Zugspitze" }],
    difficulty: 1,
  },
  {
    id: "q089",
    category: "Essen & Trinken",
    question: "Wie viel Gramm Salz pro Tag sollten Erwachsene laut WHO-Empfehlung höchstens essen?",
    answer: 5,
    unit: "g",
    answerFormat: "integer",
    explanation:
      "Fünf Gramm sind ungefähr ein gestrichener Teelöffel und entsprechen weniger als 2.000 Milligramm Natrium. In den meisten Ländern landet deutlich mehr auf dem Teller, und zwar meist ohne Zutun des Salzstreuers. Der Großteil steckt nämlich längst in Brot, Wurst, Käse und Fertiggerichten.",
    sources: [
      {
        title: "WHO: Salt reduction",
        url: "https://www.who.int/news-room/fact-sheets/detail/salt-reduction",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q090",
    category: "Essen & Trinken",
    question:
      "Wie viele Liter Wasser stecken im globalen Durchschnitt hinter einem Kilo Rindfleisch?",
    answer: 15400,
    unit: "Liter",
    answerFormat: "integer",
    explanation:
      "Der Löwenanteil dieses Wasserfußabdrucks fließt nicht ins Tier selbst, sondern in das Futter, das über Jahre erst einmal wachsen muss. Ein Kilogramm Hühnerfleisch kommt im selben Modell auf rund 4.300 Liter, ein Kilogramm Kuhmilch auf etwa 1.000 Liter. Die Zahl ist ein weltweiter Mittelwert und schwankt je nach Haltungsform und Region erheblich.",
    sources: [
      {
        title: "Water Footprint Network: Water footprint of crop and animal products",
        url: "https://www.waterfootprint.org/water-footprint-2/product-water-footprint/water-footprint-crop-and-animal-products/",
      },
    ],
    difficulty: 3,
  },
  {
    id: "q091",
    category: "Technik & Internet",
    question:
      "Wie viele Elektronenröhren glühten im ENIAC, einem der ersten elektronischen Rechner?",
    answer: 17468,
    unit: "Elektronenröhren",
    answerFormat: "integer",
    explanation:
      "Die Maschine wog 27 Tonnen, füllte einen ganzen Saal und zog rund 140 Kilowatt aus dem Netz. Weil vor allem das Ein- und Ausschalten den Röhren zusetzte, ließ man den Rechner lieber durchlaufen. Dein Smartphone rechnet heute millionenfach schneller und passt trotzdem in die Hosentasche.",
    sources: [{ title: "Wikipedia: ENIAC", url: "https://de.wikipedia.org/wiki/ENIAC" }],
    difficulty: 3,
  },
  {
    id: "q092",
    category: "Technik & Internet",
    question:
      "Aus wie vielen Pixeln bestand das Display des originalen Game Boy von 1989 insgesamt?",
    answer: 23040,
    unit: "Pixel",
    answerFormat: "integer",
    explanation:
      "160 mal 144 Bildpunkte ergeben zusammen 23.040 Pixel, dargestellt in vier Graugrüntönen auf gerade einmal 4,7 mal 4,3 Zentimetern. Für Tetris, Super Mario Land und die ersten Pokémon reichte das offenbar völlig aus. Zusammen mit dem Game Boy Color verkaufte sich das Gerät über 118 Millionen Mal.",
    sources: [{ title: "Wikipedia: Game Boy", url: "https://de.wikipedia.org/wiki/Game_Boy" }],
    difficulty: 3,
  },
  {
    id: "q093",
    category: "Sport",
    question:
      "Wie viele olympische Medaillen hat der Schwimmer Michael Phelps insgesamt gesammelt?",
    answer: 28,
    unit: "Medaillen",
    answerFormat: "integer",
    explanation:
      "23 davon sind aus Gold, dazu kommen drei Silber und zwei Bronze, erschwommen bei vier Olympischen Spielen. Vor Phelps hielt die Turnerin Larissa Latynina den Rekord mit 18 Medaillen, und der galt fast fünfzig Jahre lang. Es gibt Nationen, die in ihrer gesamten olympischen Geschichte weniger gewonnen haben.",
    sources: [
      { title: "Wikipedia: Michael Phelps", url: "https://de.wikipedia.org/wiki/Michael_Phelps" },
    ],
    difficulty: 1,
  },
  {
    id: "q094",
    category: "Sport",
    question: "Mit wie vielen Kugeln wird eine Partie Snooker auf dem Tisch insgesamt gespielt?",
    answer: 22,
    unit: "Kugeln",
    answerFormat: "integer",
    explanation:
      "Fünfzehn rote, sechs farbige und die weiße Spielkugel ergeben zusammen 22 Stück. Die farbigen Kugeln wandern nach jedem Treffer wieder zurück auf ihren Punkt, solange noch rote auf dem Tisch liegen. Genau deshalb liegt das höchstmögliche Break bei 147 Punkten und nicht einfach bei allem, was daliegt.",
    sources: [{ title: "Wikipedia: Snooker", url: "https://de.wikipedia.org/wiki/Snooker" }],
    difficulty: 2,
  },
  {
    id: "q095",
    category: "Weltraum & Wissenschaft",
    question: "Wie viele Erdjahre braucht der Neptun für eine einzige Runde um die Sonne?",
    answer: 165,
    unit: "Erdjahre",
    answerFormat: "integer",
    explanation:
      "Genau sind es 164 Jahre und 288 Tage, weshalb üblicherweise auf rund 165 Erdjahre gerundet wird. Seit seiner Entdeckung im Jahr 1846 hat Neptun deshalb erst eine einzige komplette Runde geschafft, die er 2011 beendete. Einen Geburtstag im Neptunjahr würdest du also kein einziges Mal feiern.",
    sources: [
      { title: "Wikipedia: Neptun (Planet)", url: "https://de.wikipedia.org/wiki/Neptun_(Planet)" },
    ],
    difficulty: 2,
  },
  {
    id: "q096",
    category: "Weltraum & Wissenschaft",
    question: "Wie viele chemische Elemente sind im Periodensystem bislang lückenlos beschrieben?",
    answer: 118,
    unit: "Elemente",
    answerFormat: "integer",
    explanation:
      "Von Wasserstoff bis Oganesson ist die Reihe seit 2015 vollständig gefüllt, ohne eine einzige Lücke. Die schwersten Vertreter entstehen ausschließlich in Teilchenbeschleunigern und zerfallen oft schon nach Sekundenbruchteilen wieder. Natürlich auf der Erde vorkommen tun deutlich weniger als hundert davon.",
    sources: [
      { title: "Wikipedia: Periodensystem", url: "https://de.wikipedia.org/wiki/Periodensystem" },
    ],
    difficulty: 1,
  },
  {
    id: "q097",
    category: "Popkultur & Musik",
    question: "Wie viele Saiten spannt eine moderne Konzertharfe mit Doppelpedalmechanik auf?",
    answer: 47,
    unit: "Saiten",
    answerFormat: "integer",
    explanation:
      "Sieben Saiten pro Oktave ergeben einen Umfang von sechseinhalb Oktaven, mehr als jedes Streichinstrument bietet. Damit trotzdem alle Halbtöne erreichbar sind, sitzen unten sieben Pedale, von denen jedes sämtliche Saiten eines Tonnamens gleichzeitig umstimmt. Zur Orientierung sind die C-Saiten meist rot eingefärbt und die F-Saiten dunkel.",
    sources: [{ title: "Wikipedia: Pedal harp", url: "https://en.wikipedia.org/wiki/Pedal_harp" }],
    difficulty: 3,
  },
  {
    id: "q098",
    category: "Popkultur & Musik",
    question:
      "Wie viele Schauspieler haben James Bond bis 2024 in den offiziellen Eon-Filmen gespielt?",
    answer: 6,
    unit: "Darsteller",
    answerFormat: "integer",
    explanation:
      "Sean Connery, George Lazenby, Roger Moore, Timothy Dalton, Pierce Brosnan und Daniel Craig teilen sich die Rolle seit 1962 untereinander auf. Am häufigsten schlüpfte Roger Moore in den Smoking, nämlich siebenmal, während George Lazenby es bei einem einzigen Auftritt beließ. Filme außerhalb der Eon-Reihe zählen in dieser Rechnung nicht mit.",
    sources: [{ title: "Wikipedia: James Bond", url: "https://de.wikipedia.org/wiki/James_Bond" }],
    difficulty: 1,
  },
  {
    id: "q099",
    category: "Alltag & Kurioses",
    question: "Wie viele Blatt Papier stecken heute nach Norm in einem Ries, dem Papierpaket?",
    answer: 500,
    unit: "Blatt",
    answerFormat: "integer",
    explanation:
      "Ein Ries A4-Papier mit 80 Gramm pro Quadratmeter wiegt damit netto genau zweieinhalb Kilogramm. Früher war das Maß ein einziges Durcheinander: In Deutschland galten für Schreibpapier 480 Bogen, in Portugal waren es sogar krumme 428. Das Wort stammt vom arabischen rizma, was schlicht Bündel bedeutet.",
    sources: [
      {
        title: "Wikipedia: Ries (Papiermaß)",
        url: "https://de.wikipedia.org/wiki/Ries_(Papierma%C3%9F)",
      },
    ],
    difficulty: 1,
  },
  {
    id: "q100",
    category: "Alltag & Kurioses",
    question: "Wie viel Gramm bringt eine einzelne 1-Euro-Münze auf die Waage?",
    answer: 7.5,
    unit: "g",
    answerFormat: "decimal",
    explanation:
      "Die Münze misst 23,25 Millimeter im Durchmesser und ist 2,33 Millimeter dick. Rund 133 Ein-Euro-Stücke ergeben zusammen ein Kilogramm, womit sich ein Sparschwein grob auf der Küchenwaage schätzen lässt. Die 2-Euro-Münze ist übrigens nur ein Gramm schwerer, wiegt also 8,5 Gramm.",
    sources: [
      { title: "Wikipedia: Euro-Münzen", url: "https://de.wikipedia.org/wiki/Euro-M%C3%BCnzen" },
    ],
    difficulty: 2,
  },
];
