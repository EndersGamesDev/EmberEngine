import { describe, expect, it } from "vitest";

import { block4 } from "@/data/blocks/block-4";
import { CATEGORIES, questionSchema } from "@/data/schema";

const EXPECTED_IDS = Array.from(
  { length: 20 },
  (_, index) => `q${String(61 + index).padStart(3, "0")}`,
);

function countSentences(text: string): number {
  return (text.match(/[.!?](\s|$)/g) ?? []).length;
}

describe("block4", () => {
  it("enthält genau 20 Fragen", () => {
    expect(block4).toHaveLength(20);
  });

  it("verwendet die IDs q061 bis q080 lückenlos und eindeutig", () => {
    const ids = block4.map((question) => question.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect([...ids].sort()).toEqual(EXPECTED_IDS);
  });

  it("erfüllt für jede Frage das questionSchema", () => {
    for (const question of block4) {
      const result = questionSchema.safeParse(question);
      expect(
        result.success,
        `${question.id}: ${JSON.stringify(result.error?.issues ?? [], null, 2)}`,
      ).toBe(true);
    }
  });

  it("hat genau 2 Fragen pro Kategorie", () => {
    for (const category of CATEGORIES) {
      const count = block4.filter((question) => question.category === category).length;
      expect(count, `Kategorie ${category} hat ${count} statt 2 Fragen`).toBe(2);
    }
  });

  it("enthält keine zwei identischen Fragetexte", () => {
    const texts = block4.map((question) => question.question);
    expect(new Set(texts).size).toBe(texts.length);
  });

  it("wiederholt innerhalb einer Kategorie keine Kombination aus Antwort und Einheit", () => {
    const seen = new Set<string>();
    for (const question of block4) {
      const key = `${question.category}|${question.answer}|${question.unit}`;
      expect(seen.has(key), `Doppelte Antwort-Einheit-Kombination bei ${question.id}: ${key}`).toBe(
        false,
      );
      seen.add(key);
    }
  });

  it("liefert bei answerFormat integer auch ganzzahlige Antworten", () => {
    for (const question of block4) {
      if (question.answerFormat === "integer") {
        expect(
          Number.isInteger(question.answer),
          `${question.id}: ${question.answer} ist nicht ganzzahlig`,
        ).toBe(true);
      }
      expect(question.answer, `${question.id}: Antwort muss größer als 0 sein`).toBeGreaterThan(0);
    }
  });

  it("hat ausschließlich brauchbare https-Quellen", () => {
    for (const question of block4) {
      expect(question.sources.length, `${question.id} hat keine Quelle`).toBeGreaterThan(0);
      for (const source of question.sources) {
        expect(source.title.length, `${question.id}: Quellentitel fehlt`).toBeGreaterThan(0);
        expect(
          source.url.startsWith("https://"),
          `${question.id}: ${source.url} ist kein https-Link`,
        ).toBe(true);
        const parsed = new URL(source.url);
        expect(
          parsed.hostname.includes("example"),
          `${question.id}: ${source.url} ist eine Platzhalter-Domain`,
        ).toBe(false);
      }
    }
  });

  it("erklärt jede Frage in 2 bis 4 Sätzen", () => {
    for (const question of block4) {
      const sentences = countSentences(question.explanation);
      expect(
        sentences,
        `${question.id}: ${sentences} Sätze in der Erklärung`,
      ).toBeGreaterThanOrEqual(2);
      expect(sentences, `${question.id}: ${sentences} Sätze in der Erklärung`).toBeLessThanOrEqual(
        4,
      );
    }
  });
});
