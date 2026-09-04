"use client";

import { useEffect, useRef, useState } from "react";

import type { Question } from "@/data/schema";
import { formatWithUnit } from "@/lib/format";
import type { RoundEntry } from "@/lib/game-reducer";
import { Button } from "./Button";

export interface ResultSummaryProps {
  questions: Question[];
  entries: RoundEntry[];
  totalPoints: number;
  maxPoints: number;
  newHighscore: boolean;
  onPlayAgain: () => void;
}

const MAX_QUESTION_LENGTH = 60;
const COPY_FEEDBACK_MS = 2000;

/** Sprüche von grandios bis grandios daneben (Abschnitt 3.4). */
const SAYINGS: readonly { minPercent: number; text: string }[] = [
  {
    minPercent: 90,
    text: "Wahnsinn! Du hast ein Gefühl für Zahlen, das sonst nur Taschenrechner haben.",
  },
  {
    minPercent: 70,
    text: "Stark geschätzt! Du liegst so oft richtig, dass es langsam unheimlich wird.",
  },
  {
    minPercent: 50,
    text: "Solide Mitte: genug gewusst, um mitzureden, genug danebengelegen, um zu staunen.",
  },
  {
    minPercent: 30,
    text: "Da war was! Ein paar Treffer, ein paar Ausrutscher – genau dafür gibt es dieses Spiel.",
  },
  {
    minPercent: 10,
    text: "Daneben ist auch drin. Und du warst heute wirklich beeindruckend oft daneben.",
  },
  {
    minPercent: 0,
    text: "Konsequent vorbei an jeder Zahl. Das muss man erst mal schaffen – bitte sofort nochmal.",
  },
];

function sayingFor(percent: number): string {
  for (const saying of SAYINGS) {
    if (percent >= saying.minPercent) return saying.text;
  }
  return SAYINGS[SAYINGS.length - 1]?.text ?? "";
}

/** Kürzt lange Fragen für die Tabelle. */
function shorten(text: string, maxLength = MAX_QUESTION_LENGTH): string {
  if (text.length <= maxLength) return text;
  return `${text.slice(0, maxLength - 1).trimEnd()}…`;
}

interface Extremes {
  bestIndex: number;
  worstIndex: number | null;
}

/**
 * Beste und schlechteste Zeile. Bei Gleichstand gewinnt jeweils die erste
 * Zeile; sind alle Punkte gleich, gibt es keine schlechteste.
 */
function findExtremes(entries: readonly RoundEntry[]): Extremes | null {
  const first = entries[0];
  if (first === undefined) return null;

  let bestIndex = 0;
  let worstIndex = 0;
  let allEqual = true;

  entries.forEach((entry, index) => {
    if (entry.points !== first.points) allEqual = false;
    const best = entries[bestIndex];
    const worst = entries[worstIndex];
    if (best !== undefined && entry.points > best.points) bestIndex = index;
    if (worst !== undefined && entry.points < worst.points) worstIndex = index;
  });

  return { bestIndex, worstIndex: allEqual ? null : worstIndex };
}

/** Ergebnisübersicht einer Runde (Abschnitt 3.4). */
export function ResultSummary({
  questions,
  entries,
  totalPoints,
  maxPoints,
  newHighscore,
  onPlayAgain,
}: ResultSummaryProps) {
  const [copied, setCopied] = useState(false);
  const [showFallback, setShowFallback] = useState(false);
  const feedbackTimer = useRef<number | null>(null);

  useEffect(() => {
    return () => {
      if (feedbackTimer.current !== null) window.clearTimeout(feedbackTimer.current);
    };
  }, []);

  const percent = maxPoints > 0 ? Math.round((totalPoints / maxPoints) * 100) : 0;
  const extremes = findExtremes(entries);
  const shareText = `Ich habe bei Verschätz dich ${totalPoints} von ${maxPoints} Punkten geholt (${percent} %). Schaffst du mehr?`;

  async function handleCopy(): Promise<void> {
    // Ohne Clipboard-API (alte Browser, unsicherer Kontext) bekommt der Nutzer
    // den Text zum Selbstkopieren – der Fehler wird also nicht versteckt,
    // sondern in eine sichtbare Alternative übersetzt.
    const clipboard: Clipboard | undefined = navigator.clipboard;
    if (clipboard === undefined) {
      setShowFallback(true);
      return;
    }
    try {
      await clipboard.writeText(shareText);
      setCopied(true);
      setShowFallback(false);
      if (feedbackTimer.current !== null) window.clearTimeout(feedbackTimer.current);
      feedbackTimer.current = window.setTimeout(() => {
        setCopied(false);
      }, COPY_FEEDBACK_MS);
    } catch {
      setShowFallback(true);
    }
  }

  return (
    <div className="card animate-card-in p-4 sm:p-6">
      <p className="font-display text-ink/70 text-xs tracking-widest uppercase">Dein Ergebnis</p>
      <p
        data-testid="result-total"
        className="font-display text-ink mt-1 text-4xl leading-none tracking-wide tabular-nums sm:text-5xl"
      >
        {totalPoints} von {maxPoints} Punkten
      </p>
      <p
        data-testid="result-percent"
        className="font-display text-accent mt-1 text-2xl tracking-wide tabular-nums"
      >
        {percent} %
      </p>
      <p data-testid="result-saying" className="text-ink mt-2 text-base leading-relaxed">
        {sayingFor(percent)}
      </p>

      {newHighscore && (
        <p
          data-testid="new-highscore"
          className="font-display border-ink text-ink mt-3 inline-block -rotate-1 rounded-lg border-[3px] bg-[#57D175] px-3 py-1 text-lg tracking-wide shadow-[3px_3px_0_0_var(--color-ink)]"
        >
          Neuer Highscore!
        </p>
      )}

      <div className="mt-5 overflow-x-auto">
        {/*
          `table-fixed` plus feste Spaltenanteile: Sonst zwingen lange Wörter
          wie „Chromosomen“ die Tabelle breiter als das Handy-Display, und die
          Punktespalte rutscht aus dem Bild.
        */}
        <table className="w-full table-fixed border-collapse text-left text-xs sm:text-sm">
          <caption className="sr-only">Alle Fragen dieser Runde mit Schätzung und Punkten</caption>
          <colgroup>
            <col className="w-[36%]" />
            <col className="w-[23%]" />
            <col className="w-[23%]" />
            <col className="w-[18%]" />
          </colgroup>
          <thead>
            <tr className="border-ink/20 border-b-[3px] text-[0.65rem] sm:text-sm">
              <th scope="col" className="font-display py-1 pr-2 font-normal tracking-wide">
                Frage
              </th>
              <th scope="col" className="font-display py-1 pr-2 font-normal tracking-wide">
                Geschätzt
              </th>
              <th scope="col" className="font-display py-1 pr-2 font-normal tracking-wide">
                Antwort
              </th>
              <th
                scope="col"
                className="font-display py-1 text-right font-normal tracking-wide whitespace-nowrap"
              >
                Punkte
              </th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry, index) => {
              const question = questions[index];
              if (question === undefined) return null;
              const isBest = extremes?.bestIndex === index;
              const isWorst = extremes?.worstIndex === index;
              const rowClasses = isBest
                ? "bg-[#D8F5DC]"
                : isWorst
                  ? "bg-[#FFE0DA]"
                  : "bg-transparent";

              return (
                <tr
                  key={entry.questionId}
                  data-testid="result-row"
                  data-points={entry.points}
                  className={`border-ink/15 border-b-2 align-top ${rowClasses}`}
                >
                  <th scope="row" className="py-2 pr-2 font-normal break-words hyphens-auto">
                    {shorten(question.question)}
                    {isBest && (
                      <span
                        data-testid="result-best"
                        aria-label="Beste Schätzung dieser Runde"
                        className="font-display border-ink ml-1 inline-block rounded border-2 bg-[#9FE39A] px-1.5 py-0.5 text-[0.65rem] tracking-wide whitespace-nowrap"
                      >
                        Beste
                      </span>
                    )}
                    {isWorst && (
                      <span
                        data-testid="result-worst"
                        aria-label="Schlechteste Schätzung dieser Runde"
                        className="font-display border-ink ml-1 inline-block rounded border-2 bg-[#F2705B] px-1.5 py-0.5 text-[0.65rem] tracking-wide whitespace-nowrap"
                      >
                        Schwächste
                      </span>
                    )}
                  </th>
                  <td className="py-2 pr-2 break-words hyphens-auto tabular-nums">
                    {entry.guess === null
                      ? "Keine Ahnung"
                      : formatWithUnit(entry.guess, question.unit)}
                  </td>
                  <td className="py-2 pr-2 break-words hyphens-auto tabular-nums">
                    {formatWithUnit(question.answer, question.unit)}
                  </td>
                  <td className="font-display py-2 text-right tracking-wide whitespace-nowrap tabular-nums">
                    {entry.points}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      <div className="mt-5 flex flex-wrap items-center gap-3">
        <Button data-testid="copy-result" variant="secondary" onClick={() => void handleCopy()}>
          Ergebnis kopieren
        </Button>
        <Button data-testid="play-again" onClick={onPlayAgain}>
          Nochmal spielen
        </Button>
      </div>

      <div aria-live="polite" className="mt-2 min-h-6">
        {copied && (
          <p data-testid="copy-feedback" className="font-display text-success text-base">
            Kopiert!
          </p>
        )}
      </div>

      {showFallback && (
        <div className="mt-2">
          <label htmlFor="share-text" className="text-ink/70 block text-sm">
            Kopieren geht in diesem Browser nicht automatisch – hier ist der Text:
          </label>
          <input
            id="share-text"
            data-testid="copy-fallback"
            readOnly
            value={shareText}
            className="border-ink text-ink mt-1 w-full rounded-lg border-[3px] bg-white px-2 py-2 text-sm"
          />
        </div>
      )}
    </div>
  );
}
