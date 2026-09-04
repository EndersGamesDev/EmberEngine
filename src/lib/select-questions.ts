/**
 * Zufällige Fragenauswahl mit Kategoriefilter und Wiederholungsschutz.
 *
 * Reine Logik: kein React, keine Browser-APIs. Die Zufallsquelle ist
 * injizierbar, damit die Auswahl im Test deterministisch ist.
 */
import type { Category, Question } from "../data/schema";

export interface SelectParams {
  pool: Question[];
  count: number;
  /** `undefined` oder leeres Array bedeutet: alle Kategorien. */
  categories?: Category[];
  playedIds: string[];
  random?: () => number;
}

export interface SelectResult {
  questions: Question[];
  playedIds: string[];
  /** `true`, wenn der Pool des Filters für diese Runde zurückgesetzt wurde. */
  poolReset: boolean;
}

/** Fisher-Yates, arbeitet auf einer Kopie und lässt die Eingabe unangetastet. */
function shuffle<T>(items: readonly T[], random: () => number): T[] {
  const result = [...items];
  for (let i = result.length - 1; i > 0; i -= 1) {
    const j = Math.floor(random() * (i + 1));
    const atI = result[i];
    const atJ = result[j];
    // Beide Indizes liegen im Bereich; `noUncheckedIndexedAccess` verlangt die
    // Prüfung trotzdem. Sie fängt zugleich eine kaputte Zufallsquelle ab, die
    // Werte außerhalb von [0, 1) liefert.
    if (atI === undefined || atJ === undefined) continue;
    result[i] = atJ;
    result[j] = atI;
  }
  return result;
}

/** Hängt IDs an, ohne Duplikate zu erzeugen, und behält die Reihenfolge bei. */
function appendUnique(existing: readonly string[], added: readonly string[]): string[] {
  const seen = new Set(existing);
  const result = [...existing];
  for (const id of added) {
    if (seen.has(id)) continue;
    seen.add(id);
    result.push(id);
  }
  return result;
}

/**
 * Zieht die Fragen einer Runde.
 *
 * Zuerst kommen die noch ungespielten Fragen des Filters dran. Reichen sie
 * nicht, gilt der gefilterte Pool als durchgespielt und wird zurückgesetzt
 * (`poolReset === true`): Die restlichen ungespielten Fragen sind trotzdem
 * garantiert dabei, aufgefüllt wird aus den bereits gespielten – nie doppelt.
 * `playedIds` anderer Kategorien bleiben dabei unverändert erhalten.
 */
export function selectQuestions(params: SelectParams): SelectResult {
  const { pool, count, categories, playedIds, random = Math.random } = params;

  const filtered =
    categories === undefined || categories.length === 0
      ? [...pool]
      : pool.filter((question) => categories.includes(question.category));

  const wanted = Number.isFinite(count) ? Math.floor(count) : 0;
  const targetCount = Math.max(0, Math.min(wanted, filtered.length));

  const playedSet = new Set(playedIds);
  const unplayed = filtered.filter((question) => !playedSet.has(question.id));

  if (unplayed.length >= targetCount) {
    const selected = shuffle(unplayed, random).slice(0, targetCount);
    return {
      questions: selected,
      playedIds: appendUnique(
        playedIds,
        selected.map((question) => question.id),
      ),
      poolReset: false,
    };
  }

  // Pool-Reset: Die verbliebenen ungespielten Fragen kommen garantiert in die
  // Runde, der Rest wird aus den bereits gespielten Fragen des Filters
  // aufgefüllt.
  const alreadyPlayed = filtered.filter((question) => playedSet.has(question.id));
  const refill = shuffle(alreadyPlayed, random).slice(0, targetCount - unplayed.length);
  const selected = shuffle([...unplayed, ...refill], random);

  const filteredIds = new Set(filtered.map((question) => question.id));
  const foreignIds = playedIds.filter((id) => !filteredIds.has(id));

  return {
    questions: selected,
    playedIds: appendUnique(
      foreignIds,
      selected.map((question) => question.id),
    ),
    poolReset: true,
  };
}
