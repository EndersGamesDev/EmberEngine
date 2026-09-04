import { describe, expect, it } from "vitest";

import { MAX_POINTS_PER_QUESTION, SCORE_LABELS, scoreGuess } from "@/lib/scoring";

describe("scoreGuess", () => {
  it("kennt genau sechs Labels in der Reihenfolge der Tabelle", () => {
    expect(SCORE_LABELS).toEqual([
      "Volltreffer!",
      "Fast perfekt",
      "Knapp daneben",
      "Nicht schlecht",
      "Naja …",
      "Voll verschätzt!",
    ]);
    expect(MAX_POINTS_PER_QUESTION).toBe(10);
  });

  it("gibt für die exakt richtige Zahl die volle Punktzahl", () => {
    expect(scoreGuess(384_400, 384_400)).toEqual({
      points: 10,
      label: "Volltreffer!",
      ratio: 1,
    });
  });

  describe("jede Stufe der Tabelle", () => {
    const cases = [
      { guess: 102, points: 10, label: "Volltreffer!" },
      { guess: 110, points: 7, label: "Fast perfekt" },
      { guess: 125, points: 5, label: "Knapp daneben" },
      { guess: 140, points: 3, label: "Nicht schlecht" },
      { guess: 180, points: 1, label: "Naja …" },
      { guess: 500, points: 0, label: "Voll verschätzt!" },
    ] as const;

    for (const { guess, points, label } of cases) {
      it(`${guess} auf 100 gibt ${points} Punkte („${label}“)`, () => {
        const result = scoreGuess(guess, 100);
        expect(result.points).toBe(points);
        expect(result.label).toBe(label);
      });
    }
  });

  describe("exakt an den Schwellen", () => {
    const cases = [
      { guess: 105, points: 10 },
      { guess: 105.01, points: 7 },
      { guess: 115, points: 7 },
      { guess: 115.01, points: 5 },
      { guess: 130, points: 5 },
      { guess: 130.01, points: 3 },
      { guess: 150, points: 3 },
      { guess: 150.01, points: 1 },
      { guess: 200, points: 1 },
      { guess: 200.01, points: 0 },
    ] as const;

    for (const { guess, points } of cases) {
      it(`${guess} auf 100 gibt ${points} Punkte`, () => {
        expect(scoreGuess(guess, 100).points).toBe(points);
      });
    }

    it("stolpert nicht über Fließkomma-Reste bei krummen Antworten", () => {
      // 3,45 / 3 ergibt in IEEE-754 1.1500000000000001 und läge ohne
      // Epsilon-Toleranz eine Stufe zu tief.
      expect(scoreGuess(3.45, 3).points).toBe(7);
      expect(scoreGuess(1.05, 1).points).toBe(10);
    });
  });

  describe("Symmetrie", () => {
    it("bewertet halb so viel wie doppelt so viel", () => {
      const half = scoreGuess(50, 100);
      const double = scoreGuess(200, 100);
      expect(half.points).toBe(double.points);
      expect(half.label).toBe(double.label);
      expect(half.ratio).toBeCloseTo(double.ratio, 12);
      expect(half.points).toBe(1);
    });

    it("liefert dasselbe Verhältnis, egal in welche Richtung man danebenliegt", () => {
      expect(scoreGuess(80, 100).ratio).toBeCloseTo(1.25, 12);
      expect(scoreGuess(125, 100).ratio).toBeCloseTo(1.25, 12);
    });
  });

  describe("Randfälle", () => {
    const invalidGuesses: (number | null)[] = [
      null,
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.NEGATIVE_INFINITY,
      -5,
      0,
    ];

    for (const guess of invalidGuesses) {
      it(`bewertet ${String(guess)} als „Voll verschätzt!“ mit ratio = Infinity`, () => {
        expect(scoreGuess(guess, 100)).toEqual({
          points: 0,
          label: "Voll verschätzt!",
          ratio: Number.POSITIVE_INFINITY,
        });
      });
    }

    it("liefert bei knapp verfehlter Schätzung das echte Verhältnis, nicht Infinity", () => {
      const result = scoreGuess(300, 100);
      expect(result.points).toBe(0);
      expect(result.ratio).toBeCloseTo(3, 12);
    });

    it("bleibt auch bei einer unmöglichen Antwort total", () => {
      // Das Schema garantiert answer > 0; die Funktion darf trotzdem nicht werfen.
      expect(scoreGuess(10, 0).points).toBe(0);
      expect(scoreGuess(10, Number.NaN).ratio).toBe(Number.POSITIVE_INFINITY);
    });

    it("kommt mit sehr kleinen und sehr großen Antworten zurecht", () => {
      expect(scoreGuess(0.5, 0.5).points).toBe(10);
      expect(scoreGuess(4e19, 4.3252003274489856e19).points).toBe(7);
    });
  });
});
