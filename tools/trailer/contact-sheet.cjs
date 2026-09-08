// Compose evenly-spaced frames from a captured shot into one contact sheet, so a
// whole take can be reviewed as a single picture instead of dozens of files.
//
//   node tools/trailer/contact-sheet.cjs <frames-dir> <out.png> [columns] [tiles]
//
// The montage is drawn in the same headless browser the capture uses; no image
// library is installed on this host and none is needed.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');

const dir = path.resolve(process.argv[2]);
const out = path.resolve(process.argv[3] || path.join(dir, 'sheet.png'));
const columns = Number(process.argv[4] || 4);
const tiles = Number(process.argv[5] || 12);

(async () => {
  const all = fs.readdirSync(dir).filter((f) => f.endsWith('.jpg')).sort();
  if (!all.length) throw new Error(`No frames in ${dir}`);
  const step = Math.max(1, Math.floor(all.length / tiles));
  const picked = [];
  for (let i = 0; i < all.length && picked.length < tiles; i += step) picked.push(all[i]);

  const items = picked.map((name) => ({
    label: name.replace(/\.jpg$/, ''),
    data: `data:image/jpeg;base64,${fs.readFileSync(path.join(dir, name)).toString('base64')}`,
  }));

  const browser = await chromium.launch({
    ...(process.env.EMBER_QA_BROWSER ? { executablePath: process.env.EMBER_QA_BROWSER } : { channel: 'msedge' }),
  });
  const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
  await page.setContent(`<style>
    body { margin: 0; background: #111; font: 12px system-ui; }
    .grid { display: grid; grid-template-columns: repeat(${columns}, 1fr); gap: 4px; padding: 4px; }
    figure { margin: 0; position: relative; }
    img { width: 100%; display: block; }
    figcaption { position: absolute; left: 4px; bottom: 4px; color: #ffd166;
                 background: rgba(0,0,0,.65); padding: 1px 5px; border-radius: 3px; }
  </style><div class="grid">${
    items.map((i) => `<figure><img src="${i.data}"><figcaption>${i.label}</figcaption></figure>`).join('')
  }</div>`);
  await page.waitForFunction(() => [...document.images].every((i) => i.complete));
  const grid = await page.$('.grid');
  fs.mkdirSync(path.dirname(out), { recursive: true });
  await grid.screenshot({ path: out });
  await browser.close();
  console.log(JSON.stringify({ frames: all.length, tiles: picked.length, out }));
})();
