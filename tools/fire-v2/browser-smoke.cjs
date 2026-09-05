// Isolated headless browser only. Never controls the user's browser or desktop input.
const { chromium } = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');

(async () => {
  const started = Date.now();
  const output = path.resolve(process.env.FIRE_QA_DIR || 'target/fire-v2-qa');
  fs.mkdirSync(output, { recursive: true });
  const browser = await chromium.launch({ channel: 'msedge', headless: true, args: ['--enable-unsafe-swiftshader'] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1120 } });
  const errors = [];
  page.on('pageerror', e => errors.push(String(e)));
  const base = process.env.FIRE_QA_URL || 'http://127.0.0.1:8766/games/fire/v2/';
  try {
    await page.goto(base);
    await page.waitForFunction(() => !document.getElementById('btn-practice').disabled, null, { timeout: 90000 });
    await page.screenshot({ path: path.join(output, 'garage-desktop.png'), fullPage: true });
    assert.equal(await page.locator('.car-card').count(), 3);
    await page.getByRole('button', { name: /Select APEX R/ }).click();
    assert.match(await page.locator('#hero-name').innerText(), /APEX R/);
    await page.setViewportSize({ width: 390, height: 844 });
    await page.screenshot({ path: path.join(output, 'garage-mobile.png'), fullPage: true });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'mobile page overflows');
    await page.setViewportSize({ width: 1440, height: 1120 });
    await page.getByRole('button', { name: 'Start race ↗' }).click();
    await page.waitForSelector('#ember-root canvas');
    await page.waitForFunction(() => document.getElementById('hud').classList.contains('live'), null, { timeout: 30000 });
    await page.waitForTimeout(1300);
    await page.screenshot({ path: path.join(output, 'grid.png'), fullPage: true });
    await page.locator('#ember-root canvas').focus();
    await page.waitForTimeout(2500);
    assert.equal(await page.locator('#hud-car').innerText(), 'APEX R');
    await page.keyboard.down('w');
    await page.waitForTimeout(2300);
    const speed = Number(await page.locator('#speed .n').innerText());
    assert.ok(speed > 35, `throttle did not accelerate: ${speed}`);
    await page.keyboard.down('Shift'); await page.waitForTimeout(100); await page.keyboard.up('Shift');
    await page.waitForTimeout(100);
    assert.equal(await page.locator('.pip.on').count(), 2, 'boost press should spend one charge');
    await page.keyboard.down('a');
    await page.keyboard.down('Space');
    await page.waitForTimeout(650);
    await page.keyboard.up('a'); await page.keyboard.up('Space');
    await page.screenshot({ path: path.join(output, 'race-driving.png'), fullPage: true });
    await page.keyboard.up('w');
    await page.keyboard.press('r');
    await page.waitForTimeout(300);
    // Exercise the real restart entry point without re-creating the renderer.
    await page.evaluate(async () => {
      const scripts = performance.getEntriesByType('resource').map(r => r.name);
      const url = scripts.find(n => /\/pkg\/fire\.js\?/.test(n));
      const wasm = await import(url);
      wasm.restart_local(2);
    });
    await page.waitForTimeout(300);
    assert.equal(await page.locator('#hud-car').innerText(), 'V8-R');
    assert.equal(await page.locator('#ember-root canvas').count(), 1);
    assert.equal(await page.locator('.pip.on').count(), 3);
    assert.ok(Number(await page.locator('#speed .n').innerText()) < 1);
    assert.deepEqual(errors, [], 'browser runtime exceptions');
    fs.writeFileSync(path.join(output, 'browser-result.json'), JSON.stringify({ passed: true, throttle_kmh: speed, errors, wall_seconds: (Date.now()-started)/1000 }, null, 2));
    console.log(JSON.stringify({ passed: true, throttle_kmh: speed, wall_seconds: (Date.now()-started)/1000 }));
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
