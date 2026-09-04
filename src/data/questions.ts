import { type Question, questionsSchema } from "@/data/schema";

/**
 * Fragenbestand. In Phase 2 auf 100 Fragen erweitert.
 * `questionsSchema.parse` laeuft beim Import und damit auch beim Build:
 * fehlerhafte Daten brechen den Build ab, statt erst zur Laufzeit aufzufallen.
 */
export const questions: Question[] = questionsSchema.parse([
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
]);
