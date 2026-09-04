import { describe, expect, it } from "vitest";

import type { Category } from "@/data/schema";
import { selectQuestions } from "@/lib/select-questions";

import { idsOf, makeQuestions } from "../fixtures/questions";

const CATEGORIES: Category[] = ["Tiere & Natur", "Geschichte", "Sport"];

/** Zwölf Fragen, reihum auf drei Kategorien verteilt (je vier Stück). */
const pool = makeQuestions(12, CATEGORIES);

/** Deterministische Zufallsquelle (linearer Kongruenzgenerator). */
function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 4294967296;
  };
}

function categoryOf(id: string): Category {
  const question = pool.find((entry) => entry.id === id);
  if (question === undefined) throw new Error(`Unbekannte Frage: ${id}`);
  return question.category;
}

describe("selectQuestions", () => {
  it("zieht die gewünschte Anzahl ohne Duplikate", () => {
    const result = selectQuestions({ pool, count: 5, playedIds: [], random: seededRandom(1) });
    expect(result.questions).toHaveLength(5);
    expect(new Set(idsOf(result.questions)).size).toBe(5);
    expect(result.poolReset).toBe(false);
  });

  it("kappt die Anzahl auf die Größe des gefilterten Pools", () => {
    const result = selectQuestions({ pool, count: 99, playedIds: [], random: seededRandom(2) });
    expect(result.questions).toHaveLength(pool.length);

    const filtered = selectQuestions({
      pool,
      count: 99,
      categories: ["Sport"],
      playedIds: [],
      random: seededRandom(2),
    });
    expect(filtered.questions).toHaveLength(4);
  });

  it("gibt bei count = 0 eine leere Runde zurück", () => {
    const result = selectQuestions({ pool, count: 0, playedIds: [], random: seededRandom(3) });
    expect(result.questions).toEqual([]);
    expect(result.playedIds).toEqual([]);
    expect(result.poolReset).toBe(false);
  });

  describe("Kategoriefilter", () => {
    it("liefert nur Fragen der gewählten Kategorien", () => {
      const result = selectQuestions({
        pool,
        count: 4,
        categories: ["Geschichte"],
        playedIds: [],
        random: seededRandom(4),
      });
      expect(result.questions).toHaveLength(4);
      for (const question of result.questions) {
        expect(question.category).toBe("Geschichte");
      }
    });

    it("behandelt ein leeres Kategorie-Array wie „alle Kategorien“", () => {
      const withEmpty = selectQuestions({
        pool,
        count: 12,
        categories: [],
        playedIds: [],
        random: seededRandom(5),
      });
      const withUndefined = selectQuestions({
        pool,
        count: 12,
        playedIds: [],
        random: seededRandom(5),
      });
      expect(idsOf(withEmpty.questions)).toEqual(idsOf(withUndefined.questions));
      expect(withEmpty.questions).toHaveLength(12);
    });
  });

  describe("Wiederholungsschutz", () => {
    it("zieht zuerst die noch ungespielten Fragen", () => {
      const played = idsOf(pool).slice(0, 9);
      const result = selectQuestions({
        pool,
        count: 3,
        playedIds: played,
        random: seededRandom(6),
      });
      expect(result.poolReset).toBe(false);
      expect(idsOf(result.questions).sort()).toEqual(idsOf(pool).slice(9).sort());
      expect(result.playedIds).toHaveLength(12);
    });

    it("merkt sich die gezogenen IDs ohne Duplikate", () => {
      const first = selectQuestions({ pool, count: 4, playedIds: [], random: seededRandom(7) });
      const second = selectQuestions({
        pool,
        count: 4,
        playedIds: first.playedIds,
        random: seededRandom(8),
      });
      expect(second.playedIds).toHaveLength(8);
      expect(new Set(second.playedIds).size).toBe(8);
      for (const id of idsOf(first.questions)) {
        expect(idsOf(second.questions)).not.toContain(id);
      }
    });
  });

  describe("Pool-Reset", () => {
    it("setzt zurück, wenn alle Fragen des Filters gespielt sind", () => {
      const result = selectQuestions({
        pool,
        count: 5,
        playedIds: idsOf(pool),
        random: seededRandom(9),
      });
      expect(result.poolReset).toBe(true);
      expect(result.questions).toHaveLength(5);
      expect(new Set(idsOf(result.questions)).size).toBe(5);
      // Nach dem Reset stehen nur noch die IDs der frischen Runde in der Liste.
      expect(result.playedIds.sort()).toEqual(idsOf(result.questions).sort());
    });

    it("nimmt die letzten ungespielten Fragen garantiert mit", () => {
      const remaining = idsOf(pool).slice(10);
      const played = idsOf(pool).slice(0, 10);
      const result = selectQuestions({
        pool,
        count: 5,
        playedIds: played,
        random: seededRandom(10),
      });
      expect(result.poolReset).toBe(true);
      expect(result.questions).toHaveLength(5);
      expect(new Set(idsOf(result.questions)).size).toBe(5);
      for (const id of remaining) {
        expect(idsOf(result.questions)).toContain(id);
      }
    });

    it("lässt gespielte IDs anderer Kategorien unangetastet", () => {
      const historyIds = idsOf(pool.filter((question) => question.category === "Geschichte"));
      const foreignIds = idsOf(pool.filter((question) => question.category !== "Geschichte"));

      const result = selectQuestions({
        pool,
        count: 4,
        categories: ["Geschichte"],
        playedIds: [...foreignIds, ...historyIds],
        random: seededRandom(11),
      });

      expect(result.poolReset).toBe(true);
      for (const id of foreignIds) {
        expect(result.playedIds).toContain(id);
      }
      expect(result.playedIds).toHaveLength(foreignIds.length + 4);
      for (const question of result.questions) {
        expect(question.category).toBe("Geschichte");
      }
      for (const id of result.playedIds.slice(foreignIds.length)) {
        expect(categoryOf(id)).toBe("Geschichte");
      }
    });
  });

  describe("Determinismus", () => {
    it("liefert bei gleicher Zufallsquelle exakt dieselbe Reihenfolge", () => {
      const first = selectQuestions({ pool, count: 6, playedIds: [], random: seededRandom(42) });
      const second = selectQuestions({ pool, count: 6, playedIds: [], random: seededRandom(42) });
      expect(idsOf(first.questions)).toEqual(idsOf(second.questions));
    });

    it("mischt die Reihenfolge, statt den Pool von vorne abzuarbeiten", () => {
      const result = selectQuestions({ pool, count: 12, playedIds: [], random: seededRandom(43) });
      expect(idsOf(result.questions)).not.toEqual(idsOf(pool));
      expect(idsOf(result.questions).sort()).toEqual(idsOf(pool).sort());
    });

    it("lässt Pool und playedIds der Eingabe unverändert", () => {
      const playedIds = idsOf(pool).slice(0, 3);
      const poolSnapshot = idsOf(pool);
      const playedSnapshot = [...playedIds];
      selectQuestions({ pool, count: 4, playedIds, random: seededRandom(44) });
      expect(idsOf(pool)).toEqual(poolSnapshot);
      expect(playedIds).toEqual(playedSnapshot);
    });
  });
});
