import { describe, expect, it } from "vitest";

import { questions } from "@/data/questions";
import { CATEGORIES, questionSchema } from "@/data/schema";

describe("Gerüst", () => {
  it("rechnet", () => {
    expect(1 + 1).toBe(2);
  });

  it("kennt genau zehn Kategorien", () => {
    expect(CATEGORIES).toHaveLength(10);
  });

  it("lädt die Beispielfragen und validiert sie gegen das Schema", () => {
    expect(questions).toHaveLength(3);
    for (const question of questions) {
      expect(questionSchema.safeParse(question).success).toBe(true);
    }
  });
});
