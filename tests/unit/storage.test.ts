import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { gameReducer, initialGameState, type GameState } from "@/lib/game-reducer";
import {
  STORAGE_KEYS,
  clearGameSession,
  isNewHighscore,
  loadGameSession,
  loadHighscores,
  loadPlayedIds,
  loadSettings,
  saveGameSession,
  saveHighscore,
  savePlayedIds,
  saveSettings,
  type Highscore,
} from "@/lib/storage";

import { makeQuestions } from "../fixtures/questions";

/** Minimaler In-Memory-Ersatz für `Storage`. */
function createStorageMock() {
  const store = new Map<string, string>();
  return {
    store,
    getItem: (key: string): string | null => store.get(key) ?? null,
    setItem: (key: string, value: string): void => {
      store.set(key, value);
    },
    removeItem: (key: string): void => {
      store.delete(key);
    },
    clear: (): void => {
      store.clear();
    },
    key: (index: number): string | null => [...store.keys()][index] ?? null,
    get length(): number {
      return store.size;
    },
  };
}

type StorageMock = ReturnType<typeof createStorageMock>;

let localStorageMock: StorageMock;
let sessionStorageMock: StorageMock;

function highscore(overrides: Partial<Highscore> = {}): Highscore {
  return {
    points: 63,
    maxPoints: 100,
    percent: 63,
    count: 10,
    achievedAt: "2026-01-01T12:00:00.000Z",
    ...overrides,
  };
}

/** Eine echte, mit dem Reducer gespielte Runde als Session-Fixture. */
function playedRound(): GameState {
  const questions = makeQuestions(2);
  let state = gameReducer(initialGameState, {
    type: "START",
    questions,
    settings: { count: 10, categories: ["Tiere & Natur"] },
  });
  state = gameReducer(state, { type: "SUBMIT_GUESS", guess: 10 });
  state = gameReducer(state, { type: "NEXT" });
  // Übersprungene Frage: `ratio` ist Infinity und überlebt JSON nur mit Sonderbehandlung.
  state = gameReducer(state, { type: "SKIP" });
  return state;
}

beforeEach(() => {
  localStorageMock = createStorageMock();
  sessionStorageMock = createStorageMock();
  vi.stubGlobal("window", {
    localStorage: localStorageMock,
    sessionStorage: sessionStorageMock,
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Highscores", () => {
  it("liefert ohne gespeicherten Wert ein leeres Objekt", () => {
    expect(loadHighscores()).toEqual({});
  });

  it("speichert je Rundengröße getrennt", () => {
    saveHighscore(highscore({ points: 40, maxPoints: 50, percent: 80, count: 5 }));
    saveHighscore(highscore({ points: 63, maxPoints: 100, percent: 63, count: 10 }));

    const highscores = loadHighscores();
    expect(highscores[5]?.points).toBe(40);
    expect(highscores[10]?.points).toBe(63);
    expect(highscores[20]).toBeUndefined();
  });

  it("ersetzt einen Eintrag nur bei Verbesserung", () => {
    saveHighscore(highscore({ points: 63 }));
    saveHighscore(highscore({ points: 50, achievedAt: "2026-02-02T12:00:00.000Z" }));
    expect(loadHighscores()[10]?.points).toBe(63);

    saveHighscore(highscore({ points: 63, achievedAt: "2026-02-02T12:00:00.000Z" }));
    expect(loadHighscores()[10]?.achievedAt).toBe("2026-01-01T12:00:00.000Z");

    saveHighscore(highscore({ points: 71, achievedAt: "2026-03-03T12:00:00.000Z" }));
    expect(loadHighscores()[10]?.points).toBe(71);
    expect(loadHighscores()[10]?.achievedAt).toBe("2026-03-03T12:00:00.000Z");
  });

  it("bewertet einen neuen Highscore pur, ohne den Speicher zu lesen", () => {
    expect(isNewHighscore({}, 0, 10)).toBe(true);
    expect(isNewHighscore({ 10: highscore({ points: 63 }) }, 64, 10)).toBe(true);
    expect(isNewHighscore({ 10: highscore({ points: 63 }) }, 63, 10)).toBe(false);
    expect(isNewHighscore({ 10: highscore({ points: 63 }) }, 62, 10)).toBe(false);
    expect(isNewHighscore({ 10: highscore({ points: 63 }) }, 1, 5)).toBe(true);
  });

  it("ignoriert ungültiges JSON und ungültige Strukturen", () => {
    localStorageMock.store.set(STORAGE_KEYS.highscores, "{kaputt");
    expect(loadHighscores()).toEqual({});

    localStorageMock.store.set(STORAGE_KEYS.highscores, JSON.stringify({ 10: { points: "viel" } }));
    expect(loadHighscores()).toEqual({});

    // Rundengröße 7 gibt es nicht – der ganze Datensatz fällt durch.
    localStorageMock.store.set(
      STORAGE_KEYS.highscores,
      JSON.stringify({ 7: { ...highscore(), count: 7 } }),
    );
    expect(loadHighscores()).toEqual({});
  });

  it("indiziert nach dem Feld count, nicht nach dem JSON-Schlüssel", () => {
    localStorageMock.store.set(
      STORAGE_KEYS.highscores,
      JSON.stringify({ irgendwas: highscore({ points: 12, count: 5 }) }),
    );
    expect(loadHighscores()[5]?.points).toBe(12);
    expect(loadHighscores()[10]).toBeUndefined();
  });
});

describe("gespielte Fragen", () => {
  it("schreibt und liest die IDs", () => {
    expect(loadPlayedIds()).toEqual([]);
    savePlayedIds(["q001", "q002"]);
    expect(loadPlayedIds()).toEqual(["q001", "q002"]);
  });

  it("liefert bei ungültigem Inhalt eine leere Liste", () => {
    localStorageMock.store.set(STORAGE_KEYS.playedIds, "[[");
    expect(loadPlayedIds()).toEqual([]);
    localStorageMock.store.set(STORAGE_KEYS.playedIds, JSON.stringify([1, 2, 3]));
    expect(loadPlayedIds()).toEqual([]);
  });
});

describe("laufende Runde", () => {
  it("überlebt einen Roundtrip durch sessionStorage – inklusive ratio = Infinity", () => {
    const state = playedRound();
    saveGameSession(state);

    // Wirklich im sessionStorage, nicht im localStorage.
    expect(sessionStorageMock.store.has(STORAGE_KEYS.session)).toBe(true);
    expect(localStorageMock.store.has(STORAGE_KEYS.session)).toBe(false);

    const restored = loadGameSession();
    expect(restored).toEqual(state);
    expect(restored?.entries[1]?.ratio).toBe(Number.POSITIVE_INFINITY);
  });

  it("liefert null ohne gespeicherte Runde", () => {
    expect(loadGameSession()).toBeNull();
  });

  it("liefert null bei ungültiger Struktur", () => {
    sessionStorageMock.store.set(STORAGE_KEYS.session, "{kaputt");
    expect(loadGameSession()).toBeNull();

    sessionStorageMock.store.set(STORAGE_KEYS.session, JSON.stringify({ phase: "zwischendrin" }));
    expect(loadGameSession()).toBeNull();

    const state = playedRound();
    sessionStorageMock.store.set(
      STORAGE_KEYS.session,
      JSON.stringify({ ...state, questions: [{ id: "q001" }] }),
    );
    expect(loadGameSession()).toBeNull();

    sessionStorageMock.store.set(
      STORAGE_KEYS.session,
      JSON.stringify({ ...state, settings: { count: 7, categories: [] } }),
    );
    expect(loadGameSession()).toBeNull();
  });

  it("löscht die Runde wieder", () => {
    saveGameSession(playedRound());
    clearGameSession();
    expect(loadGameSession()).toBeNull();
  });
});

describe("Einstellungen", () => {
  it("liefert ohne gespeicherte Werte den Standard", () => {
    expect(loadSettings()).toEqual({ count: 10, categories: [] });
  });

  it("schreibt und liest die Einstellungen", () => {
    saveSettings({ count: 20, categories: ["Sport", "Geschichte"] });
    expect(loadSettings()).toEqual({ count: 20, categories: ["Sport", "Geschichte"] });
  });

  it("fällt bei ungültigen Werten auf den Standard zurück", () => {
    localStorageMock.store.set(
      STORAGE_KEYS.settings,
      JSON.stringify({ count: 7, categories: ["Gibt es nicht"] }),
    );
    expect(loadSettings()).toEqual({ count: 10, categories: [] });
  });
});

describe("ohne Browser", () => {
  it("wirft ohne window nicht und liefert Standardwerte", () => {
    vi.stubGlobal("window", undefined);
    expect(typeof window).toBe("undefined");

    expect(() => {
      saveHighscore(highscore());
      savePlayedIds(["q001"]);
      saveGameSession(playedRound());
      saveSettings({ count: 5, categories: [] });
      clearGameSession();
    }).not.toThrow();

    expect(loadHighscores()).toEqual({});
    expect(loadPlayedIds()).toEqual([]);
    expect(loadGameSession()).toBeNull();
    expect(loadSettings()).toEqual({ count: 10, categories: [] });
  });

  it("wirft nicht, wenn der Zugriff auf den Speicher selbst blockiert ist", () => {
    // Safari im Privatmodus wirft schon beim Lesen von `window.localStorage`.
    vi.stubGlobal("window", {
      get localStorage(): Storage {
        throw new Error("Zugriff verweigert");
      },
      get sessionStorage(): Storage {
        throw new Error("Zugriff verweigert");
      },
    });

    expect(() => {
      saveHighscore(highscore());
      saveGameSession(playedRound());
    }).not.toThrow();
    expect(loadHighscores()).toEqual({});
    expect(loadGameSession()).toBeNull();
  });

  it("wirft nicht, wenn der Speicher voll ist", () => {
    vi.stubGlobal("window", {
      localStorage: {
        ...createStorageMock(),
        setItem: (): void => {
          throw new Error("QuotaExceededError");
        },
      },
      sessionStorage: createStorageMock(),
    });

    expect(() => {
      savePlayedIds(["q001"]);
    }).not.toThrow();
  });
});
