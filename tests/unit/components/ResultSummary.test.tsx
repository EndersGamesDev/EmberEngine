import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ResultSummary } from "@/components/ResultSummary";
import type { Question } from "@/data/schema";
import type { RoundEntry } from "@/lib/game-reducer";
import type { ScoreLabel } from "@/lib/scoring";
import { makeQuestions } from "../../fixtures/questions";

const questions: Question[] = makeQuestions(3);

function entry(index: number, points: number, label: ScoreLabel, guess: number | null): RoundEntry {
  const question = questions[index];
  return {
    questionId: question?.id ?? `q${String(index + 1).padStart(3, "0")}`,
    guess,
    points,
    label,
    ratio: guess === null ? Number.POSITIVE_INFINITY : 1.2,
  };
}

const entries: RoundEntry[] = [
  entry(0, 5, "Knapp daneben", 12),
  entry(1, 10, "Volltreffer!", 20),
  entry(2, 0, "Voll verschätzt!", null),
];

interface RenderOptions {
  entries?: RoundEntry[];
  newHighscore?: boolean;
}

function renderSummary(options: RenderOptions = {}) {
  const usedEntries = options.entries ?? entries;
  const onPlayAgain = vi.fn();
  render(
    <ResultSummary
      questions={questions}
      entries={usedEntries}
      totalPoints={usedEntries.reduce((sum, item) => sum + item.points, 0)}
      maxPoints={usedEntries.length * 10}
      newHighscore={options.newHighscore ?? false}
      onPlayAgain={onPlayAgain}
    />,
  );
  return { onPlayAgain };
}

/** Ersetzt `navigator.clipboard`, das jsdom nicht mitbringt. */
function stubClipboard(writeText: () => Promise<void>): void {
  Object.defineProperty(navigator, "clipboard", {
    value: { writeText },
    configurable: true,
  });
}

afterEach(() => {
  Reflect.deleteProperty(navigator, "clipboard");
});

describe("ResultSummary", () => {
  it("zeigt Gesamtpunkte, Prozentwert und einen Spruch", () => {
    renderSummary();

    expect(screen.getByTestId("result-total")).toHaveTextContent("15 von 30 Punkten");
    expect(screen.getByTestId("result-percent")).toHaveTextContent("50 %");
    expect(screen.getByTestId("result-saying").textContent?.length ?? 0).toBeGreaterThan(10);
  });

  it("zeigt eine Zeile pro Frage mit Schätzung, Antwort und Punkten", () => {
    renderSummary();

    const rows = screen.getAllByTestId("result-row");
    expect(rows).toHaveLength(questions.length);
    expect(rows[0]).toHaveAttribute("data-points", "5");
    expect(rows[1]).toHaveAttribute("data-points", "10");
    expect(rows[2]).toHaveAttribute("data-points", "0");

    expect(rows[0]).toHaveTextContent("12 Stück");
    expect(rows[0]).toHaveTextContent("10 Stück");
    expect(rows[2]).toHaveTextContent("Keine Ahnung");
  });

  it("kürzt lange Fragen in der Tabelle", () => {
    const longQuestions = makeQuestions(1).map((question) => ({
      ...question,
      question: `${"Wie viele Beispielwerte stecken in dieser besonders langen Testfrage".padEnd(150, "x")}?`,
    }));
    const onPlayAgain = vi.fn();
    render(
      <ResultSummary
        questions={longQuestions}
        entries={[entry(0, 3, "Nicht schlecht", 8)]}
        totalPoints={3}
        maxPoints={10}
        newHighscore={false}
        onPlayAgain={onPlayAgain}
      />,
    );

    const row = screen.getByTestId("result-row");
    expect(row.textContent).toContain("…");
    expect(row.textContent?.length ?? 0).toBeLessThan(120);
  });

  it("markiert beste und schlechteste Schätzung", () => {
    renderSummary();

    const rows = screen.getAllByTestId("result-row");
    expect(rows[1]).toContainElement(screen.getByTestId("result-best"));
    expect(rows[2]).toContainElement(screen.getByTestId("result-worst"));
  });

  it("markiert bei durchgehend gleichen Punkten nur die beste Zeile", () => {
    renderSummary({
      entries: [
        entry(0, 5, "Knapp daneben", 12),
        entry(1, 5, "Knapp daneben", 24),
        entry(2, 5, "Knapp daneben", 36),
      ],
    });

    const rows = screen.getAllByTestId("result-row");
    expect(rows[0]).toContainElement(screen.getByTestId("result-best"));
    expect(screen.queryByTestId("result-worst")).not.toBeInTheDocument();
  });

  it("zeigt den Highscore-Hinweis nur bei einem neuen Bestwert", () => {
    const { unmount } = render(
      <ResultSummary
        questions={questions}
        entries={entries}
        totalPoints={15}
        maxPoints={30}
        newHighscore={false}
        onPlayAgain={vi.fn()}
      />,
    );
    expect(screen.queryByTestId("new-highscore")).not.toBeInTheDocument();
    unmount();

    renderSummary({ newHighscore: true });
    expect(screen.getByTestId("new-highscore")).toHaveTextContent("Neuer Highscore!");
  });

  it("kopiert den Ergebnistext in die Zwischenablage und bestätigt kurz", async () => {
    const writeText = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
    const user = userEvent.setup();
    renderSummary();
    stubClipboard(writeText);

    await user.click(screen.getByTestId("copy-result"));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledExactlyOnceWith(
        "Ich habe bei Verschätz dich 15 von 30 Punkten geholt (50 %). Schaffst du mehr?",
      );
    });
    expect(await screen.findByTestId("copy-feedback")).toHaveTextContent("Kopiert!");
  });

  it("zeigt den Text zum Selbstkopieren, wenn die Zwischenablage fehlt", async () => {
    const user = userEvent.setup();
    renderSummary();
    Reflect.deleteProperty(navigator, "clipboard");

    await user.click(screen.getByTestId("copy-result"));

    expect(await screen.findByTestId("copy-fallback")).toHaveValue(
      "Ich habe bei Verschätz dich 15 von 30 Punkten geholt (50 %). Schaffst du mehr?",
    );
  });

  it("ruft „Nochmal spielen“ auf", async () => {
    const user = userEvent.setup();
    const { onPlayAgain } = renderSummary();

    await user.click(screen.getByTestId("play-again"));

    expect(onPlayAgain).toHaveBeenCalledTimes(1);
  });
});
