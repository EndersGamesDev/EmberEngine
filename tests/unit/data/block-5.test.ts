import { describe, expect, it } from "vitest";

import { block5 } from "@/data/blocks/block-5";
import { CATEGORIES, questionSchema } from "@/data/schema";

const EXPECTED_IDS = Array.from(
  { length: 20 },
  (_, index) => `q${String(81 + index).padStart(3, "0")}`,
);

/**
 * Zaehlt Satzenden: ein `.`, `!` oder `?`, dem ein Leerzeichen folgt oder das Textende.
 * In den Erklaerungen werden bewusst keine Abkuerzungen mit Punkt verwendet, damit die
 * Zaehlung nicht durch "z. B." oder "ca." verfaelscht wird.
 */
function countSentences(text: string): number {
  return (text.match(/[.!?](?=\s|$)/g) ?? []).length;
}

describe("block5", () => {
  it("enthaelt genau 20 Fragen", () => {
    expect(block5).toHaveLength(20);
  });

  it("nutzt die IDs q081 bis q100 lueckenlos und eindeutig", () => {
    const ids = block5.map((question) => question.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect([...ids].sort()).toEqual(EXPECTED_IDS);
  });

  it("erfuellt fuer jede Frage das questionSchema", () => {
    for (const question of block5) {
      const result = questionSchema.safeParse(question);
      expect(
        result.success,
        `${question.id} verletzt das Schema: ${JSON.stringify(result.error?.issues, null, 2)}`,
      ).toBe(true);
    }
  });

  it("verteilt genau 2 Fragen auf jede der 10 Kategorien", () => {
    for (const category of CATEGORIES) {
      const count = block5.filter((question) => question.category === category).length;
      expect(count, `Kategorie ${category} hat ${count} statt 2 Fragen`).toBe(2);
    }
  });

  it("enthaelt keine zwei identischen Fragetexte", () => {
    const texts = block5.map((question) => question.question);
    expect(new Set(texts).size).toBe(texts.length);
  });

  it("wiederholt innerhalb einer Kategorie keine Kombination aus answer und unit", () => {
    const seen = new Set<string>();
    for (const question of block5) {
      const key = `${question.category}|${question.answer}|${question.unit}`;
      expect(seen.has(key), `Doppelte Antwort-Einheit-Kombination bei ${question.id}: ${key}`).toBe(
        false,
      );
      seen.add(key);
    }
  });

  it("liefert bei answerFormat integer auch ganze Zahlen", () => {
    for (const question of block5) {
      if (question.answerFormat === "integer") {
        expect(
          Number.isInteger(question.answer),
          `${question.id} ist als integer deklariert, hat aber ${question.answer}`,
        ).toBe(true);
      }
    }
  });

  it("verweist ausschliesslich auf plausible https-Quellen", () => {
    for (const question of block5) {
      expect(question.sources.length).toBeGreaterThan(0);
      for (const source of question.sources) {
        expect(source.title.length).toBeGreaterThan(0);
        expect(source.url.startsWith("https://"), `${question.id}: ${source.url}`).toBe(true);
        const url = new URL(source.url);
        expect(url.hostname.includes("example"), `${question.id}: ${source.url}`).toBe(false);
      }
    }
  });

  it("formuliert jede Erklaerung in 2 bis 4 Saetzen", () => {
    for (const question of block5) {
      const sentences = countSentences(question.explanation);
      expect(
        sentences,
        `${question.id} hat ${sentences} Saetze: ${question.explanation}`,
      ).toBeGreaterThanOrEqual(2);
      expect(
        sentences,
        `${question.id} hat ${sentences} Saetze: ${question.explanation}`,
      ).toBeLessThanOrEqual(4);
    }
  });
});
