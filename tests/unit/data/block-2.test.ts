import { describe, expect, it } from "vitest";

import { block2 } from "@/data/blocks/block-2";
import { CATEGORIES, questionSchema } from "@/data/schema";

const EXPECTED_IDS = Array.from({ length: 20 }, (_, i) => `q${String(i + 21).padStart(3, "0")}`);

/** Zaehlt Satzenden: `.`, `!` oder `?`, gefolgt von Leerzeichen oder Textende. */
function countSentences(text: string): number {
  return (text.match(/[.!?](\s|$)/g) ?? []).length;
}

describe("block2", () => {
  it("enthaelt genau 20 Fragen", () => {
    expect(block2).toHaveLength(20);
  });

  it("hat die IDs q021 bis q040, lueckenlos und eindeutig", () => {
    const ids = block2.map((q) => q.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect([...ids].sort()).toEqual(EXPECTED_IDS);
  });

  it("erfuellt fuer jede Frage das questionSchema", () => {
    for (const question of block2) {
      const result = questionSchema.safeParse(question);
      expect(
        result.success,
        `${question.id} verletzt das Schema: ${JSON.stringify(result.error?.issues, null, 2)}`,
      ).toBe(true);
    }
  });

  it("hat genau 2 Fragen pro Kategorie", () => {
    for (const category of CATEGORIES) {
      const count = block2.filter((q) => q.category === category).length;
      expect(count, `Kategorie ${category} hat ${count} Fragen`).toBe(2);
    }
  });

  it("enthaelt keine zwei identischen Fragetexte", () => {
    const texts = block2.map((q) => q.question);
    expect(new Set(texts).size).toBe(texts.length);
  });

  it("enthaelt pro Kategorie keine identische Kombination aus answer und unit", () => {
    for (const category of CATEGORIES) {
      const combos = block2
        .filter((q) => q.category === category)
        .map((q) => `${q.answer}|${q.unit}`);
      expect(new Set(combos).size, `Doppelte Antwort in ${category}: ${combos.join(", ")}`).toBe(
        combos.length,
      );
    }
  });

  it("hat bei answerFormat integer nur ganzzahlige Antworten", () => {
    for (const question of block2.filter((q) => q.answerFormat === "integer")) {
      expect(
        Number.isInteger(question.answer),
        `${question.id} ist als integer deklariert, hat aber ${question.answer}`,
      ).toBe(true);
    }
  });

  it("verlinkt nur gueltige https-Quellen", () => {
    for (const question of block2) {
      expect(question.sources.length).toBeGreaterThan(0);
      for (const source of question.sources) {
        expect(source.title.length).toBeGreaterThan(0);
        expect(source.url.startsWith("https://"), `${question.id}: ${source.url}`).toBe(true);
        const url = new URL(source.url);
        expect(url.hostname.includes("example"), `${question.id}: ${source.url}`).toBe(false);
      }
    }
  });

  it("erklaert jede Frage in 2 bis 4 Saetzen", () => {
    for (const question of block2) {
      const sentences = countSentences(question.explanation);
      expect(
        sentences,
        `${question.id} hat ${sentences} Saetze: ${question.explanation}`,
      ).toBeGreaterThanOrEqual(2);
      expect(sentences).toBeLessThanOrEqual(4);
    }
  });
});
