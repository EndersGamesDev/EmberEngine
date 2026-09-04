import { describe, expect, it } from "vitest";

import { questions } from "@/data/questions";
import { CATEGORIES, questionSchema } from "@/data/schema";

const EXPECTED_IDS = Array.from(
  { length: 100 },
  (_, index) => `q${String(index + 1).padStart(3, "0")}`,
);

/** Zählt Satzenden: ein Punkt, Ausrufe- oder Fragezeichen gefolgt von Leerraum oder Textende. */
function countSentences(text: string): number {
  return text.match(/[.!?](\s|$)/g)?.length ?? 0;
}

/**
 * Typische deutsche Wortstämme in ASCII-Ersatzschreibung. Bewusst eine feste Liste
 * statt einer allgemeinen ae/oe/ue-Regex: Sonst schlagen echte Fremdwörter wie
 * „Queen“, „Duell“, „Poesie“ oder „Aerobic“ fälschlich an.
 */
const ASCII_UMLAUT_STEMS = [
  "aehnlich",
  "aelt",
  "aender",
  "baeum",
  "beruehmt",
  "buech",
  "duerf",
  "erklaer",
  "faellt",
  "fluess",
  "fuehr",
  "fuenf",
  "fuer",
  "fuess",
  "gebaeude",
  "gefuehl",
  "gelaende",
  "gemaess",
  "groess",
  "gruen",
  "haelt",
  "haend",
  "haeuf",
  "hoech",
  "hoeh",
  "hoer",
  "jaehr",
  "koenig",
  "koenn",
  "koerper",
  "kuerz",
  "laeng",
  "laesst",
  "laeuft",
  "loesung",
  "moeg",
  "muenz",
  "muess",
  "naeher",
  "naemlich",
  "natuerl",
  "oel",
  "roem",
  "schluess",
  "schoen",
  "spaet",
  "staerk",
  "stueck",
  "suess",
  "taegl",
  "traegt",
  "tuer",
  "ueber",
  "ungefaehr",
  "verhaeltnis",
  "voegel",
  "waehr",
  "waeld",
  "waer",
  "wuerd",
  "zaehl",
  "zurueck",
] as const;

const ASCII_UMLAUT_PATTERN = new RegExp(`\\b(?:${ASCII_UMLAUT_STEMS.join("|")})`, "i");

describe("questions", () => {
  it("enthält genau 100 Fragen", () => {
    expect(questions).toHaveLength(100);
  });

  it("hat die IDs q001 bis q100 lückenlos und eindeutig", () => {
    const ids = questions.map((question) => question.id);
    expect(new Set(ids).size, "es gibt doppelte IDs").toBe(ids.length);
    expect([...ids].sort()).toEqual(EXPECTED_IDS);
  });

  it("erfüllt für jede Frage das questionSchema", () => {
    for (const question of questions) {
      const result = questionSchema.safeParse(question);
      expect(
        result.success,
        `${question.id} verletzt das Schema: ${JSON.stringify(result.error?.issues, null, 2)}`,
      ).toBe(true);
    }
  });

  it("hat genau 10 Fragen pro Kategorie", () => {
    for (const category of CATEGORIES) {
      const count = questions.filter((question) => question.category === category).length;
      expect(count, `Kategorie ${category} hat ${count} statt 10 Fragen`).toBe(10);
    }
  });

  it("hat keine zwei identischen Fragetexte", () => {
    const seen = new Map<string, string>();
    for (const question of questions) {
      const previous = seen.get(question.question);
      expect(
        previous,
        `${question.id} wiederholt den Fragetext von ${previous ?? ""}: ${question.question}`,
      ).toBeUndefined();
      seen.set(question.question, question.id);
    }
  });

  it("hat innerhalb einer Kategorie keine identische Kombination aus Antwort und Einheit", () => {
    const seen = new Map<string, string>();
    for (const question of questions) {
      const key = `${question.category}|${question.answer}|${question.unit}`;
      const previous = seen.get(key);
      expect(
        previous,
        `${question.id} hat dieselbe Antwort wie ${previous ?? ""} in ${question.category}: ${question.answer} ${question.unit}`,
      ).toBeUndefined();
      seen.set(key, question.id);
    }
  });

  it("hat für jede Quelle eine gültige https-URL ohne Platzhalter im Hostnamen", () => {
    for (const question of questions) {
      for (const source of question.sources) {
        expect(
          source.url.startsWith("https://"),
          `${question.id}: ${source.url} beginnt nicht mit https://`,
        ).toBe(true);

        let parsed: URL | null = null;
        try {
          parsed = new URL(source.url);
        } catch {
          parsed = null;
        }
        expect(parsed, `${question.id}: ${source.url} ist keine gültige URL`).not.toBeNull();
        expect(
          parsed?.hostname.includes("example"),
          `${question.id}: ${source.url} zeigt auf einen Platzhalter-Hostnamen`,
        ).toBe(false);
      }
    }
  });

  it("hat bei answerFormat „integer“ eine ganzzahlige Antwort", () => {
    for (const question of questions.filter((entry) => entry.answerFormat === "integer")) {
      expect(
        Number.isInteger(question.answer),
        `${question.id} ist als integer deklariert, hat aber die Antwort ${question.answer}`,
      ).toBe(true);
    }
  });

  it("hat bei answerFormat „decimal“ eine nicht ganzzahlige Antwort", () => {
    for (const question of questions.filter((entry) => entry.answerFormat === "decimal")) {
      expect(
        Number.isInteger(question.answer),
        `${question.id} ist als decimal deklariert, hat aber die ganzzahlige Antwort ${question.answer}`,
      ).toBe(false);
    }
  });

  it("erklärt jede Frage in 2 bis 4 Sätzen", () => {
    for (const question of questions) {
      const sentences = countSentences(question.explanation);
      expect(
        sentences,
        `${question.id} hat ${sentences} Sätze in der Erklärung statt 2 bis 4`,
      ).toBeGreaterThanOrEqual(2);
      expect(
        sentences,
        `${question.id} hat ${sentences} Sätze in der Erklärung statt 2 bis 4`,
      ).toBeLessThanOrEqual(4);
    }
  });

  it("schreibt Umlaute aus, statt sie durch ae, oe oder ue zu ersetzen", () => {
    for (const question of questions) {
      for (const [feld, text] of [
        ["question", question.question],
        ["explanation", question.explanation],
      ] as const) {
        const treffer = ASCII_UMLAUT_PATTERN.exec(text);
        expect(
          treffer?.[0],
          `${question.id} (${feld}) benutzt die Ersatzschreibung „${treffer?.[0] ?? ""}“ statt eines Umlauts`,
        ).toBeUndefined();
      }
    }
  });

  it("verwendet jeden Schwierigkeitsgrad mindestens 20-mal", () => {
    for (const difficulty of [1, 2, 3] as const) {
      const count = questions.filter((question) => question.difficulty === difficulty).length;
      expect(
        count,
        `Schwierigkeitsgrad ${difficulty} kommt nur ${count}-mal vor`,
      ).toBeGreaterThanOrEqual(20);
    }
  });

  it("mischt die Größenordnungen der Antworten", () => {
    const klein = questions.filter((question) => question.answer < 10);
    const riesig = questions.filter((question) => question.answer > 100_000);
    expect(
      klein.length,
      `nur ${klein.length} Antworten sind kleiner als 10`,
    ).toBeGreaterThanOrEqual(15);
    expect(
      riesig.length,
      `nur ${riesig.length} Antworten sind größer als 100.000`,
    ).toBeGreaterThanOrEqual(8);
  });
});
