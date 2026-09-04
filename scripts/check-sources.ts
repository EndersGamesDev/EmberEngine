/**
 * Prüft alle Quellen-URLs des Fragenbestands per HTTP.
 *
 * Aufruf: `pnpm check:sources` (läuft über tsx, nicht Teil von `pnpm check`).
 * Erst HEAD, bei Misserfolg GET – manche Server beantworten HEAD gar nicht
 * oder mit 405. Exit-Code 1, sobald mindestens eine Quelle tot ist.
 */
import { questions } from "../src/data/questions";

const TIMEOUT_MS = 15_000;
const CONCURRENCY = 6;

const REQUEST_HEADERS: Record<string, string> = {
  // Wikipedia (und einige andere) antworten ohne eigenen User-Agent mit 403.
  "User-Agent": "verschaetz-dich-source-check/1.0 (+https://github.com/verschaetz-dich)",
  "Accept-Language": "de",
};

interface SourceEntry {
  url: string;
  questionIds: string[];
}

interface Attempt {
  status: number | null;
  ok: boolean;
  error: string | null;
}

interface CheckResult extends SourceEntry {
  status: number | null;
  ok: boolean;
  error: string | null;
  method: "HEAD" | "GET";
}

/** Sammelt jede URL genau einmal und merkt sich, welche Fragen sie benutzen. */
function collectSources(): SourceEntry[] {
  const byUrl = new Map<string, string[]>();
  for (const question of questions) {
    for (const source of question.sources) {
      const ids = byUrl.get(source.url);
      if (ids === undefined) {
        byUrl.set(source.url, [question.id]);
      } else if (!ids.includes(question.id)) {
        ids.push(question.id);
      }
    }
  }
  return [...byUrl].map(([url, questionIds]) => ({ url, questionIds }));
}

async function request(url: string, method: "HEAD" | "GET"): Promise<Attempt> {
  try {
    const response = await fetch(url, {
      method,
      redirect: "follow",
      headers: REQUEST_HEADERS,
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    const status = response.status;
    // Body verwerfen, sonst bleibt die Verbindung unnötig offen. Ein Fehler
    // beim Abbrechen sagt nichts über die Erreichbarkeit der Quelle aus.
    await response.body?.cancel().catch(() => undefined);
    return { status, ok: status >= 200 && status < 400, error: null };
  } catch (error) {
    return {
      status: null,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

async function checkSource(entry: SourceEntry): Promise<CheckResult> {
  const head = await request(entry.url, "HEAD");
  if (head.ok) return { ...entry, ...head, method: "HEAD" };
  const get = await request(entry.url, "GET");
  return { ...entry, ...get, method: "GET" };
}

/** Arbeitet die Liste mit fester Nebenläufigkeit ab und behält die Reihenfolge. */
async function mapWithConcurrency<T, R>(
  items: readonly T[],
  limit: number,
  worker: (item: T) => Promise<R>,
): Promise<R[]> {
  const results: R[] = new Array<R>(items.length);
  let cursor = 0;

  async function runWorker(): Promise<void> {
    for (;;) {
      const index = cursor;
      cursor += 1;
      if (index >= items.length) return;
      const item = items[index];
      if (item === undefined) return;
      results[index] = await worker(item);
    }
  }

  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, () => runWorker()));
  return results;
}

function pad(value: string, width: number): string {
  return value.length >= width ? value : value + " ".repeat(width - value.length);
}

function padStart(value: string, width: number): string {
  return value.length >= width ? value : " ".repeat(width - value.length) + value;
}

function printReport(results: readonly CheckResult[]): void {
  const longestUrl = Math.max(3, ...results.map((result) => result.url.length));
  const urlWidth = Math.min(longestUrl, 90);

  console.log(
    `${padStart("Status", 6)}  ${pad("Ergebnis", 8)}  ${pad("URL", urlWidth)}  Fragen-IDs`,
  );
  console.log(`${"-".repeat(6)}  ${"-".repeat(8)}  ${"-".repeat(urlWidth)}  ${"-".repeat(10)}`);

  for (const result of results) {
    const status = result.status === null ? "—" : String(result.status);
    console.log(
      `${padStart(status, 6)}  ${pad(result.ok ? "OK" : "TOT", 8)}  ${pad(result.url, urlWidth)}  ${result.questionIds.join(", ")}`,
    );
    if (result.error !== null) {
      console.log(`${" ".repeat(16)}↳ ${result.error}`);
    }
  }
}

async function main(): Promise<number> {
  const sources = collectSources();
  if (sources.length === 0) {
    console.log("Keine Quellen gefunden – nichts zu prüfen.");
    return 0;
  }

  console.log(
    `Prüfe ${sources.length} Quellen aus ${questions.length} Fragen (Nebenläufigkeit ${CONCURRENCY}, Timeout ${TIMEOUT_MS / 1000} s) …\n`,
  );

  const results = await mapWithConcurrency(sources, CONCURRENCY, checkSource);
  printReport(results);

  const dead = results.filter((result) => !result.ok);
  console.log(
    `\n${results.length - dead.length} von ${results.length} Quellen erreichbar, ${dead.length} tot.`,
  );

  if (dead.length > 0) {
    console.log("\nTote Quellen:");
    for (const result of dead) {
      console.log(`  ${result.url}  (${result.questionIds.join(", ")})`);
    }
  }

  return dead.length;
}

main()
  .then((deadCount) => {
    process.exitCode = deadCount > 0 ? 1 : 0;
  })
  .catch((error: unknown) => {
    console.error("check-sources ist fehlgeschlagen:", error);
    process.exitCode = 1;
  });
