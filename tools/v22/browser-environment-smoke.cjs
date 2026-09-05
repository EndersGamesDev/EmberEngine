// Headless browser verification. Never sends keyboard, mouse or pointer input.
// Run from the repository root after wasm-bindgen has populated web/pkg:
// EMBER_QA_PLAYWRIGHT=<absolute playwright module path> node tools/v22/browser-environment-smoke.cjs
// Optional: EMBER_QA_BROWSER=<browser executable>, EMBER_QA_SERVER=<arena-server executable>.
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const net = require('node:net');
const crypto = require('node:crypto');
const { spawn } = require('node:child_process');
const { chromium } = require(process.env.EMBER_QA_PLAYWRIGHT || 'playwright');

const root = process.cwd();
const webRoot = path.join(root, 'web');
const output = path.resolve(process.env.EMBER_QA_OUTPUT || path.join(root, 'target', 'environment-captures'));
const gamePort = 7779;
const webPort = 8079;
const started = Date.now();
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const html = `<!doctype html><meta charset="utf-8"><title>Environment QA</title>
<style>html,body{margin:0;background:#18202a;color:white}#ember-root{width:960px;height:540px}canvas{width:960px!important;height:540px!important}#status{font:14px monospace;white-space:pre-wrap}</style>
<div id="ember-root"><span id="loading">Loading engine</span></div><pre id="status">Waiting</pre><div id="scoreboard"></div>`;
const result = { maps: [], serverLog: [], elapsedSeconds: 0 };
let server;
let game;
let browser;

function listening(port) {
  return new Promise(resolve => {
    const socket = net.connect({ host: '127.0.0.1', port });
    socket.once('connect', () => { socket.destroy(); resolve(true); });
    socket.once('error', () => { socket.destroy(); resolve(false); });
  });
}

async function main() {
  fs.mkdirSync(output, { recursive: true });
  for (const port of [gamePort, webPort]) {
    if (await listening(port)) throw new Error(`Test port ${port} is occupied; refusing to use an existing service`);
  }
  const serverExe = process.env.EMBER_QA_SERVER || path.join(root, 'target', 'release', process.platform === 'win32' ? 'arena-server.exe' : 'arena-server');
  game = spawn(serverExe, ['--bind', `127.0.0.1:${gamePort}`, '--name', 'environment-qa'], {
    cwd: root, windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
  });
  game.stdout.on('data', data => result.serverLog.push(data.toString()));
  game.stderr.on('data', data => result.serverLog.push(data.toString()));
  game.on('error', error => { result.serverError = String(error); });
  for (let attempt = 0; attempt < 50 && !await listening(gamePort); attempt++) await sleep(100);
  if (!await listening(gamePort)) throw new Error(`Own arena-server did not start: ${result.serverError || result.serverLog.join('')}`);
  result.serverPid = game.pid;

  server = http.createServer((request, response) => {
    const pathname = decodeURIComponent(new URL(request.url, `http://127.0.0.1:${webPort}`).pathname);
    if (pathname === '/qa') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end(html); return; }
    if (pathname === '/favicon.ico') { response.writeHead(204); response.end(); return; }
    const file = path.resolve(webRoot, `.${pathname}`);
    if (!file.startsWith(`${webRoot}${path.sep}`) || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      response.writeHead(404); response.end(); return;
    }
    const mime = { '.js': 'text/javascript', '.wasm': 'application/wasm', '.html': 'text/html' }[path.extname(file)] || 'application/octet-stream';
    response.writeHead(200, { 'Content-Type': mime, 'Cache-Control': 'no-store' });
    fs.createReadStream(file).pipe(response);
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(webPort, '127.0.0.1', resolve); });
  browser = await chromium.launch({
    ...(process.env.EMBER_QA_BROWSER ? { executablePath: process.env.EMBER_QA_BROWSER } : { channel: 'msedge' }),
    headless: true,
    args: ['--disable-webgpu', '--disable-features=WebGPU', '--enable-webgl', '--ignore-gpu-blocklist'],
  });
  for (const map of ['freight-yard', 'trench-city']) {
    const item = { map, errors: [], warnings: [], console: [], screenshots: [] };
    result.maps.push(item);
    console.log(`Starting ${map}`);
    const context = await browser.newContext({ viewport: { width: 960, height: 590 }, deviceScaleFactor: 1 });
    const page = await context.newPage();
    page.on('pageerror', error => item.errors.push(String(error)));
    page.on('console', message => {
      if (message.type() === 'error') item.errors.push(message.text());
      else if (message.type() === 'warning') item.warnings.push(message.text());
      else item.console.push(message.text());
    });
    await page.addInitScript(() => {
      Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
      window.__drawCalls = 0;
      window.__contextCalls = [];
      const getContext = HTMLCanvasElement.prototype.getContext;
      HTMLCanvasElement.prototype.getContext = function(type, ...args) {
        const context = getContext.call(this, type, ...args);
        window.__contextCalls.push({ type, success: Boolean(context) });
        if (type === 'webgl2' && context) {
          const debug = context.getExtension('WEBGL_debug_renderer_info');
          window.__glInfo = {
            version: context.getParameter(context.VERSION),
            renderer: debug ? context.getParameter(debug.UNMASKED_RENDERER_WEBGL) : context.getParameter(context.RENDERER),
          };
        }
        return context;
      };
      for (const method of ['drawArrays', 'drawElements', 'drawArraysInstanced', 'drawElementsInstanced']) {
        const original = WebGL2RenderingContext.prototype[method];
        WebGL2RenderingContext.prototype[method] = function(...args) {
          window.__drawCalls++;
          return original.apply(this, args);
        };
      }
    });
    try {
      await page.goto(`http://127.0.0.1:${webPort}/qa`);
      await page.evaluate(async ({ map, port }) => {
        const arena = await import('/pkg/arena.js');
        await arena.default();
        window.__protocol = arena.proto_version();
        arena.start_online(JSON.stringify({ url: `ws://127.0.0.1:${port}`, action: 'create', lobby: `qa-${map}-${Date.now()}`, handle: 'environment-qa', map, mode: 'ffa' }));
      }, { map, port: gamePort });
      await page.waitForFunction(() => window.__drawCalls > 40 && /HP|hp|frags|arena/.test(document.querySelector('#status').textContent), null, { timeout: 60000 });
      for (let index = 0; index < 2; index++) {
        if (index > 0) await page.waitForTimeout(4000);
        const file = path.join(output, `${map}-${index}.png`);
        const bytes = await page.locator('#ember-root canvas').screenshot({ path: file });
        item.screenshots.push({ file, sha256: crypto.createHash('sha256').update(bytes).digest('hex'), drawCalls: await page.evaluate(() => window.__drawCalls) });
      }
      item.details = await page.evaluate(() => ({ protocol: window.__protocol, gl: window.__glInfo, contexts: window.__contextCalls, drawCalls: window.__drawCalls, status: document.querySelector('#status').textContent, loading: document.querySelector('#loading')?.textContent || null }));
      item.framesDiffer = item.screenshots[0].sha256 !== item.screenshots[1].sha256;
      item.passed = item.errors.length === 0 && item.framesDiffer && Boolean(item.details.gl) && item.details.drawCalls > 40;
    } catch (error) {
      item.errors.push(String(error));
      item.details = await page.evaluate(() => ({ gl: window.__glInfo, contexts: window.__contextCalls, drawCalls: window.__drawCalls, status: document.querySelector('#status')?.textContent }));
      await page.screenshot({ path: path.join(output, `${map}-failure.png`) }).catch(() => {});
      item.passed = false;
    }
    console.log(JSON.stringify(item));
    await context.close();
  }
}

main().catch(error => { result.error = String(error); console.error(error); }).finally(async () => {
  if (browser) await browser.close();
  if (server) await new Promise(resolve => server.close(resolve));
  if (game && game.exitCode === null) game.kill();
  result.elapsedSeconds = (Date.now() - started) / 1000;
  fs.writeFileSync(path.join(output, 'browser-results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ elapsedSeconds: result.elapsedSeconds, maps: result.maps.map(({ map, passed }) => ({ map, passed })), error: result.error || null }));
  if (result.error || result.maps.some(map => !map.passed)) process.exitCode = 1;
});
