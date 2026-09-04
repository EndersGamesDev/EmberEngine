export interface ProgressBarProps {
  /** 1-basiert: die wievielte Frage gerade dran ist. */
  current: number;
  total: number;
}

/** „Frage 3 von 10“ plus Balken (Abschnitt 3.2). */
export function ProgressBar({ current, total }: ProgressBarProps) {
  const safeTotal = Math.max(1, total);
  const percent = Math.min(100, Math.max(0, (current / safeTotal) * 100));

  return (
    <div className="w-full">
      <p data-testid="progress-text" className="font-display text-ink text-base tracking-wide">
        Frage {current} von {total}
      </p>
      <div
        role="progressbar"
        aria-label="Fortschritt in dieser Runde"
        aria-valuenow={current}
        aria-valuemin={1}
        aria-valuemax={safeTotal}
        aria-valuetext={`Frage ${current} von ${total}`}
        className="border-ink bg-card mt-1.5 h-3 w-full overflow-hidden rounded-full border-[3px]"
      >
        <div
          className="bg-accent h-full rounded-r-full transition-[width] duration-300"
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}
