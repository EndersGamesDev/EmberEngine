/**
 * Screenshots aller vier Bildschirme nach `screenshots/` (Abschnitt 8.4).
 *
 * Läuft in beiden Projekten; der Projektname landet im Dateinamen. Damit die
 * Bilder von Lauf zu Lauf vergleichbar bleiben, zieht die Runde aus einem fest
 * gesetzten Fünferpack Fragen – nur ihre Reihenfolge bleibt zufällig.
 */
import path from "node:path";

import { expect, test } from "@playwright/test";

import {
  answerText,
  beginRound,
  currentQuestion,
  finishRound,
  goNext,
  openStart,
  seedUnplayedQuestions,
  submitGuess,
} from "./helpers";

const ROUND_SIZE = 5;

/** Fünf Fragen aus fünf Kategorien, von „3 Herzen“ bis „65.536 Byte“. */
const SCREENSHOT_QUESTION_IDS = ["q001", "q003", "q029", "q046", "q051"];

/** Count-up dauert 800 ms; danach steht die Zahl still. */
const COUNT_UP_MS = 900;
/** Karten- und Abzeichen-Animation sind nach 360 ms durch. */
const CARD_ANIMATION_MS = 400;

test("Screenshots von Start, Frage, Auflösung und Ergebnis", async ({ page }, testInfo) => {
  const suffix = testInfo.project.name;
  // `testInfo.config.rootDir` zeigt auf `testDir`, nicht auf das Projekt –
  // deshalb zwei Ebenen hoch von dieser Datei aus.
  const shotDir = path.resolve(__dirname, "..", "..", "screenshots");
  const shot = (name: string): string => path.join(shotDir, `${name}-${suffix}.png`);
  const shotFull = (name: string): string => path.join(shotDir, `${name}-${suffix}-full.png`);

  await seedUnplayedQuestions(page, SCREENSHOT_QUESTION_IDS);

  // --- Start ---------------------------------------------------------------
  await openStart(page);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(CARD_ANIMATION_MS);
  await page.screenshot({ path: shot("start"), fullPage: false });

  // --- Frage ---------------------------------------------------------------
  await beginRound(page, ROUND_SIZE);
  const question = await currentQuestion(page);
  await expect(page.getByTestId("estimate-hint")).toHaveText("Tipp eine Zahl ein");
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(CARD_ANIMATION_MS);
  // Der Viewport zählt: Auf dem Handy muss alles ohne Scrollen zu sehen sein.
  await page.screenshot({ path: shot("frage"), fullPage: false });

  // --- Auflösung (Volltreffer, also mit Konfetti) ---------------------------
  await submitGuess(page, question.answer);
  await expect(page.getByTestId("score-label")).toHaveText("Volltreffer!");
  // Konfetti lebt 2,5 s – beide Aufnahmen müssen in dieses Fenster passen.
  await page.waitForTimeout(COUNT_UP_MS);
  await expect(page.getByTestId("answer-value")).toHaveText(answerText(question));
  await page.screenshot({ path: shot("aufloesung"), fullPage: false });
  await page.screenshot({ path: shotFull("aufloesung"), fullPage: true });

  // --- Ergebnis ------------------------------------------------------------
  await goNext(page);
  await finishRound(page, ROUND_SIZE - 1);
  await expect(page.getByTestId("result-row")).toHaveCount(ROUND_SIZE);
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(CARD_ANIMATION_MS);
  await page.screenshot({ path: shot("ergebnis"), fullPage: false });
  await page.screenshot({ path: shotFull("ergebnis"), fullPage: true });
});
