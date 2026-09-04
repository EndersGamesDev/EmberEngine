/**
 * Punktesystem (Spezifikation Abschnitt 4).
 *
 * Reine Logik: kein React, keine Browser-APIs. Bewertet wird symmetrisch über
 * das Verhältnis `max(guess / answer, answer / guess)`, damit „halb so viel“
 * und „doppelt so viel“ gleich viele Punkte geben.
 */

export const SCORE_LABELS = [
  "Volltreffer!",
  "Fast perfekt",
  "Knapp daneben",
  "Nicht schlecht",
  "Naja …",
  "Voll verschätzt!",
] as const;

export type ScoreLabel = (typeof SCORE_LABELS)[number];

export const MAX_POINTS_PER_QUESTION = 10;

export interface ScoreResult {
  points: number;
  label: ScoreLabel;
  ratio: number;
}

/**
 * Fließkomma-Toleranz für die Schwellenvergleiche.
 *
 * Die Schwellen 1,05 / 1,15 / 1,3 / 1,5 / 2 sind in IEEE-754 nicht exakt
 * darstellbar. Für „glatte“ Fälle wie 115/100 trifft die Division zwar
 * denselben Double wie das Literal 1.15, bei anderen Antworten aber nicht:
 * 3,45 / 3 ergibt 1.1500000000000001 und läge ohne Toleranz eine Stufe zu
 * tief. Ein absolutes Epsilon von 1e-9 ist um Größenordnungen kleiner als
 * jeder Punkteunterschied und größer als der Rundungsfehler in diesem
 * Wertebereich.
 */
const THRESHOLD_EPSILON = 1e-9;

const SCORE_STEPS = [
  { maxRatio: 1.05, points: 10, label: "Volltreffer!" },
  { maxRatio: 1.15, points: 7, label: "Fast perfekt" },
  { maxRatio: 1.3, points: 5, label: "Knapp daneben" },
  { maxRatio: 1.5, points: 3, label: "Nicht schlecht" },
  { maxRatio: 2, points: 1, label: "Naja …" },
] as const satisfies readonly { maxRatio: number; points: number; label: ScoreLabel }[];

/** Ergebnis für ungültige Eingaben: 0 Punkte, schlechtestes Label, `ratio = Infinity`. */
function missedCompletely(): ScoreResult {
  return { points: 0, label: "Voll verschätzt!", ratio: Number.POSITIVE_INFINITY };
}

/**
 * Bewertet eine Schätzung gegen die richtige Antwort.
 *
 * `guess` darf `null` sein („Keine Ahnung“). `null`, `NaN`, `±Infinity`,
 * negative Werte und 0 geben 0 Punkte, das Label „Voll verschätzt!“ und
 * `ratio = Infinity`. `answer` ist per Schema immer endlich und > 0; die
 * Prüfung hier hält die Funktion trotzdem total.
 */
export function scoreGuess(guess: number | null, answer: number): ScoreResult {
  if (guess === null || !Number.isFinite(guess) || guess <= 0) {
    return missedCompletely();
  }
  if (!Number.isFinite(answer) || answer <= 0) {
    return missedCompletely();
  }

  const ratio = Math.max(guess / answer, answer / guess);

  for (const step of SCORE_STEPS) {
    if (ratio <= step.maxRatio + THRESHOLD_EPSILON) {
      return { points: step.points, label: step.label, ratio };
    }
  }

  return { points: 0, label: "Voll verschätzt!", ratio };
}
