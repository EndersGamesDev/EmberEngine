/**
 * Gemeinsame Helfer für die End-to-End-Tests.
 *
 * Die Fragen einer Runde werden zufällig gezogen. Damit die Tests trotzdem
 * gezielt einen Volltreffer oder eine Niete erzeugen können, lesen sie den
 * Fragetext aus dem DOM und schlagen die Antwort in `src/data/questions.ts`
 * nach – also in derselben Quelle, aus der auch die App ihre Fragen zieht.
 * Playwright läuft in Node und kann die Datei direkt importieren.
 */
import { expect, type Page } from "@playwright/test";

import { questions } from "../../src/data/questions";
import type { Question } from "../../src/data/schema";
import { formatWithUnit } from "../../src/lib/format";
import type { RoundSize } from "../../src/lib/game-reducer";
import { STORAGE_KEYS } from "../../src/lib/storage";

/** Vergleichsform für Text aus dem DOM: Umbrüche und Mehrfach-Leerraum raus. */
function normalize(text: string): string {
  return text.replace(/\s+/g, " ").trim();
}

/** Die 100 Fragetexte sind laut Datentests eindeutig und taugen als Schlüssel. */
const QUESTIONS_BY_TEXT = new Map(
  questions.map((question) => [normalize(question.question), question] as const),
);

export function questionByText(text: string): Question {
  const question = QUESTIONS_BY_TEXT.get(normalize(text));
  if (question === undefined) {
    throw new Error(`Unbekannter Fragetext im DOM: „${normalize(text)}“`);
  }
  return question;
}

/** Die Frage, die gerade auf dem Bildschirm steht – samt Antwort und Quellen. */
export async function currentQuestion(page: Page): Promise<Question> {
  const text = await page.getByTestId("question-text").textContent();
  if (text === null) {
    throw new Error("Auf dem Bildschirm steht keine Frage.");
  }
  return questionByText(text);
}

/** Die Frage, deren Auflösung gerade zu sehen ist (die Karte wiederholt sie). */
export async function revealedQuestion(page: Page): Promise<Question> {
  const text = await page.getByTestId("reveal-card").locator("p").first().textContent();
  if (text === null) {
    throw new Error("Die Auflösung zeigt keinen Fragetext.");
  }
  return questionByText(text);
}

/**
 * Schreibt eine Zahl so, wie ein Mensch sie eintippen würde.
 *
 * Ganze Zahlen wandern unverändert ins Feld, gebrochene mit Dezimalkomma. Das
 * Komma ist bewusst gewählt: Ein Punkt wäre bei genau drei Nachkommastellen
 * („1.234“) ein Tausenderpunkt und damit mehrdeutig. Alle 100 Antworten liegen
 * zwischen 1,44 und 3,04 Billionen – dort schreibt `String` nie in
 * Exponentialschreibweise; die Prüfung sichert die Annahme trotzdem ab.
 */
export function toInputText(value: number): string {
  const text = String(value);
  if (!/^\d+(\.\d+)?$/.test(text)) {
    throw new Error(`Der Wert ${text} lässt sich nicht als deutsche Zahl eintippen.`);
  }
  return text.replace(".", ",");
}

/** Die richtige Antwort, so wie die Auflösung sie schreibt. */
export function answerText(question: Question): string {
  return formatWithUnit(question.answer, question.unit);
}

/**
 * Legt fest, welche Fragen die nächste Runde ziehen kann.
 *
 * `selectQuestions` bedient sich zuerst bei den noch ungespielten Fragen. Sind
 * genau so viele übrig, wie die Runde braucht, steht die Auswahl damit fest –
 * nur die Reihenfolge bleibt zufällig. Der Merker wird nur gesetzt, wenn noch
 * keiner da ist, damit spätere Navigationen die Runde nicht nachträglich
 * umschreiben.
 */
export async function seedUnplayedQuestions(page: Page, ids: readonly string[]): Promise<void> {
  const keep = new Set(ids);
  const playedIds = questions
    .filter((question) => !keep.has(question.id))
    .map((question) => question.id);

  await page.addInitScript(
    ({ key, value }: { key: string; value: string }) => {
      // Auf `about:blank` gibt es keinen Speicher; das Skript läuft aber auch dort.
      if (!window.location.protocol.startsWith("http")) return;
      if (window.localStorage.getItem(key) !== null) return;
      window.localStorage.setItem(key, value);
    },
    { key: STORAGE_KEYS.playedIds, value: JSON.stringify(playedIds) },
  );
}

/** Öffnet den Startbildschirm. */
export async function openStart(page: Page): Promise<void> {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Verschätz dich", level: 1 })).toBeVisible();
}

/**
 * Wählt die Rundengröße.
 *
 * Der Startbildschirm wird auch auf dem Server gerendert; ein Klick vor der
 * Hydration verpufft wirkungslos. Statt auf ein Zeitfenster zu setzen, wird so
 * lange geklickt, bis React die Auswahl übernommen hat – das ist zugleich der
 * Beweis, dass die Seite jetzt interaktiv ist.
 */
export async function chooseRoundSize(page: Page, size: RoundSize): Promise<void> {
  const button = page.getByTestId(`round-size-${size}`);
  await expect(async () => {
    await button.click();
    await expect(button).toHaveAttribute("aria-pressed", "true", { timeout: 1_000 });
  }).toPass({ timeout: 30_000 });
}

/** Startet eine Runde vom Startbildschirm aus und wartet auf die erste Frage. */
export async function beginRound(page: Page, size: RoundSize): Promise<void> {
  await chooseRoundSize(page, size);
  await page.getByTestId("start-button").click();
  await page.waitForURL(/\/spielen$/);
  await expect(page.getByTestId("question-text")).toBeVisible();
}

/** Startbildschirm öffnen und Runde starten. */
export async function startRound(page: Page, size: RoundSize): Promise<void> {
  await openStart(page);
  await beginRound(page, size);
}

/** Trägt eine Schätzung ein und wartet auf die Auflösung. */
export async function submitGuess(page: Page, value: number): Promise<void> {
  await page.getByTestId("estimate-input").fill(toInputText(value));
  await page.getByTestId("submit-guess").click();
  await expect(page.getByTestId("reveal-card")).toBeVisible();
}

/** Drückt „Keine Ahnung“ und wartet auf die Auflösung. */
export async function skipQuestion(page: Page): Promise<void> {
  await page.getByTestId("skip-button").click();
  await expect(page.getByTestId("reveal-card")).toBeVisible();
}

/** Blättert von der Auflösung weiter. */
export async function goNext(page: Page): Promise<void> {
  await page.getByTestId("next-button").click();
}

/** Liest die Punkte aus dem Bewertungs-Abzeichen der Auflösung. */
export async function readPoints(page: Page): Promise<number> {
  const badge = page.getByTestId("score-points");
  await expect(badge).toBeVisible();
  const text = normalize((await badge.textContent()) ?? "");
  if (!/^\d+ Punkte?$/.test(text)) {
    throw new Error(`Unerwarteter Punktetext: „${text}“`);
  }
  return Number.parseInt(text, 10);
}

/** Spielt die restlichen Fragen einer Runde durch und landet auf `/ergebnis`. */
export async function finishRound(page: Page, remaining: number): Promise<void> {
  for (let step = 0; step < remaining; step += 1) {
    await skipQuestion(page);
    await goNext(page);
  }
  await page.waitForURL(/\/ergebnis$/);
  await expect(page.getByTestId("result-total")).toBeVisible();
}

/** `document.documentElement.scrollWidth` darf den Viewport nie überschreiten. */
export async function expectNoHorizontalScroll(page: Page, viewportWidth: number): Promise<void> {
  const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
  expect(scrollWidth).toBeLessThanOrEqual(viewportWidth);

  // Zusatzprüfung: `body { overflow-x: hidden }` würde echtes Überlaufen des
  // Inhalts verstecken. Der Inhaltsbereich selbst muss deshalb ebenfalls passen.
  const box = await page.locator("main").boundingBox();
  if (box === null) {
    throw new Error("Der Inhaltsbereich <main> ist nicht sichtbar.");
  }
  expect(box.x).toBeGreaterThanOrEqual(0);
  expect(box.x + box.width).toBeLessThanOrEqual(viewportWidth);
}

/** Prüft, dass ein Element ohne Scrollen vollständig im Viewport liegt. */
export async function expectInsideViewport(
  page: Page,
  testId: string,
  viewportHeight: number,
): Promise<void> {
  const scrollY = await page.evaluate(() => window.scrollY);
  expect(scrollY, "Die Seite darf für diese Prüfung nicht gescrollt sein.").toBe(0);

  const box = await page.getByTestId(testId).boundingBox();
  if (box === null) {
    throw new Error(`Element „${testId}“ hat keinen sichtbaren Rahmen.`);
  }
  expect(box.y, `Oberkante von „${testId}“`).toBeGreaterThanOrEqual(0);
  expect(box.y + box.height, `Unterkante von „${testId}“`).toBeLessThanOrEqual(viewportHeight);
}
