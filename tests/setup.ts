import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, beforeEach } from "vitest";

import { installMatchMedia, resetReducedMotion } from "./support/reduced-motion";

// Vor jedem Test steht ein frisches `matchMedia` bereit, standardmäßig ohne
// `prefers-reduced-motion`. Tests, die es brauchen, schalten mit
// `setReducedMotion(true)` um.
beforeEach(() => {
  resetReducedMotion();
  installMatchMedia();
});

// `globals: false` in der Vitest-Config heißt: Testing Library räumt nicht von
// allein auf. Also hier, einmal für alle Komponententests.
afterEach(() => {
  cleanup();
});
