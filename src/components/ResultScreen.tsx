"use client";

import { useRouter } from "next/navigation";
import { useEffect, useRef } from "react";

import { useGame } from "./GameProvider";
import { ResultSummary } from "./ResultSummary";

/** Ergebnisbildschirm (Abschnitt 3.4). */
export function ResultScreen() {
  const router = useRouter();
  const { state, hydrated, newHighscore, reset } = useGame();
  // „Nochmal spielen“ navigiert selbst; der Wächter-Effekt hält sich dann raus.
  const leaving = useRef(false);

  useEffect(() => {
    if (!hydrated || leaving.current) return;
    if (state.phase !== "finished") {
      router.replace("/");
    }
  }, [hydrated, state.phase, router]);

  function handlePlayAgain(): void {
    leaving.current = true;
    reset();
    router.push("/");
  }

  const ready = hydrated && state.phase === "finished";

  return (
    <main className="mx-auto flex min-h-dvh max-w-[40rem] flex-col gap-3 px-4 py-4">
      <header className="text-center">
        <h1 className="font-display text-ink inline-block -rotate-2 text-3xl leading-none tracking-wide sm:text-4xl">
          Verschätz dich
        </h1>
      </header>

      {ready ? (
        <ResultSummary
          questions={state.questions}
          entries={state.entries}
          totalPoints={state.totalPoints}
          maxPoints={state.maxPoints}
          newHighscore={newHighscore}
          onPlayAgain={handlePlayAgain}
        />
      ) : (
        <div className="card flex min-h-[16rem] items-center justify-center p-4">
          <p className="font-display text-ink/50 text-lg tracking-wide">Einen Moment …</p>
        </div>
      )}
    </main>
  );
}
