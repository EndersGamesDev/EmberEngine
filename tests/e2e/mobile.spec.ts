/**
 * Layoutprüfung auf 375 × 667 (Abschnitte 6.3 und 8.4).
 *
 * Damit die Prüfung nicht vom Zufall abhängt, bekommt die Runde die fünf
 * längsten Fragen des Datenbestands – der ungünstigste Fall für „ohne Scrollen
 * sichtbar“.
 */
import { expect, test } from "@playwright/test";

import { questions } from "../../src/data/questions";
import {
  beginRound,
  currentQuestion,
  expectInsideViewport,
  expectNoHorizontalScroll,
  finishRound,
  goNext,
  openStart,
  seedUnplayedQuestions,
  submitGuess,
} from "./helpers";

const VIEWPORT_WIDTH = 375;
const VIEWPORT_HEIGHT = 667;
const ROUND_SIZE = 5;

/** Die fünf längsten Fragetexte – mehr Zeilen kann eine Fragekarte nicht bekommen. */
const LONGEST_QUESTION_IDS = [...questions]
  .sort((left, right) => right.question.length - left.question.length)
  .slice(0, ROUND_SIZE)
  .map((question) => question.id);

test("Mobile 375×667: kein Querscrollen, Frage und Eingabe ohne Scrollen sichtbar", async ({
  page,
}, testInfo) => {
  test.skip(testInfo.project.name !== "mobile", "Gilt nur für den Mobile-Viewport.");

  await seedUnplayedQuestions(page, LONGEST_QUESTION_IDS);

  // Startbildschirm
  await openStart(page);
  await expectNoHorizontalScroll(page, VIEWPORT_WIDTH);

  // Fragebildschirm: der eigentliche Prüfpunkt aus Abschnitt 6.3.
  await beginRound(page, ROUND_SIZE);
  await expectNoHorizontalScroll(page, VIEWPORT_WIDTH);
  for (const testId of ["question-text", "estimate-input", "submit-guess", "skip-button"]) {
    await expectInsideViewport(page, testId, VIEWPORT_HEIGHT);
  }

  // Auflösung
  const question = await currentQuestion(page);
  await submitGuess(page, question.answer);
  await expectNoHorizontalScroll(page, VIEWPORT_WIDTH);

  // Ergebnis (die Tabelle ist der breiteste Inhalt der App)
  await goNext(page);
  await finishRound(page, ROUND_SIZE - 1);
  await expectNoHorizontalScroll(page, VIEWPORT_WIDTH);
  await expect(page.getByTestId("result-row")).toHaveCount(ROUND_SIZE);
});
