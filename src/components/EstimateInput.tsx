"use client";

import { useEffect, useId, useRef, useState, type FormEvent } from "react";

import { formatWithUnit, parseGermanNumber } from "@/lib/format";
import { Button } from "./Button";

export interface EstimateInputProps {
  unit: string;
  /**
   * ID der aktuellen Frage. Der Bildschirm setzt sie zugleich als React-`key`,
   * damit jede neue Frage eine frische Eingabe bekommt; hier steuert sie den
   * Autofokus (Abschnitt 3.2).
   */
  questionId: string;
  onSubmit: (value: number) => void;
  onSkip: () => void;
}

const INVALID_HINT = "Bitte nur Ziffern, Komma oder Punkt";
const EMPTY_HINT = "Tipp eine Zahl ein";

/** Eingabefeld für die Schätzung (Abschnitt 3.2). */
export function EstimateInput({ unit, questionId, onSubmit, onSkip }: EstimateInputProps) {
  const [raw, setRaw] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const inputId = useId();
  const hintId = useId();

  useEffect(() => {
    inputRef.current?.focus();
  }, [questionId]);

  const isEmpty = raw.trim() === "";
  const parsed = isEmpty ? null : parseGermanNumber(raw);
  const isInvalid = !isEmpty && parsed === null;

  function handleSubmit(event: FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    if (parsed === null) return;
    onSubmit(parsed);
  }

  return (
    <form onSubmit={handleSubmit} noValidate className="mt-4">
      <label htmlFor={inputId} className="font-display text-ink block text-base tracking-wide">
        Deine Schätzung
      </label>
      <div className="mt-1 flex items-stretch gap-3">
        <input
          ref={inputRef}
          id={inputId}
          data-testid="estimate-input"
          type="text"
          inputMode="decimal"
          autoComplete="off"
          value={raw}
          onChange={(event) => {
            setRaw(event.target.value);
          }}
          aria-invalid={isInvalid}
          aria-describedby={hintId}
          placeholder="z. B. 1.250.000"
          className="border-ink font-display text-ink placeholder:text-ink/35 min-h-12 w-full min-w-0 rounded-xl border-[3px] bg-white px-3 py-2 text-xl tracking-wide tabular-nums shadow-[inset_3px_3px_0_0_rgba(27,27,27,0.08)] focus-visible:outline-4 focus-visible:outline-offset-2"
        />
        {unit.trim() !== "" && (
          <span className="font-display text-ink/70 flex shrink-0 items-center text-base">
            {unit.trim()}
          </span>
        )}
      </div>

      {/*
        Vorschau und Hinweis teilen sich einen Platz mit fester Mindesthöhe,
        damit beim Tippen nichts springt.
      */}
      <div id={hintId} aria-live="polite" className="mt-1 min-h-6 text-sm leading-6">
        {parsed !== null ? (
          <p data-testid="estimate-preview" className="text-ink font-semibold">
            Deine Schätzung: {formatWithUnit(parsed, unit)}
          </p>
        ) : (
          <p
            data-testid="estimate-hint"
            className={isInvalid ? "text-error font-semibold" : "text-ink/70"}
          >
            {isInvalid ? INVALID_HINT : EMPTY_HINT}
          </p>
        )}
      </div>

      <div className="mt-3 flex flex-wrap gap-3">
        <Button
          type="submit"
          data-testid="submit-guess"
          disabled={parsed === null}
          className="flex-1"
        >
          Schätzen
        </Button>
        <Button
          type="button"
          variant="secondary"
          data-testid="skip-button"
          onClick={onSkip}
          className="flex-1"
        >
          Keine Ahnung
        </Button>
      </div>
    </form>
  );
}
