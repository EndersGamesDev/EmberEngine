/**
 * Was einen Reload überleben muss: laufende Runde, Auflösung und Bestleistung.
 * Dazu die Wächter, die einen Direktaufruf ohne Runde zurück auf den Start
 * schicken.
 */
import { expect, test } from "@playwright/test";

import {
  answerText,
  beginRound,
  currentQuestion,
  goNext,
  readPoints,
  revealedQuestion,
  skipQuestion,
  startRound,
  submitGuess,
} from "./helpers";

const ROUND_SIZE = 5;
const MAX_POINTS = ROUND_SIZE * 10;
const BEST_ROUND_TEXT = `${ROUND_SIZE} Fragen: ${MAX_POINTS} von ${MAX_POINTS} Punkten (100 %)`;

test("Reload mitten in der Runde stellt Frage, Fortschritt und Punktestand wieder her", async ({
  page,
}) => {
  await startRound(page, ROUND_SIZE);

  const first = await currentQuestion(page);
  await submitGuess(page, first.answer);
  await expect(page.getByTestId("score-points")).toHaveText("10 Punkte");
  await goNext(page);

  await expect(page.getByTestId("progress-text")).toHaveText(`Frage 2 von ${ROUND_SIZE}`);
  const second = await currentQuestion(page);

  await page.reload();

  await expect(page.getByTestId("progress-text")).toHaveText(`Frage 2 von ${ROUND_SIZE}`);
  await expect(page.getByTestId("question-text")).toHaveText(second.question);
  await expect(page.getByTestId("estimate-input")).toBeVisible();

  // Der Punktestand der Runde steht erst in der Auflösung – dort muss die 10
  // aus der ersten Frage nach dem Reload noch drinstecken.
  await skipQuestion(page);
  await expect(page.getByTestId("round-score")).toHaveText(
    `Punkte in dieser Runde: 10 von ${MAX_POINTS}`,
  );

  // Zweiter Reload, diesmal in der Auflösung: Die Auflösung bleibt stehen.
  await page.reload();

  await expect(page.getByTestId("reveal-card")).toBeVisible();
  expect((await revealedQuestion(page)).id).toBe(second.id);
  await expect(page.getByTestId("progress-text")).toHaveText(`Frage 2 von ${ROUND_SIZE}`);
  await expect(page.getByTestId("guess-value")).toHaveText("Keine Ahnung");
  await expect(page.getByTestId("answer-value")).toHaveText(answerText(second));
  await expect(page.getByTestId("score-points")).toHaveText("0 Punkte");
  await expect(page.getByTestId("round-score")).toHaveText(
    `Punkte in dieser Runde: 10 von ${MAX_POINTS}`,
  );
});

test("Highscore steht auf dem Start, überlebt den Reload und bleibt bei schwächerer Runde stehen", async ({
  page,
}) => {
  await startRound(page, ROUND_SIZE);

  // Erste Runde: jede Frage exakt getroffen, also 50 von 50.
  let total = 0;
  for (let index = 0; index < ROUND_SIZE; index += 1) {
    const question = await currentQuestion(page);
    await submitGuess(page, question.answer);
    total += await readPoints(page);
    await goNext(page);
  }
  await page.waitForURL(/\/ergebnis$/);
  expect(total).toBe(MAX_POINTS);
  await expect(page.getByTestId("result-total")).toHaveText(
    `${MAX_POINTS} von ${MAX_POINTS} Punkten`,
  );
  await expect(page.getByTestId("new-highscore")).toBeVisible();

  await page.getByTestId("play-again").click();
  await page.waitForURL(/\/$/);

  const highscore = page.getByTestId("highscore-section");
  await expect(highscore).toBeVisible();
  await expect(highscore).toContainText(BEST_ROUND_TEXT);

  await page.reload();
  await expect(highscore).toBeVisible();
  await expect(highscore).toContainText(BEST_ROUND_TEXT);

  // Zweite Runde: alles überspringen, also 0 von 50 – kein neuer Bestwert.
  await beginRound(page, ROUND_SIZE);
  for (let index = 0; index < ROUND_SIZE; index += 1) {
    await skipQuestion(page);
    await goNext(page);
  }
  await page.waitForURL(/\/ergebnis$/);
  await expect(page.getByTestId("result-total")).toHaveText(`0 von ${MAX_POINTS} Punkten`);
  await expect(page.getByTestId("new-highscore")).toHaveCount(0);

  await page.getByTestId("play-again").click();
  await page.waitForURL(/\/$/);
  await expect(page.getByTestId("highscore-section")).toContainText(BEST_ROUND_TEXT);
});

test("Direktaufruf von /ergebnis ohne Runde leitet auf den Start um", async ({ page }) => {
  await page.goto("/ergebnis");

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("start-button")).toBeVisible();
  await expect(page.getByTestId("highscore-section")).toHaveCount(0);
});

test("Direktaufruf von /spielen ohne Runde leitet auf den Start um", async ({ page }) => {
  await page.goto("/spielen");

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByTestId("start-button")).toBeVisible();
});
