/**
 * Der Weg durch eine Runde: schätzen, auflösen, weiterblättern, Ergebnis.
 */
import { expect, test } from "@playwright/test";

import {
  answerText,
  currentQuestion,
  goNext,
  readPoints,
  skipQuestion,
  startRound,
  submitGuess,
  toInputText,
} from "./helpers";

const ROUND_SIZE = 5;
const MAX_POINTS = ROUND_SIZE * 10;

/**
 * Fünf Schätzungen, die je eine andere Bewertungsstufe treffen.
 *
 * Der Faktor bezieht sich auf die richtige Antwort, die der Test aus den
 * Fragedaten nachschlägt. So entsteht in jeder Runde derselbe Punktestand,
 * obwohl die Fragen selbst zufällig gezogen werden.
 */
const PLAN = [
  { factor: 1, label: "Volltreffer!", points: 10 },
  { factor: 1000, label: "Voll verschätzt!", points: 0 },
  { factor: 1.1, label: "Fast perfekt", points: 7 },
  { factor: 1.25, label: "Knapp daneben", points: 5 },
  { factor: 1.4, label: "Nicht schlecht", points: 3 },
] as const;

test("komplette Runde mit fünf Fragen: Einzelpunkte ergeben das Endergebnis", async ({ page }) => {
  await startRound(page, ROUND_SIZE);

  const collected: number[] = [];

  for (const [index, step] of PLAN.entries()) {
    const position = index + 1;
    await expect(page.getByTestId("progress-text")).toHaveText(
      `Frage ${position} von ${ROUND_SIZE}`,
    );
    await expect(page.getByTestId("category-badge")).toBeVisible();
    await expect(page.getByTestId("question-text")).toBeVisible();

    const question = await currentQuestion(page);
    await submitGuess(page, question.answer * step.factor);

    await expect(page.getByTestId("answer-value")).toHaveText(answerText(question));
    await expect(page.getByTestId("score-label")).toHaveText(step.label);
    const points = await readPoints(page);
    expect(points, `Punkte für Frage ${position}`).toBe(step.points);
    collected.push(points);

    const running = collected.reduce((sum, value) => sum + value, 0);
    await expect(page.getByTestId("round-score")).toHaveText(
      `Punkte in dieser Runde: ${running} von ${MAX_POINTS}`,
    );
    await expect(page.getByTestId("next-button")).toHaveText(
      position === ROUND_SIZE ? "Zum Ergebnis" : "Weiter",
    );

    await goNext(page);
  }

  await page.waitForURL(/\/ergebnis$/);

  const total = collected.reduce((sum, value) => sum + value, 0);
  await expect(page.getByTestId("result-total")).toHaveText(`${total} von ${MAX_POINTS} Punkten`);
  await expect(page.getByTestId("result-percent")).toHaveText(
    `${Math.round((total / MAX_POINTS) * 100)} %`,
  );
  await expect(page.getByTestId("result-saying")).not.toBeEmpty();

  const rows = page.getByTestId("result-row");
  await expect(rows).toHaveCount(ROUND_SIZE);
  const rowPoints = await rows.evaluateAll((elements) =>
    elements.map((element) => element.getAttribute("data-points")),
  );
  expect(rowPoints).toEqual(collected.map(String));

  // Beste und schlechteste Schätzung sind markiert – 10 und 0 liegen im Plan.
  await expect(page.getByTestId("result-best")).toBeVisible();
  await expect(page.getByTestId("result-worst")).toBeVisible();
});

test("„Keine Ahnung“ gibt null Punkte und zeigt trotzdem Erklärung und Quellen", async ({
  page,
}) => {
  await startRound(page, ROUND_SIZE);

  const question = await currentQuestion(page);
  await skipQuestion(page);

  await expect(page.getByTestId("guess-value")).toHaveText("Keine Ahnung");
  await expect(page.getByTestId("answer-value")).toHaveText(answerText(question));
  await expect(page.getByTestId("score-points")).toHaveText("0 Punkte");
  await expect(page.getByTestId("score-label")).toHaveText("Voll verschätzt!");
  await expect(page.getByTestId("round-score")).toHaveText(
    `Punkte in dieser Runde: 0 von ${MAX_POINTS}`,
  );

  const explanation = page.getByTestId("explanation");
  await expect(explanation).not.toBeEmpty();
  await expect(explanation).toHaveText(question.explanation);

  const links = page.getByTestId("source-link");
  await expect(links).toHaveCount(question.sources.length);
  expect(question.sources.length).toBeGreaterThan(0);
  for (let index = 0; index < question.sources.length; index += 1) {
    const link = links.nth(index);
    await expect(link).toHaveAttribute("target", "_blank");
    await expect(link).toHaveAttribute("rel", /noopener/);
    await expect(link).toHaveAttribute("href", /^https:\/\//);
  }
});

test("Tastatur: Enter schätzt ab, Enter blättert weiter, Fokus landet im Eingabefeld", async ({
  page,
}) => {
  await startRound(page, ROUND_SIZE);

  const input = page.getByTestId("estimate-input");
  await expect(input, "Autofokus auf dem Eingabefeld").toBeFocused();

  // Tab-Reihenfolge innerhalb der Fragekarte: Feld → Schätzen → Keine Ahnung.
  await input.pressSequentially("100");
  await input.press("Tab");
  await expect(page.getByTestId("submit-guess")).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(page.getByTestId("skip-button")).toBeFocused();

  const question = await currentQuestion(page);
  await input.fill("");
  await input.pressSequentially(toInputText(question.answer));
  await expect(page.getByTestId("estimate-preview")).toHaveText(
    `Deine Schätzung: ${answerText(question)}`,
  );

  await input.press("Enter");
  await expect(page.getByTestId("reveal-card")).toBeVisible();
  await expect(page.getByTestId("score-label")).toHaveText("Volltreffer!");
  await expect(page.getByTestId("next-button"), "Fokus für Enter auf „Weiter“").toBeFocused();

  await page.keyboard.press("Enter");
  await expect(page.getByTestId("progress-text")).toHaveText(`Frage 2 von ${ROUND_SIZE}`);
  await expect(page.getByTestId("estimate-input"), "Autofokus bei der neuen Frage").toBeFocused();
  await expect(page.getByTestId("estimate-input")).toHaveValue("");
});
