"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useState,
  type ReactNode,
} from "react";

import { questions as questionPool } from "@/data/questions";
import {
  gameReducer,
  initialGameState,
  type GameSettings,
  type GameState,
} from "@/lib/game-reducer";
import { selectQuestions } from "@/lib/select-questions";
import {
  clearGameSession,
  clearNewHighscoreFlag,
  loadGameSession,
  loadNewHighscoreFlag,
  loadPlayedIds,
  saveGameSession,
  saveNewHighscoreFlag,
  savePlayedIds,
  saveSettings,
} from "@/lib/storage";
import { useHydrated } from "./useHydrated";

export interface GameContextValue {
  state: GameState;
  /** `false`, bis der Spielstand aus dem `sessionStorage` gelesen wurde. */
  hydrated: boolean;
  /** Zieht die Fragen, startet die Runde und meldet, ob das geklappt hat. */
  startRound: (settings: GameSettings) => boolean;
  submitGuess: (value: number) => void;
  skip: () => void;
  next: () => void;
  reset: () => void;
  newHighscore: boolean;
  /** Merkt (auch über einen Reload hinweg), ob die Runde ein Bestwert war. */
  markNewHighscore: (value: boolean) => void;
}

const GameContext = createContext<GameContextValue | null>(null);

/**
 * Liest eine laufende Runde aus dem `sessionStorage`.
 *
 * Läuft als Lazy-Initializer schon im ersten Client-Render – nicht in einem
 * Effekt. Auf dem Server gibt es kein `sessionStorage`, dort bleibt es beim
 * Startzustand. Einen Hydration-Unterschied kann das nicht auslösen, weil alle
 * Bildschirme bis `hydrated === true` nur ihren neutralen Platzhalter zeigen.
 */
function restoreState(fallback: GameState): GameState {
  return loadGameSession() ?? fallback;
}

export function GameProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(gameReducer, initialGameState, restoreState);
  const hydrated = useHydrated();
  const [newHighscore, setNewHighscore] = useState(loadNewHighscoreFlag);

  useEffect(() => {
    if (!hydrated) return;
    if (state.phase === "idle") {
      clearGameSession();
      return;
    }
    saveGameSession(state);
  }, [hydrated, state]);

  const startRound = useCallback((settings: GameSettings): boolean => {
    saveSettings(settings);

    const selection = selectQuestions({
      pool: questionPool,
      count: settings.count,
      categories: settings.categories,
      playedIds: loadPlayedIds(),
    });
    if (selection.questions.length === 0) return false;
    savePlayedIds(selection.playedIds);

    clearNewHighscoreFlag();
    setNewHighscore(false);
    // `START` wirkt nur im Zustand `idle`; nach einer Runde führt der Weg
    // deshalb erst über `RESET`.
    dispatch({ type: "RESET" });
    dispatch({ type: "START", questions: selection.questions, settings });
    return true;
  }, []);

  const submitGuess = useCallback((value: number) => {
    dispatch({ type: "SUBMIT_GUESS", guess: value });
  }, []);

  const skip = useCallback(() => {
    dispatch({ type: "SKIP" });
  }, []);

  const next = useCallback(() => {
    dispatch({ type: "NEXT" });
  }, []);

  const reset = useCallback(() => {
    dispatch({ type: "RESET" });
    clearGameSession();
    clearNewHighscoreFlag();
    setNewHighscore(false);
  }, []);

  const markNewHighscore = useCallback((value: boolean) => {
    setNewHighscore(value);
    saveNewHighscoreFlag(value);
  }, []);

  const value = useMemo<GameContextValue>(
    () => ({
      state,
      hydrated,
      startRound,
      submitGuess,
      skip,
      next,
      reset,
      newHighscore,
      markNewHighscore,
    }),
    [state, hydrated, startRound, submitGuess, skip, next, reset, newHighscore, markNewHighscore],
  );

  return <GameContext.Provider value={value}>{children}</GameContext.Provider>;
}

export function useGame(): GameContextValue {
  const context = useContext(GameContext);
  if (context === null) {
    throw new Error("useGame muss innerhalb von <GameProvider> verwendet werden.");
  }
  return context;
}
