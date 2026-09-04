"use client";

import { useEffect, useRef } from "react";

import type { Question } from "@/data/schema";
import { formatWithUnit } from "@/lib/format";
import type { RoundEntry } from "@/lib/game-reducer";
import { Button } from "./Button";
import { Confetti } from "./Confetti";
import { CountUp } from "./CountUp";
import { ScoreBadge } from "./ScoreBadge";
import { SourceList } from "./SourceList";

export interface RevealCardProps {
  question: Question;
  entry: RoundEntry;
  totalPoints: number;
  maxPoints: number;
  isLast: boolean;
  onNext: () => void;
}

/** Elemente, die Enter selbst auswerten – dort greift der globale Handler nicht. */
const INTERACTIVE_SELECTOR = "a, button, input, textarea, select, [contenteditable='true']";

/** Auflösung: Gegenüberstellung, Bewertung, Erklärung, Quellen (Abschnitt 3.3). */
export function RevealCard({
  question,
  entry,
  totalPoints,
  maxPoints,
  isLast,
  onNext,
}: RevealCardProps) {
  const nextLabel = isLast ? "Zum Ergebnis" : "Weiter";
  const nextButton = useRef<HTMLButtonElement>(null);

  // Fokus auf „Weiter“, damit Enter sofort weiterführt – aber ohne die Seite
  // zu scrollen: Sonst rutscht auf schmalen Geräten der Kopfbereich aus dem
  // Bild, nur weil die Karte etwas höher ist als der Viewport.
  useEffect(() => {
    nextButton.current?.focus({ preventScroll: true });
  }, []);

  // Enter führt weiter, auch wenn der Fokus irgendwo im Fließtext steht. Liegt
  // er auf einem Link oder einer Schaltfläche, macht der Browser das ohnehin
  // selbst – dann hält sich der Handler heraus, sonst löste er doppelt aus.
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (event.key !== "Enter" || event.defaultPrevented) return;
      const target = event.target;
      if (target instanceof Element && target.closest(INTERACTIVE_SELECTOR) !== null) return;
      event.preventDefault();
      onNext();
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [onNext]);

  return (
    <article
      data-testid="reveal-card"
      className="card animate-card-in flex min-h-[24rem] flex-col p-4 sm:p-6"
    >
      {entry.label === "Volltreffer!" && <Confetti />}

      <p className="text-ink/70 text-xs leading-snug sm:text-sm">{question.question}</p>

      <div
        role="status"
        aria-live="polite"
        className="border-ink/15 mt-2 border-t-[3px] border-dashed pt-2"
      >
        <div className="grid grid-cols-2 gap-3">
          <div>
            <h3 className="font-display text-ink/70 text-xs tracking-widest uppercase">
              Deine Schätzung
            </h3>
            <p
              data-testid="guess-value"
              className="font-display text-ink mt-0.5 text-xl leading-tight tracking-wide tabular-nums sm:text-2xl"
            >
              {entry.guess === null ? "Keine Ahnung" : formatWithUnit(entry.guess, question.unit)}
            </p>
          </div>
          <div>
            <h3 className="font-display text-ink/70 text-xs tracking-widest uppercase">
              Richtige Antwort
            </h3>
            <CountUp
              data-testid="answer-value"
              value={question.answer}
              unit={question.unit}
              className="font-display text-accent mt-0.5 block text-2xl leading-tight tracking-wide tabular-nums sm:text-3xl"
            />
          </div>
        </div>

        <div className="mt-3 flex justify-center">
          <ScoreBadge label={entry.label} points={entry.points} />
        </div>
      </div>

      <p
        data-testid="explanation"
        className="text-ink mt-3 text-sm leading-snug sm:text-base sm:leading-relaxed"
      >
        {question.explanation}
      </p>

      <SourceList sources={question.sources} />

      <div className="mt-auto pt-3">
        <p data-testid="round-score" className="font-display text-ink text-base tracking-wide">
          Punkte in dieser Runde: {totalPoints} von {maxPoints}
        </p>
        <Button ref={nextButton} data-testid="next-button" onClick={onNext} className="mt-2 w-full">
          {nextLabel}
        </Button>
      </div>
    </article>
  );
}
