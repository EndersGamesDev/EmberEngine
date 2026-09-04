import { describe, expect, it } from "vitest";

import {
  DEFAULT_ROUND_SIZE,
  ROUND_SIZES,
  currentEntry,
  currentQuestion,
  gameReducer,
  initialGameState,
  isLastQuestion,
  type GameSettings,
  type GameState,
} from "@/lib/game-reducer";

import { makeQuestions } from "../fixtures/questions";

const SETTINGS: GameSettings = { count: 5, categories: [] };

/** Drei Fragen mit den Antworten 10, 20 und 30. */
const threeQuestions = makeQuestions(3);

function startedRound(): GameState {
  return gameReducer(initialGameState, {
    type: "START",
    questions: threeQuestions,
    settings: SETTINGS,
  });
}

describe("Konstanten", () => {
  it("bietet die Rundengrößen 5, 10 und 20 mit Standard 10 an", () => {
    expect(ROUND_SIZES).toEqual([5, 10, 20]);
    expect(DEFAULT_ROUND_SIZE).toBe(10);
  });

  it("startet im Zustand idle ohne Punkte", () => {
    expect(initialGameState).toEqual({
      phase: "idle",
      settings: { count: 10, categories: [] },
      questions: [],
      index: 0,
      entries: [],
      totalPoints: 0,
      maxPoints: 0,
    });
  });
});

describe("START", () => {
  it("wechselt nach question und setzt die Maximalpunktzahl", () => {
    const state = startedRound();
    expect(state.phase).toBe("question");
    expect(state.index).toBe(0);
    expect(state.questions).toEqual(threeQuestions);
    expect(state.settings).toEqual(SETTINGS);
    expect(state.maxPoints).toBe(30);
    expect(state.totalPoints).toBe(0);
    expect(state.entries).toEqual([]);
  });

  it("bleibt bei null Fragen unverändert im Zustand idle", () => {
    const state = gameReducer(initialGameState, {
      type: "START",
      questions: [],
      settings: SETTINGS,
    });
    expect(state).toBe(initialGameState);
    expect(state.phase).toBe("idle");
  });
});

describe("kompletter Durchlauf einer 3-Fragen-Runde", () => {
  it("summiert die Punkte korrekt und endet in finished", () => {
    // Antworten der Fixtures: 10, 20, 30.
    let state = startedRound();

    // Frage 1: exakt richtig → 10 Punkte.
    state = gameReducer(state, { type: "SUBMIT_GUESS", guess: 10 });
    expect(state.phase).toBe("reveal");
    expect(state.entries).toHaveLength(1);
    expect(state.totalPoints).toBe(10);
    expect(currentEntry(state)).toEqual({
      questionId: "q001",
      guess: 10,
      points: 10,
      label: "Volltreffer!",
      ratio: 1,
    });

    state = gameReducer(state, { type: "NEXT" });
    expect(state.phase).toBe("question");
    expect(state.index).toBe(1);

    // Frage 2: „Keine Ahnung“ → 0 Punkte.
    state = gameReducer(state, { type: "SKIP" });
    expect(state.phase).toBe("reveal");
    expect(state.totalPoints).toBe(10);
    expect(currentEntry(state)).toEqual({
      questionId: "q002",
      guess: null,
      points: 0,
      label: "Voll verschätzt!",
      ratio: Number.POSITIVE_INFINITY,
    });

    state = gameReducer(state, { type: "NEXT" });
    expect(state.index).toBe(2);
    expect(isLastQuestion(state)).toBe(true);

    // Frage 3: doppelt so viel wie 30 → 1 Punkt.
    state = gameReducer(state, { type: "SUBMIT_GUESS", guess: 60 });
    expect(state.totalPoints).toBe(11);

    state = gameReducer(state, { type: "NEXT" });
    expect(state.phase).toBe("finished");
    expect(state.entries).toHaveLength(3);
    expect(state.totalPoints).toBe(11);
    expect(state.maxPoints).toBe(30);
    expect(state.entries.reduce((sum, entry) => sum + entry.points, 0)).toBe(state.totalPoints);
  });
});

describe("Aktionen im falschen Zustand", () => {
  it("ignoriert SUBMIT_GUESS, SKIP und NEXT im Zustand idle", () => {
    expect(gameReducer(initialGameState, { type: "SUBMIT_GUESS", guess: 1 })).toBe(
      initialGameState,
    );
    expect(gameReducer(initialGameState, { type: "SKIP" })).toBe(initialGameState);
    expect(gameReducer(initialGameState, { type: "NEXT" })).toBe(initialGameState);
  });

  it("ignoriert START und NEXT im Zustand question", () => {
    const state = startedRound();
    expect(
      gameReducer(state, { type: "START", questions: threeQuestions, settings: SETTINGS }),
    ).toBe(state);
    expect(gameReducer(state, { type: "NEXT" })).toBe(state);
  });

  it("ignoriert SUBMIT_GUESS, SKIP und START im Zustand reveal", () => {
    const state = gameReducer(startedRound(), { type: "SUBMIT_GUESS", guess: 10 });
    expect(gameReducer(state, { type: "SUBMIT_GUESS", guess: 99 })).toBe(state);
    expect(gameReducer(state, { type: "SKIP" })).toBe(state);
    expect(
      gameReducer(state, { type: "START", questions: threeQuestions, settings: SETTINGS }),
    ).toBe(state);
  });

  it("ignoriert alles außer RESET im Zustand finished", () => {
    const oneQuestion = makeQuestions(1);
    let state = gameReducer(initialGameState, {
      type: "START",
      questions: oneQuestion,
      settings: SETTINGS,
    });
    state = gameReducer(state, { type: "SKIP" });
    state = gameReducer(state, { type: "NEXT" });
    expect(state.phase).toBe("finished");

    expect(gameReducer(state, { type: "NEXT" })).toBe(state);
    expect(gameReducer(state, { type: "SUBMIT_GUESS", guess: 1 })).toBe(state);
    expect(gameReducer(state, { type: "SKIP" })).toBe(state);
    expect(
      gameReducer(state, { type: "START", questions: threeQuestions, settings: SETTINGS }),
    ).toBe(state);
    expect(gameReducer(state, { type: "RESET" })).toBe(initialGameState);
  });
});

describe("RESET", () => {
  it("führt aus jedem Zustand zurück zum Ausgangszustand", () => {
    const question = startedRound();
    const revealed = gameReducer(question, { type: "SUBMIT_GUESS", guess: 10 });

    expect(gameReducer(initialGameState, { type: "RESET" })).toBe(initialGameState);
    expect(gameReducer(question, { type: "RESET" })).toBe(initialGameState);
    expect(gameReducer(revealed, { type: "RESET" })).toBe(initialGameState);
  });
});

describe("Reinheit", () => {
  it("verändert den übergebenen Zustand nicht", () => {
    const state = startedRound();
    const snapshot = structuredClone(state);
    gameReducer(state, { type: "SUBMIT_GUESS", guess: 42 });
    gameReducer(state, { type: "SKIP" });
    expect(state).toEqual(snapshot);
  });
});

describe("Selektoren", () => {
  it("currentQuestion liefert die Frage am aktuellen Index", () => {
    expect(currentQuestion(initialGameState)).toBeNull();
    const state = startedRound();
    expect(currentQuestion(state)?.id).toBe("q001");
    const next = gameReducer(gameReducer(state, { type: "SKIP" }), { type: "NEXT" });
    expect(currentQuestion(next)?.id).toBe("q002");
  });

  it("currentEntry liefert nur in der Auflösung einen Eintrag", () => {
    const state = startedRound();
    expect(currentEntry(state)).toBeNull();
    const revealed = gameReducer(state, { type: "SUBMIT_GUESS", guess: 10 });
    expect(currentEntry(revealed)?.questionId).toBe("q001");
    const next = gameReducer(revealed, { type: "NEXT" });
    expect(currentEntry(next)).toBeNull();
  });

  it("isLastQuestion erkennt die letzte Frage", () => {
    expect(isLastQuestion(initialGameState)).toBe(false);
    let state = startedRound();
    expect(isLastQuestion(state)).toBe(false);
    state = gameReducer(gameReducer(state, { type: "SKIP" }), { type: "NEXT" });
    expect(isLastQuestion(state)).toBe(false);
    state = gameReducer(gameReducer(state, { type: "SKIP" }), { type: "NEXT" });
    expect(isLastQuestion(state)).toBe(true);
  });
});
