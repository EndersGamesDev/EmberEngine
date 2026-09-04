import type { Source } from "@/data/schema";

export interface SourceListProps {
  sources: Source[];
}

/** Quellenliste der Auflösung; jeder Link öffnet einen neuen Tab (Abschnitt 3.3). */
export function SourceList({ sources }: SourceListProps) {
  if (sources.length === 0) return null;

  return (
    <section aria-labelledby="quellen-titel" className="mt-3">
      <h3 id="quellen-titel" className="font-display text-ink/70 text-xs tracking-widest uppercase">
        Quellen
      </h3>
      <ul className="mt-1 space-y-1">
        {sources.map((source) => (
          <li key={source.url} className="text-sm leading-snug">
            <a
              data-testid="source-link"
              href={source.url}
              target="_blank"
              rel="noopener noreferrer"
              className="text-ink decoration-accent hover:decoration-ink underline decoration-2 underline-offset-2"
            >
              {source.title}
            </a>
          </li>
        ))}
      </ul>
    </section>
  );
}
