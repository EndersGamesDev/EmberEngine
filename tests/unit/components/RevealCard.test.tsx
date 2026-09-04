import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { RevealCard } from "@/components/RevealCard";
import type { Question } from "@/data/schema";
import type { RoundEntry } from "@/lib/game-reducer";
import { setReducedMotion } from "../../support/reduced-motion";
import { makeQuestion } from "../../fixtures/questions";

const question: Question = makeQuestion({
  id: "q042",
  category: "Weltraum & Wissenschaft",
  question: "Wie weit ist der Mond im Mittel von der Erde entfernt?",
  answer: 384400,
  unit: "km",
  sources: [
    { title: "Wikipedia: Mond", url: "https://de.wikipedia.org/wiki/Mond" },
    { title: "NASA: Moon", url: "https://science.nasa.gov/moon/" },
  ],
});

function entryFor(overrides: Partial<RoundEntry> = {}): RoundEntry {
  return {
    questionId: question.id,
    guess: 350000,
    points: 5,
    label: "Knapp daneben",
    ratio: 1.098,
    ...overrides,
  };
}

interface RenderOptions {
  entry?: RoundEntry;
  isLast?: boolean;
}

function renderCard(options: RenderOptions = {}) {
  const onNext = vi.fn();
  render(
    <RevealCard
      question={question}
      entry={options.entry ?? entryFor()}
      totalPoints={17}
      maxPoints={50}
      isLast={options.isLast ?? false}
      onNext={onNext}
    />,
  );
  return { onNext };
}

describe("RevealCard", () => {
  it("zeigt Punkte, Label und Erklärung", () => {
    setReducedMotion(true);
    renderCard();

    expect(screen.getByTestId("score-label")).toHaveTextContent("Knapp daneben");
    expect(screen.getByTestId("score-points")).toHaveTextContent("5 Punkte");
    expect(screen.getByTestId("explanation")).toHaveTextContent(question.explanation);
    expect(screen.getByTestId("round-score")).toHaveTextContent(
      "Punkte in dieser Runde: 17 von 50",
    );
  });

  it("stellt Schätzung und Antwort formatiert gegenüber", () => {
    setReducedMotion(true);
    renderCard();

    expect(screen.getByTestId("guess-value")).toHaveTextContent("350.000 km");
    expect(screen.getByTestId("answer-value")).toHaveTextContent("384.400 km");
  });

  it("zeigt bei reduzierter Bewegung den Endwert sofort", () => {
    setReducedMotion(true);
    renderCard();

    expect(screen.getByTestId("answer-value")).toHaveTextContent("384.400 km");
  });

  it("erreicht nach dem Count-up exakt den Endwert", async () => {
    setReducedMotion(false);
    renderCard();

    await waitFor(
      () => {
        expect(screen.getByTestId("answer-value")).toHaveTextContent("384.400 km");
      },
      { timeout: 4000 },
    );
  });

  it("verlinkt alle Quellen in einem neuen Tab", () => {
    setReducedMotion(true);
    renderCard();

    const links = screen.getAllByTestId("source-link");
    expect(links).toHaveLength(question.sources.length);
    links.forEach((link, index) => {
      expect(link).toHaveTextContent(question.sources[index]?.title ?? "");
      expect(link).toHaveAttribute("href", question.sources[index]?.url ?? "");
      expect(link).toHaveAttribute("target", "_blank");
      expect(link).toHaveAttribute("rel", "noopener noreferrer");
    });
  });

  it("zeigt „Keine Ahnung“ statt einer Schätzung", () => {
    setReducedMotion(true);
    renderCard({
      entry: entryFor({
        guess: null,
        points: 0,
        label: "Voll verschätzt!",
        ratio: Number.POSITIVE_INFINITY,
      }),
    });

    expect(screen.getByTestId("guess-value")).toHaveTextContent("Keine Ahnung");
    expect(screen.getByTestId("score-points")).toHaveTextContent("0 Punkte");
    expect(screen.getByTestId("explanation")).toHaveTextContent(question.explanation);
    expect(screen.getAllByTestId("source-link")).toHaveLength(question.sources.length);
  });

  it("beschriftet die Schaltfläche je nach Position in der Runde", () => {
    setReducedMotion(true);
    const { unmount } = render(
      <RevealCard
        question={question}
        entry={entryFor()}
        totalPoints={17}
        maxPoints={50}
        isLast={false}
        onNext={vi.fn()}
      />,
    );
    expect(screen.getByTestId("next-button")).toHaveTextContent("Weiter");
    unmount();

    renderCard({ isLast: true });
    expect(screen.getByTestId("next-button")).toHaveTextContent("Zum Ergebnis");
  });

  it("legt den Fokus auf die Weiter-Schaltfläche", () => {
    setReducedMotion(true);
    renderCard();

    expect(screen.getByTestId("next-button")).toHaveFocus();
  });

  it("ruft onNext beim Klick auf", async () => {
    setReducedMotion(true);
    const user = userEvent.setup();
    const { onNext } = renderCard();

    await user.click(screen.getByTestId("next-button"));

    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it("ruft onNext auch bei Enter außerhalb der Schaltfläche auf", () => {
    setReducedMotion(true);
    const { onNext } = renderCard();

    fireEvent.keyDown(document.body, { key: "Enter" });

    expect(onNext).toHaveBeenCalledTimes(1);
  });

  it("löst bei Enter auf einem Link nicht zusätzlich aus", () => {
    setReducedMotion(true);
    const { onNext } = renderCard();

    const link = screen.getAllByTestId("source-link")[0];
    expect(link).toBeDefined();
    fireEvent.keyDown(link as HTMLElement, { key: "Enter" });

    expect(onNext).not.toHaveBeenCalled();
  });
});
