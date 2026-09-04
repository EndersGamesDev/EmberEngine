import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Das schwebende Dev-Abzeichen liegt sonst über dem Inhalt und landet in
  // jedem Screenshot. Betrifft nur `next dev`, nicht den Produktionsbuild.
  devIndicators: false,

  // Next 16 lässt pro Ausgabeverzeichnis nur einen `next dev` zu und legt dafür
  // die Sperrdatei `<distDir>/dev/lock` an. Ein offener Entwicklungsserver
  // würde damit jeden `pnpm test:e2e` blockieren. Der Playwright-Server bekommt
  // deshalb über `NEXT_DIST_DIR` sein eigenes Verzeichnis; ohne die Variable
  // bleibt alles beim Standard `.next`.
  distDir: process.env.NEXT_DIST_DIR ?? ".next",
};

export default nextConfig;
