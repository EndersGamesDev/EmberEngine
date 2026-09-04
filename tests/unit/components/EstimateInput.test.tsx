import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi, type Mock } from "vitest";

import { EstimateInput } from "@/components/EstimateInput";

interface Handlers {
  onSubmit: Mock<(value: number) => void>;
  onSkip: Mock<() => void>;
}

function setup(unit = "km"): Handlers {
  const handlers: Handlers = {
    onSubmit: vi.fn<(value: number) => void>(),
    onSkip: vi.fn<() => void>(),
  };
  render(
    <EstimateInput
      unit={unit}
      questionId="q001"
      onSubmit={handlers.onSubmit}
      onSkip={handlers.onSkip}
    />,
  );
  return handlers;
}

function input(): HTMLElement {
  return screen.getByTestId("estimate-input");
}

function submitButton(): HTMLElement {
  return screen.getByTestId("submit-guess");
}

describe("EstimateInput", () => {
  it("übergibt eine Zahl mit Dezimalkomma korrekt geparst", async () => {
    const user = userEvent.setup();
    const { onSubmit } = setup();

    await user.type(input(), "12,5");
    await user.click(submitButton());

    expect(onSubmit).toHaveBeenCalledExactlyOnceWith(12.5);
  });

  it("übergibt eine Zahl mit Dezimalpunkt korrekt geparst", async () => {
    const user = userEvent.setup();
    const { onSubmit } = setup();

    await user.type(input(), "12.5");
    await user.click(submitButton());

    expect(onSubmit).toHaveBeenCalledExactlyOnceWith(12.5);
  });

  it("versteht Tausenderpunkte", async () => {
    const user = userEvent.setup();
    const { onSubmit } = setup();

    await user.type(input(), "1.250.000");
    await user.click(submitButton());

    expect(onSubmit).toHaveBeenCalledExactlyOnceWith(1250000);
  });

  it("deaktiviert den Button bei leerer Eingabe und zeigt keinen Fehler", () => {
    setup();

    expect(submitButton()).toBeDisabled();
    expect(screen.getByTestId("estimate-hint")).toHaveTextContent("Tipp eine Zahl ein");
    expect(screen.queryByTestId("estimate-preview")).not.toBeInTheDocument();
  });

  it("sendet mit Enter im Eingabefeld ab", async () => {
    const user = userEvent.setup();
    const { onSubmit } = setup();

    await user.type(input(), "300{Enter}");

    expect(onSubmit).toHaveBeenCalledExactlyOnceWith(300);
  });

  it("zeigt die Vorschau formatiert mit Einheit", async () => {
    const user = userEvent.setup();
    setup("km");

    await user.type(input(), "1250000");

    expect(screen.getByTestId("estimate-preview")).toHaveTextContent(
      "Deine Schätzung: 1.250.000 km",
    );
  });

  it("lässt die Einheit in der Vorschau weg, wenn es keine gibt", async () => {
    const user = userEvent.setup();
    setup("");

    await user.type(input(), "42");

    expect(screen.getByTestId("estimate-preview")).toHaveTextContent("Deine Schätzung: 42");
  });

  it("zeigt bei ungültiger Eingabe einen Hinweis und deaktiviert den Button", async () => {
    const user = userEvent.setup();
    setup();

    await user.type(input(), "1,250.5");

    expect(screen.getByTestId("estimate-hint")).toHaveTextContent(
      "Bitte nur Ziffern, Komma oder Punkt",
    );
    expect(screen.queryByTestId("estimate-preview")).not.toBeInTheDocument();
    expect(submitButton()).toBeDisabled();
    expect(input()).toHaveAttribute("aria-invalid", "true");
  });

  it("ruft bei „Keine Ahnung“ onSkip auf", async () => {
    const user = userEvent.setup();
    const { onSkip, onSubmit } = setup();

    await user.click(screen.getByTestId("skip-button"));

    expect(onSkip).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("setzt den Fokus auf das Eingabefeld", () => {
    setup();
    expect(input()).toHaveFocus();
  });
});
