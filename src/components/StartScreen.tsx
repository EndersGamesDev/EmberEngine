"use client";

import { useRouter } from "next/navigation";
import { useMemo, useState } from "react";

import { CATEGORIES, type Category } from "@/data/schema";
import { DEFAULT_ROUND_SIZE, ROUND_SIZES, type RoundSize } from "@/lib/game-reducer";
import { loadHighscores, loadSettings, type Highscores } from "@/lib/storage";
import { Button } from "./Button";
import { CATEGORY_COLORS } from "./CategoryBadge";
import { useGame } from "./GameProvider";
import { useHydrated } from "./useHydrated";

/** Startbildschirm mit Rundengröße, Kategoriefilter und Bestleistung (Abschnitt 3.1). */
export function StartScreen() {
  const router = useRouter();
  const { startRound } = useGame();

  // Gespeicherte Werte werden erst gelesen, wenn die Hydration durch ist –
  // vorher zeigt der Bildschirm genau das, was auch der Server gerendert hat.
  // Sobald der Nutzer etwas anfasst, gewinnt seine Auswahl („Override“).
  const hydrated = useHydrated();
  const stored = useMemo(() => (hydrated ? loadSettings() : null), [hydrated]);
  const highscores: Highscores = useMemo(() => (hydrated ? loadHighscores() : {}), [hydrated]);

  const [countOverride, setCountOverride] = useState<RoundSize | null>(null);
  const [selectedOverride, setSelectedOverride] = useState<Category[] | null>(null);

  const count = countOverride ?? stored?.count ?? DEFAULT_ROUND_SIZE;
  // Leere Liste heißt „alle Kategorien“ (siehe `select-questions.ts`).
  const storedCategories =
    stored === null || stored.categories.length === 0 ? [...CATEGORIES] : stored.categories;
  const selected = selectedOverride ?? storedCategories;

  const noCategory = selected.length === 0;
  const highscoreEntries = ROUND_SIZES.map((size) => highscores[size]).filter(
    (entry) => entry !== undefined,
  );

  function toggleCategory(category: Category): void {
    setSelectedOverride(
      selected.includes(category)
        ? selected.filter((item) => item !== category)
        : [...selected, category],
    );
  }

  function handleStart(): void {
    if (noCategory) return;
    // Sind alle Kategorien aktiv, wird das als „kein Filter“ gespeichert –
    // dann greifen später ergänzte Kategorien automatisch mit.
    const categories = selected.length === CATEGORIES.length ? [] : selected;
    if (!startRound({ count, categories })) return;
    router.push("/spielen");
  }

  return (
    <main className="mx-auto flex min-h-dvh max-w-[40rem] flex-col justify-center px-4 py-8">
      <header className="text-center">
        <h1 className="font-display text-ink inline-block -rotate-3 text-5xl leading-none tracking-wide drop-shadow-[4px_4px_0_var(--color-card)] sm:text-7xl">
          Verschätz dich
        </h1>
        <p className="font-display text-accent mt-3 text-2xl tracking-wide sm:text-3xl">
          Daneben ist auch drin.
        </p>
        <p className="text-ink mx-auto mt-3 max-w-[28rem] text-base leading-relaxed text-balance">
          Schräge Fragen, eine Zahl als Antwort und ganz viel Bauchgefühl. Je näher deine Schätzung
          dran ist, desto mehr Punkte gibt es – vorbei ist hier ausdrücklich erlaubt.
        </p>
      </header>

      <section aria-labelledby="rundengroesse-titel" className="card mt-6 p-4 sm:p-6">
        <h2 id="rundengroesse-titel" className="font-display text-ink text-lg tracking-wide">
          Wie viele Fragen?
        </h2>
        <div className="mt-2 flex gap-3">
          {ROUND_SIZES.map((size) => {
            const active = size === count;
            return (
              <Button
                key={size}
                data-testid={`round-size-${size}`}
                variant={active ? "primary" : "secondary"}
                aria-pressed={active}
                onClick={() => {
                  setCountOverride(size);
                }}
                className="flex-1"
              >
                {size}
              </Button>
            );
          })}
        </div>

        <h2 id="kategorien-titel" className="font-display text-ink mt-5 text-lg tracking-wide">
          Welche Themen?
        </h2>
        <div
          role="group"
          aria-labelledby="kategorien-titel"
          className="mt-2 flex flex-wrap gap-1.5"
        >
          {CATEGORIES.map((category, index) => {
            const active = selected.includes(category);
            return (
              <button
                key={category}
                type="button"
                data-testid={`category-toggle-${index}`}
                aria-pressed={active}
                onClick={() => {
                  toggleCategory(category);
                }}
                className={`font-display border-ink text-ink cursor-pointer rounded-lg border-[3px] px-2.5 py-1 text-sm tracking-wide transition-[transform,box-shadow,opacity] duration-100 ${
                  active
                    ? "shadow-[3px_3px_0_0_var(--color-ink)]"
                    : "opacity-45 shadow-none saturate-50"
                } active:translate-x-[2px] active:translate-y-[2px] active:shadow-none`}
                style={{ backgroundColor: CATEGORY_COLORS[category] }}
              >
                {category}
              </button>
            );
          })}
        </div>

        <div className="mt-3 flex gap-3">
          <Button
            variant="ghost"
            className="min-h-11 px-2 text-base"
            onClick={() => {
              setSelectedOverride([...CATEGORIES]);
            }}
          >
            Alle
          </Button>
          <Button
            variant="ghost"
            className="min-h-11 px-2 text-base"
            onClick={() => {
              setSelectedOverride([]);
            }}
          >
            Keine
          </Button>
        </div>

        <div aria-live="polite" className="mt-3 min-h-6">
          {noCategory && (
            <p className="text-error text-sm font-semibold">
              Wähle mindestens ein Thema aus, dann geht es los.
            </p>
          )}
        </div>

        <Button
          data-testid="start-button"
          disabled={noCategory}
          onClick={handleStart}
          className="mt-1 w-full text-2xl"
        >
          Los geht&rsquo;s
        </Button>
      </section>

      {highscoreEntries.length > 0 && (
        <section
          data-testid="highscore-section"
          aria-labelledby="bestleistung-titel"
          className="card mt-4 p-4"
        >
          <h2 id="bestleistung-titel" className="font-display text-ink text-lg tracking-wide">
            Deine Bestleistung
          </h2>
          <ul className="mt-1 space-y-0.5">
            {highscoreEntries.map((entry) => (
              <li key={entry.count} className="text-ink text-sm tabular-nums">
                <span className="font-display tracking-wide">{entry.count} Fragen:</span>{" "}
                {entry.points} von {entry.maxPoints} Punkten ({entry.percent} %)
              </li>
            ))}
          </ul>
        </section>
      )}
    </main>
  );
}
