import { describe, expect, it } from "vitest";

import { block3 } from "@/data/blocks/block-3";
import { CATEGORIES, questionSchema } from "@/data/schema";

const EXPECTED_IDS = Array.from(
  { length: 20 },
  (_, index) => `q${String(41 + index).padStart(3, "0")}`,
);

/**
 * Zaehlt Satzenden: ".", "!" oder "?" gefolgt von Leerraum oder Textende.
 * In den Erklaerungen sind bewusst keine Abkuerzungen mit Punkt erlaubt,
 * damit dieser simple Zaehler nicht danebenliegt.
 */
function countSentences(text: string): number {
  return (text.match(/[.!?](?=\s|$)/g) ?? []).length;
}

describe("block3", () => {
  it("enthält genau 20 Fragen", () => {
    expect(block3).toHaveLength(20);
  });

  it("hat die IDs q041 bis q060 lückenlos, aufsteigend und eindeutig", () => {
    const ids = block3.map((question) => question.id);
    expect(ids).toEqual(EXPECTED_IDS);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("erfüllt für jede Frage das questionSchema", () => {
    for (const question of block3) {
      const result = questionSchema.safeParse(question);
      expect(
        result.success,
        `${question.id} verletzt das Schema: ${JSON.stringify(
          result.success ? [] : result.error.issues,
          null,
          2,
        )}`,
      ).toBe(true);
    }
  });

  it("enthält genau 2 Fragen pro Kategorie", () => {
    for (const category of CATEGORIES) {
      const count = block3.filter((question) => question.category === category).length;
      expect(count, `Kategorie ${category} hat ${count} statt 2 Fragen`).toBe(2);
    }
  });

  it("hat keine zwei identischen Fragetexte", () => {
    const texts = block3.map((question) => question.question);
    expect(new Set(texts).size).toBe(texts.length);
  });

  it("hat je Kategorie keine identische Kombination aus answer und unit", () => {
    for (const category of CATEGORIES) {
      const combos = block3
        .filter((question) => question.category === category)
        .map((question) => `${question.answer}|${question.unit}`);
      expect(
        new Set(combos).size,
        `Kategorie ${category} enthält doppelte Antwort-Einheit-Kombinationen: ${combos.join(", ")}`,
      ).toBe(combos.length);
    }
  });

  it("liefert bei answerFormat integer auch ganzzahlige Antworten", () => {
    for (const question of block3) {
      if (question.answerFormat === "integer") {
        expect(
          Number.isInteger(question.answer),
          `${question.id} ist als integer deklariert, hat aber ${question.answer}`,
        ).toBe(true);
      }
    }
  });

  it("verwendet ausschließlich echte https-Quellen", () => {
    for (const question of block3) {
      expect(question.sources.length).toBeGreaterThan(0);
      for (const source of question.sources) {
        expect(source.title.length).toBeGreaterThan(0);
        expect(source.url.startsWith("https://"), `${question.id}: ${source.url}`).toBe(true);
        expect(() => new URL(source.url), `${question.id}: ${source.url}`).not.toThrow();
        expect(
          new URL(source.url).hostname.includes("example"),
          `${question.id} nutzt eine Platzhalter-Domain: ${source.url}`,
        ).toBe(false);
      }
    }
  });

  it("erklärt jede Frage in 2 bis 4 Sätzen", () => {
    for (const question of block3) {
      const sentences = countSentences(question.explanation);
      expect(
        sentences,
        `${question.id} hat ${sentences} Sätze: ${question.explanation}`,
      ).toBeGreaterThanOrEqual(2);
      expect(
        sentences,
        `${question.id} hat ${sentences} Sätze: ${question.explanation}`,
      ).toBeLessThanOrEqual(4);
    }
  });
});
