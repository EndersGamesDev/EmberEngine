/**
 * Persistenz in `localStorage` und `sessionStorage`.
 *
 * Die Modul-Funktionen sind SSR-sicher: Ohne `window` liefern Lesefunktionen
 * Standardwerte, Schreibfunktionen tun nichts, und nichts wirft.
 */
import { z } from "zod";

import { categorySchema, questionsSchema } from "../data/schema";
import {
  DEFAULT_ROUND_SIZE,
  type GameSettings,
  type GameState,
  type RoundSize,
} from "./game-reducer";
import { SCORE_LABELS } from "./scoring";

const KEY_PREFIX = "verschaetz-dich:";

export const STORAGE_KEYS = {
  highscores: `${KEY_PREFIX}highscores`,
  playedIds: `${KEY_PREFIX}played-ids`,
  settings: `${KEY_PREFIX}settings`,
  session: `${KEY_PREFIX}session`,
} as const;

export interface Highscore {
  points: number;
  maxPoints: number;
  percent: number;
  count: RoundSize;
  /** ISO-8601-Zeitstempel. */
  achievedAt: string;
}

export type Highscores = Partial<Record<RoundSize, Highscore>>;

// --- Zugriff auf den Speicher ------------------------------------------------

type StorageKind = "local" | "session";

/**
 * Holt den Speicher – oder `null`, wenn er nicht nutzbar ist.
 *
 * Der `try/catch` ist hier bewusst gesetzt und nicht als Fehler-Versteck
 * gedacht: Auf dem Server gibt es kein `window`, und Safari im Privatmodus
 * sowie Browser mit blockierten Website-Daten werfen schon beim reinen Zugriff
 * auf `window.localStorage`. Ein gespeicherter Spielstand ist ein Komfort, kein
 * kritischer Wert – die App muss ohne ihn weiterlaufen.
 */
function getStorage(kind: StorageKind): Storage | null {
  if (typeof window === "undefined") return null;
  try {
    return kind === "local" ? window.localStorage : window.sessionStorage;
  } catch {
    return null;
  }
}

function readRaw(kind: StorageKind, key: string): string | null {
  const storage = getStorage(kind);
  if (storage === null) return null;
  try {
    return storage.getItem(key);
  } catch {
    return null;
  }
}

function writeRaw(kind: StorageKind, key: string, value: string): void {
  const storage = getStorage(kind);
  if (storage === null) return;
  try {
    storage.setItem(key, value);
  } catch {
    // Quota überschritten oder Speicher gesperrt: siehe Begründung an `getStorage`.
  }
}

function removeRaw(kind: StorageKind, key: string): void {
  const storage = getStorage(kind);
  if (storage === null) return;
  try {
    storage.removeItem(key);
  } catch {
    // siehe Begründung an `getStorage`
  }
}

/** Liest und parst JSON; ungültiges JSON zählt wie „nichts gespeichert“. */
function readJson(kind: StorageKind, key: string): unknown {
  const raw = readRaw(kind, key);
  if (raw === null) return undefined;
  try {
    return JSON.parse(raw) as unknown;
  } catch {
    return undefined;
  }
}

function writeJson(kind: StorageKind, key: string, value: unknown): void {
  writeRaw(kind, key, JSON.stringify(value));
}

// --- Schemata ----------------------------------------------------------------

/** Spiegelt `ROUND_SIZES` aus `game-reducer.ts`. */
const roundSizeSchema = z.union([z.literal(5), z.literal(10), z.literal(20)]);

const highscoreSchema = z.object({
  points: z.number(),
  maxPoints: z.number(),
  percent: z.number(),
  count: roundSizeSchema,
  achievedAt: z.string(),
});

const storedHighscoresSchema = z.record(z.string(), highscoreSchema);

const gameSettingsSchema = z.object({
  count: roundSizeSchema,
  categories: z.array(categorySchema),
});

/**
 * `ratio` kann `Infinity` sein (ungültige Schätzung). JSON kennt kein
 * Infinity – `JSON.stringify` macht daraus `null`. Beim Lesen wird `null`
 * deshalb wieder zu `Infinity`, sonst ginge nach einem Reload jede
 * übersprungene Frage verloren.
 */
const ratioSchema = z
  .union([z.number(), z.null()])
  .transform((value) => value ?? Number.POSITIVE_INFINITY);

const roundEntrySchema = z.object({
  questionId: z.string(),
  guess: z.union([z.number(), z.null()]),
  points: z.number(),
  label: z.enum(SCORE_LABELS),
  ratio: ratioSchema,
});

const gameStateSchema = z.object({
  phase: z.enum(["idle", "question", "reveal", "finished"]),
  settings: gameSettingsSchema,
  questions: questionsSchema,
  index: z.number().int().min(0),
  entries: z.array(roundEntrySchema),
  totalPoints: z.number(),
  maxPoints: z.number(),
});

const playedIdsSchema = z.array(z.string());

// --- Highscores --------------------------------------------------------------

/**
 * Highscores liegen je Rundengröße getrennt vor: 40 von 50 und 63 von 100 sind
 * sonst nicht vergleichbar.
 */
export function loadHighscores(): Highscores {
  const parsed = storedHighscoresSchema.safeParse(readJson("local", STORAGE_KEYS.highscores));
  if (!parsed.success) return {};

  const result: Highscores = {};
  for (const entry of Object.values(parsed.data)) {
    // Nach dem Eintrag selbst indizieren, nicht nach dem JSON-Schlüssel: Der
    // Schlüssel ist im JSON ein String, `count` ist typsicher eine RoundSize.
    result[entry.count] = entry;
  }
  return result;
}

/** Speichert nur, wenn der Eintrag die bisherige Bestleistung übertrifft. */
export function saveHighscore(entry: Highscore): void {
  const highscores = loadHighscores();
  if (!isNewHighscore(highscores, entry.points, entry.count)) return;

  const next: Highscores = { ...highscores };
  next[entry.count] = entry;
  writeJson("local", STORAGE_KEYS.highscores, next);
}

/** Pur: keine Speicherzugriffe, damit die UI das Ergebnis direkt bewerten kann. */
export function isNewHighscore(highscores: Highscores, points: number, count: RoundSize): boolean {
  const current = highscores[count];
  return current === undefined || points > current.points;
}

// --- Gespielte Fragen --------------------------------------------------------

export function loadPlayedIds(): string[] {
  const parsed = playedIdsSchema.safeParse(readJson("local", STORAGE_KEYS.playedIds));
  return parsed.success ? parsed.data : [];
}

export function savePlayedIds(ids: string[]): void {
  writeJson("local", STORAGE_KEYS.playedIds, ids);
}

// --- Laufende Runde ----------------------------------------------------------

/**
 * Stellt eine laufende Runde wieder her. Die Struktur wird komplett validiert
 * (inklusive der Fragen gegen `questionSchema`); alles Ungültige gilt als
 * „keine Runde“.
 */
export function loadGameSession(): GameState | null {
  const parsed = gameStateSchema.safeParse(readJson("session", STORAGE_KEYS.session));
  return parsed.success ? parsed.data : null;
}

export function saveGameSession(state: GameState): void {
  writeJson("session", STORAGE_KEYS.session, state);
}

export function clearGameSession(): void {
  removeRaw("session", STORAGE_KEYS.session);
}

// --- Einstellungen -----------------------------------------------------------

export function loadSettings(): GameSettings {
  const parsed = gameSettingsSchema.safeParse(readJson("local", STORAGE_KEYS.settings));
  return parsed.success ? parsed.data : { count: DEFAULT_ROUND_SIZE, categories: [] };
}

export function saveSettings(settings: GameSettings): void {
  writeJson("local", STORAGE_KEYS.settings, settings);
}
