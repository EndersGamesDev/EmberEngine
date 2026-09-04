"use client";

import { useRouter } from "next/navigation";
import { useEffect, useRef } from "react";

import { currentEntry, currentQuestion, isLastQuestion, type GameState } from "@/lib/game-reducer";
import { isNewHighscore, loadHighscores, saveHighscore } from "@/lib/storage";
import { EstimateInput } from "./EstimateInput";
import { useGame } from "./GameProvider";
import { ProgressBar } from "./ProgressBar";
import { QuestionCard } from "./QuestionCard";
import { RevealCard } from "./RevealCard";

/** Platzhalter mit derselben Kartenhülle – so springt beim Hydrieren nichts. */
function PlaceholderCard() {
  return (
    <div className="card flex min-h-[24rem] items-center justify-center p-4 sm:p-6">
      <p className="font-display text-ink/50 text-lg tracking-wide">Einen Moment …</p>
    </div>
  );
}

/** Schreibt die Bestleistung fort und meldet, ob es ein neuer Bestwert war. */
function recordHighscore(state: GameState): boolean {
  const { count } = state.settings;
  const isNew = isNewHighscore(loadHighscores(), state.totalPoints, count);
  saveHighscore({
    points: state.totalPoints,
    maxPoints: state.maxPoints,
    percent: state.maxPoints > 0 ? Math.round((state.totalPoints / state.maxPoints) * 100) : 0,
    count,
    achievedAt: new Date().toISOString(),
  });
  return isNew;
}

/** Frage- und Auflösungsbildschirm (Abschnitte 3.2 und 3.3). */
export function PlayScreen() {
  const router = useRouter();
  const { state, hydrated, submitGuess, skip, next, markNewHighscore } = useGame();
  // Beim Abschluss der Runde navigiert der Klick-Handler selbst; dann soll der
  // Wächter-Effekt nicht zusätzlich umleiten.
  const finishing = useRef(false);

  useEffect(() => {
    if (!hydrated) return;
    if (state.phase === "idle") {
      router.replace("/");
      return;
    }
    if (state.phase === "finished" && !finishing.current) {
      router.replace("/ergebnis");
    }
  }, [hydrated, state.phase, router]);

  const question = currentQuestion(state);
  const entry = currentEntry(state);
  const total = state.questions.length;
  const showGame = hydrated && (state.phase === "question" || state.phase === "reveal");

  function handleNext(): void {
    if (!isLastQuestion(state)) {
      next();
      return;
    }
    finishing.current = true;
    markNewHighscore(recordHighscore(state));
    next();
    router.push("/ergebnis");
  }

  return (
    <main className="mx-auto flex min-h-dvh max-w-[40rem] flex-col gap-3 px-4 py-3">
      <header className="flex items-baseline justify-between gap-3">
        <h1 className="font-display text-ink -rotate-2 text-lg leading-none tracking-wide">
          Verschätz dich
        </h1>
        <p className="font-display text-ink/70 text-xs tracking-widest uppercase">
          Daneben ist auch drin
        </p>
      </header>

      {showGame && question !== null ? (
        <>
          <ProgressBar current={state.index + 1} total={total} />
          {state.phase === "question" ? (
            <QuestionCard question={question}>
              {/* `key`: Jede neue Frage bekommt ein frisches Eingabefeld – leer
                  und mit Fokus (Abschnitt 3.2). */}
              <EstimateInput
                key={question.id}
                unit={question.unit}
                questionId={question.id}
                onSubmit={submitGuess}
                onSkip={skip}
              />
            </QuestionCard>
          ) : (
            entry !== null && (
              <RevealCard
                // `key`: Der Count-up soll bei jeder Frage neu loslaufen.
                key={question.id}
                question={question}
                entry={entry}
                totalPoints={state.totalPoints}
                maxPoints={state.maxPoints}
                isLast={isLastQuestion(state)}
                onNext={handleNext}
              />
            )
          )}
        </>
      ) : (
        <PlaceholderCard />
      )}
    </main>
  );
}
