// Render the trailer's title card as a PNG. Text is drawn by the browser rather
// than generated: an image model cannot spell a wordmark reliably, and the card
// is the one frame a viewer reads rather than watches.
//
//   node tools/trailer/title-card.cjs <out.png> [width] [height]
'use strict';
const path = require('node:path');
const fs = require('node:fs');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');

const out = path.resolve(process.argv[2] || 'title.png');
const width = Number(process.argv[3] || 1280);
const height = Number(process.argv[4] || 720);

const CARD = `<style>
  @font-face { font-family: x; src: local('Segoe UI Semibold'); }
  html, body { margin: 0; height: 100%; background: #07090d; }
  .card { position: relative; width: ${width}px; height: ${height}px; overflow: hidden;
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    font-family: 'Segoe UI', system-ui, sans-serif; color: #f5f5f7; }
  /* A slow warm glow off to one side, so the card is not a flat black rectangle. */
  .glow { position: absolute; width: 900px; height: 900px; border-radius: 50%;
    background: radial-gradient(circle, rgba(255,183,77,.20), rgba(255,183,77,0) 62%);
    top: -280px; left: -180px; }
  .glow2 { position: absolute; width: 760px; height: 760px; border-radius: 50%;
    background: radial-gradient(circle, rgba(90,140,190,.18), rgba(90,140,190,0) 62%);
    bottom: -300px; right: -160px; }
  .scan { position: absolute; inset: 0;
    background: repeating-linear-gradient(0deg, rgba(255,255,255,.022) 0 1px, transparent 1px 3px); }
  .word { position: relative; font-size: 132px; font-weight: 700; letter-spacing: .17em;
    text-indent: .17em; line-height: 1; color: #fff; text-shadow: 0 0 46px rgba(255,183,77,.34); }
  .rule { position: relative; width: 430px; height: 2px; margin: 30px 0 24px;
    background: linear-gradient(90deg, transparent, #ffb74d, transparent); }
  .sub { position: relative; font-size: 27px; letter-spacing: .30em; text-indent: .30em;
    color: #ffb74d; font-weight: 600; }
  .url { position: relative; margin-top: 40px; font-size: 20px; letter-spacing: .07em; color: #9aa3ad; }
</style>
<div class="card">
  <div class="glow"></div><div class="glow2"></div><div class="scan"></div>
  <div class="word">KILLSHOT</div>
  <div class="rule"></div>
  <div class="sub">V31 &middot; BREACH-12</div>
  <div class="url">endersgamesdev.github.io/EmberEngine</div>
</div>`;

(async () => {
  const browser = await chromium.launch({
    ...(process.env.EMBER_QA_BROWSER ? { executablePath: process.env.EMBER_QA_BROWSER } : { channel: 'msedge' }),
  });
  const page = await browser.newPage({ viewport: { width, height }, deviceScaleFactor: 1 });
  await page.setContent(CARD);
  await page.waitForTimeout(300);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  await page.locator('.card').screenshot({ path: out });
  await browser.close();
  console.log(JSON.stringify({ out, width, height, bytes: fs.statSync(out).size }));
})();
