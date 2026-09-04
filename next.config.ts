import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Das schwebende Dev-Abzeichen liegt sonst über dem Inhalt und landet in
  // jedem Screenshot. Betrifft nur `next dev`, nicht den Produktionsbuild.
  devIndicators: false,
};

export default nextConfig;
