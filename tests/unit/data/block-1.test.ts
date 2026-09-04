import { describe, expect, it } from "vitest";

import { block1 } from "@/data/blocks/block-1";
import { CATEGORIES, questionSchema } from "@/data/schema";

const EXPECTED_IDS = Array.from(
  { length: 20 },
  (_, index) => `q${String(index + 1).padStart(3, "0")}`,
);

/** Zählt Satzenden: ein Punkt, Ausrufe- oder Fragezeichen gefolgt von Leerraum oder Textende. */
function countSentences(text: string): number {
  return text.match(/[.!?](\s|$)/g)?.length ?? 0;
}

describe("block1", () => {
  it("enthält genau 20 Fragen", () => {
    expect(block1).toHaveLength(20);
  });

  it("hat die IDs q001 bis q020 lückenlos und eindeutig", () => {
    const ids = block1.map((question) => question.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect([...ids].sort()).toEqual(EXPECTED_IDS);
  });

  it("erfüllt für jede Frage das questionSchema", () => {
    for (const question of block1) {
      const result = questionSchema.safeParse(question);
      expect(
        result.success,
        `${question.id} verletzt das Schema: ${JSON.stringify(result.error?.issues, null, 2)}`,
      ).toBe(true);
    }
  });

  it("hat genau 2 Fragen pro Kategorie", () => {
    for (const category of CATEGORIES) {
      const count = block1.filter((question) => question.category === category).length;
      expect(count, `Kategorie ${category} hat ${count} statt 2 Fragen`).toBe(2);
    }
  });

  it("hat keine zwei identischen Fragetexte", () => {
    const texts = block1.map((question) => question.question);
    expect(new Set(texts).size).toBe(texts.length);
  });

  it("hat je Kategorie keine identische Kombination aus answer und unit", () => {
    for (const category of CATEGORIES) {
      const combos = block1
        .filter((question) => question.category === category)
        .map((question) => `${question.answer}|${question.unit}`);
      expect(new Set(combos).size, `Doppelte Antwort in Kategorie ${category}`).toBe(combos.length);
    }
  });

  it("nutzt answerFormat integer nur für ganzzahlige Antworten", () => {
    for (const question of block1) {
      if (question.answerFormat === "integer") {
        expect(
          Number.isInteger(question.answer),
          `${question.id} ist als integer deklariert, hat aber die Antwort ${question.answer}`,
        ).toBe(true);
      }
    }
  });

  it("verlinkt ausschließlich brauchbare https-Quellen", () => {
    for (const question of block1) {
      expect(question.sources.length, `${question.id} hat keine Quelle`).toBeGreaterThan(0);
      for (const source of question.sources) {
        expect(source.title.length, `${question.id} hat eine Quelle ohne Titel`).toBeGreaterThan(0);
        expect(source.url.startsWith("https://"), `${question.id}: ${source.url}`).toBe(true);
        const parsed = new URL(source.url);
        expect(parsed.hostname.includes("example"), `${question.id}: ${source.url}`).toBe(false);
      }
    }
  });

  it("erklärt jede Frage in 2 bis 4 Sätzen", () => {
    for (const question of block1) {
      const sentences = countSentences(question.explanation);
      expect(
        sentences,
        `${question.id} hat ${sentences} Sätze in der Erklärung: ${question.explanation}`,
      ).toBeGreaterThanOrEqual(2);
      expect(
        sentences,
        `${question.id} hat ${sentences} Sätze in der Erklärung: ${question.explanation}`,
      ).toBeLessThanOrEqual(4);
    }
  });
});
