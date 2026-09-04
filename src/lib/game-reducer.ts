/**
 * Zustandsautomat einer Runde: idle → question → reveal → finished.
 *
 * Reine Logik: kein React, keine Browser-APIs. Der Reducer ist pur und
 * immutabel; jede Aktion im falschen Zustand gibt denselben State
 * (dieselbe Referenz) unverändert zurück.
 */
import type { Category, Question } from "../data/schema";
import { MAX_POINTS_PER_QUESTION, scoreGuess, type ScoreLabel } from "./scoring";

export const ROUND_SIZES = [5, 10, 20] as const;
export type RoundSize = (typeof ROUND_SIZES)[number];
export const DEFAULT_ROUND_SIZE: RoundSize = 10;

export type GamePhase = "idle" | "question" | "reveal" | "finished";

export interface GameSettings {
  count: RoundSize;
  /** Leeres Array bedeutet: alle Kategorien. */
  categories: Category[];
}

export interface RoundEntry {
  questionId: string;
  guess: number | null;
  points: number;
  label: ScoreLabel;
  ratio: number;
}

export interface GameState {
  phase: GamePhase;
  settings: GameSettings;
  questions: Question[];
  index: number;
  entries: RoundEntry[];
  totalPoints: number;
  maxPoints: number;
}

export type GameAction =
  | { type: "START"; questions: Question[]; settings: GameSettings }
  | { type: "SUBMIT_GUESS"; guess: number }
  | { type: "SKIP" }
  | { type: "NEXT" }
  | { type: "RESET" };

export const initialGameState: GameState = {
  phase: "idle",
  settings: { count: DEFAULT_ROUND_SIZE, categories: [] },
  questions: [],
  index: 0,
  entries: [],
  totalPoints: 0,
  maxPoints: 0,
};

/** Hängt die Bewertung der aktuellen Frage an und wechselt in die Auflösung. */
function reveal(state: GameState, guess: number | null): GameState {
  const question = state.questions[state.index];
  if (question === undefined) return state;

  const result = scoreGuess(guess, question.answer);
  const entry: RoundEntry = {
    questionId: question.id,
    guess,
    points: result.points,
    label: result.label,
    ratio: result.ratio,
  };

  return {
    ...state,
    phase: "reveal",
    entries: [...state.entries, entry],
    totalPoints: state.totalPoints + result.points,
  };
}

export function gameReducer(state: GameState, action: GameAction): GameState {
  switch (action.type) {
    case "START": {
      // START gehört zum Übergang idle → question. Nach einer Runde führt der
      // Weg über RESET zurück nach idle, erst dann startet die nächste Runde.
      if (state.phase !== "idle") return state;
      if (action.questions.length === 0) return state;
      return {
        phase: "question",
        settings: action.settings,
        questions: action.questions,
        index: 0,
        entries: [],
        totalPoints: 0,
        maxPoints: action.questions.length * MAX_POINTS_PER_QUESTION,
      };
    }

    case "SUBMIT_GUESS": {
      if (state.phase !== "question") return state;
      return reveal(state, action.guess);
    }

    case "SKIP": {
      if (state.phase !== "question") return state;
      // „Keine Ahnung“: keine Schätzung, damit 0 Punkte.
      return reveal(state, null);
    }

    case "NEXT": {
      if (state.phase !== "reveal") return state;
      if (isLastQuestion(state)) {
        return { ...state, phase: "finished" };
      }
      return { ...state, phase: "question", index: state.index + 1 };
    }

    case "RESET":
      return initialGameState;
  }
}

/** Die Frage, die gerade dran ist – oder `null`, wenn keine läuft. */
export function currentQuestion(state: GameState): Question | null {
  return state.questions[state.index] ?? null;
}

/** Der Eintrag zur aktuellen Frage; nur in der Phase „reveal“ gesetzt. */
export function currentEntry(state: GameState): RoundEntry | null {
  if (state.phase !== "reveal") return null;
  return state.entries[state.index] ?? null;
}

/** Ob die aktuelle Frage die letzte der Runde ist. */
export function isLastQuestion(state: GameState): boolean {
  return state.questions.length > 0 && state.index === state.questions.length - 1;
}
