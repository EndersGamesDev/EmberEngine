import { defineConfig, devices } from "@playwright/test";

// Standard ist Port 3170 (3100 war auf der Entwicklungsmaschine fremdbelegt). Ist der Port schon
// belegt, wuerde Playwright wegen `reuseExistingServer` die fremde App testen —
// deshalb laesst sich der Port per Umgebungsvariable ausweichen.
const PORT = Number(process.env.PLAYWRIGHT_PORT ?? 3170);
const baseURL = `http://localhost:${PORT}`;

/** Build-Ausgabe des Testservers, getrennt von `.next` des Entwicklungsservers. */
const E2E_DIST_DIR = ".next-e2e";

export default defineConfig({
  testDir: "tests/e2e",
  // Eine komplette Runde ist ein langer Test, und `next dev` uebersetzt jede
  // Route beim ersten Aufruf frisch. Die 30 Sekunden der Voreinstellung sind
  // dafuer knapp; 90 Sekunden lassen Luft, ohne echte Haenger zu verschleiern.
  timeout: 90_000,
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  reporter: [["list"], ["html", { open: "never" }]],
  use: {
    baseURL,
    screenshot: "only-on-failure",
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "desktop",
      use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 } },
    },
    {
      name: "mobile",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 375, height: 667 },
        isMobile: true,
        hasTouch: true,
      },
    },
  ],
  webServer: {
    command: `pnpm dev --port ${PORT}`,
    url: baseURL,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
    // Eigenes Ausgabeverzeichnis: Next 16 laesst pro `distDir` nur einen
    // `next dev` zu. So stoert ein offener Entwicklungsserver die Tests nicht
    // — und die Tests stoeren ihn nicht (siehe `next.config.ts`).
    env: { NEXT_DIST_DIR: E2E_DIST_DIR },
  },
});
