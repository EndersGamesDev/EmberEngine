import type { ScoreLabel } from "@/lib/scoring";

/**
 * Farbe je Bewertungsstufe. Von Grün bis Rot, alle Töne hell genug, damit die
 * ink-farbene Beschriftung darauf deutlich über 4,5:1 liegt (WCAG AA).
 */
const LABEL_COLORS: Record<ScoreLabel, string> = {
  "Volltreffer!": "#57D175",
  "Fast perfekt": "#A9E36B",
  "Knapp daneben": "#FFD84D",
  "Nicht schlecht": "#FFA94D",
  "Naja …": "#ED7A21",
  "Voll verschätzt!": "#F2705B",
};

export interface ScoreBadgeProps {
  label: ScoreLabel;
  points: number;
}

/** Bewertung der Schätzung: Label, Punkte, Farbe (Abschnitt 3.3). */
export function ScoreBadge({ label, points }: ScoreBadgeProps) {
  return (
    <div
      className="animate-badge-pop border-ink text-ink inline-flex flex-col items-center gap-0.5 rounded-2xl border-[3px] px-5 py-1.5 shadow-[5px_5px_0_0_var(--color-ink)] sm:py-2"
      style={{ backgroundColor: LABEL_COLORS[label] }}
    >
      <span
        data-testid="score-label"
        className="font-display text-xl leading-tight tracking-wide sm:text-2xl"
      >
        {label}
      </span>
      <span data-testid="score-points" className="font-display text-base tracking-wide">
        {points} {points === 1 ? "Punkt" : "Punkte"}
      </span>
    </div>
  );
}
