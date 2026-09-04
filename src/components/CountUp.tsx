"use client";

import { useEffect, useState } from "react";

import { formatWithUnit } from "@/lib/format";
import { useReducedMotionSafe } from "./useReducedMotionSafe";

export interface CountUpProps {
  value: number;
  unit: string;
  durationMs?: number;
  className?: string;
  "data-testid"?: string;
}

/** Kubisches Ausklingen: schnell los, sanft ankommen. */
function easeOutCubic(progress: number): number {
  return 1 - Math.pow(1 - progress, 3);
}

/**
 * Zählt eine Zahl animiert hoch (Abschnitt 3.3).
 *
 * Bewusst mit `requestAnimationFrame` statt mit einer Motion-Value: Der
 * Endwert muss zeichengenau `formatWithUnit(value, unit)` sein, und die
 * Zwischenwerte müssen bei ganzzahligen Antworten ganzzahlig bleiben. Bei
 * `prefers-reduced-motion` steht der Endwert sofort da.
 */
export function CountUp({
  value,
  unit,
  durationMs = 800,
  className,
  "data-testid": testId,
}: CountUpProps) {
  const reducedMotion = useReducedMotionSafe();
  const animates = !reducedMotion && durationMs > 0;
  // Fortschritt der Animation von 0 bis 1. Der Zustand wird ausschließlich aus
  // dem `requestAnimationFrame`-Callback gesetzt, nie synchron im Effekt.
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    if (!animates) return undefined;

    let frame = 0;
    const start = performance.now();

    const step = (now: number): void => {
      const elapsed = Math.min(1, (now - start) / durationMs);
      setProgress(elapsed);
      if (elapsed < 1) frame = requestAnimationFrame(step);
    };

    frame = requestAnimationFrame(step);
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [value, durationMs, animates]);

  // `easeOutCubic(1)` ist exakt 1, der Endwert also exakt `value` – genau das,
  // was `formatWithUnit(value, unit)` erwartet.
  const current = animates ? value * easeOutCubic(progress) : value;
  // Ganzzahlige Antworten zählen auch ganzzahlig hoch, sonst flackern
  // unterwegs Nachkommastellen auf, die es in der Antwort gar nicht gibt.
  const shown = Number.isInteger(value) ? Math.round(current) : current;

  return (
    <span data-testid={testId} className={className}>
      {formatWithUnit(shown, unit)}
    </span>
  );
}
