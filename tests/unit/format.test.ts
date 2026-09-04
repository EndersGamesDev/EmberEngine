import { describe, expect, it } from "vitest";

import { formatNumber, formatWithUnit, parseGermanNumber } from "@/lib/format";

describe("parseGermanNumber", () => {
  describe("gültige Eingaben", () => {
    const cases: [string, number][] = [
      ["1.250.000", 1_250_000],
      ["1250000", 1_250_000],
      ["12,5", 12.5],
      ["12.5", 12.5],
      ["12,50", 12.5],
      ["1.250,75", 1250.75],
      ["1.250", 1250],
      ["0,5", 0.5],
      ["0", 0],
      ["1.2345", 1.2345],
      ["3", 3],
      ["  42  ", 42],
      ["1 250 000", 1_250_000],
    ];

    for (const [input, expected] of cases) {
      it(`liest „${input}“ als ${expected}`, () => {
        expect(parseGermanNumber(input)).toBe(expected);
      });
    }
  });

  describe("ungültige Eingaben", () => {
    const cases = [
      "",
      "   ",
      "abc",
      "1,250.5",
      "1,2,3",
      "12.",
      "12,",
      ",5",
      ".5",
      "1.25.000",
      "-5",
      "+5",
      "1e5",
      "12 km",
      "3,5%",
    ];

    for (const input of cases) {
      it(`weist „${input}“ ab`, () => {
        expect(parseGermanNumber(input)).toBeNull();
      });
    }

    it("weist Eingaben ab, die keine endliche Zahl ergeben", () => {
      expect(parseGermanNumber("9".repeat(400))).toBeNull();
    });
  });

  it("behandelt einen einzelnen Punkt vor drei Ziffern als Tausenderpunkt", () => {
    expect(parseGermanNumber("1.250")).toBe(1250);
    expect(parseGermanNumber("1.25")).toBe(1.25);
    expect(parseGermanNumber("1.2500")).toBe(1.25);
  });
});

describe("formatNumber", () => {
  const cases: [number, string][] = [
    [42.195, "42,195"],
    [1_250_000, "1.250.000"],
    [99.86, "99,86"],
    [3, "3"],
    [0.5, "0,5"],
    [1250.75, "1.250,75"],
    [4.3252003274489856e19, "43.252.003.274.489.856.000"],
  ];

  for (const [value, expected] of cases) {
    it(`schreibt ${value} als „${expected}“`, () => {
      expect(formatNumber(value)).toBe(expected);
    });
  }

  it("kürzt auf drei Nachkommastellen, wenn nichts anderes gefordert ist", () => {
    expect(formatNumber(1.23456)).toBe("1,235");
  });

  it("respektiert maximumFractionDigits", () => {
    expect(formatNumber(42.195, { maximumFractionDigits: 0 })).toBe("42");
    expect(formatNumber(1.23456, { maximumFractionDigits: 5 })).toBe("1,23456");
  });
});

describe("formatWithUnit", () => {
  it("hängt die Einheit mit Leerzeichen an", () => {
    expect(formatWithUnit(1_250_000, "km")).toBe("1.250.000 km");
    expect(formatWithUnit(71, "%")).toBe("71 %");
    expect(formatWithUnit(122, "Jahre")).toBe("122 Jahre");
  });

  it("lässt die Einheit weg, wenn sie leer ist", () => {
    expect(formatWithUnit(3, "")).toBe("3");
    expect(formatWithUnit(3, "   ")).toBe("3");
  });

  it("reicht die Formatoptionen durch", () => {
    expect(formatWithUnit(42.195, "km", { maximumFractionDigits: 1 })).toBe("42,2 km");
  });
});
