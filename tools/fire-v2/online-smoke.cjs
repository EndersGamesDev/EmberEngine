// Real WebSocket server + WebAssembly client in an isolated headless browser.
const { chromium } = require('playwright');
const fs = require('node:fs');
const assert = require('node:assert/strict');
const path = require('node:path');
(async()=>{
  const started=Date.now(), output=path.resolve(process.env.FIRE_QA_DIR||'target/fire-v2-qa');
  const browser=await chromium.launch({channel:'msedge',headless:true,args:['--enable-unsafe-swiftshader']});
  const page=await browser.newPage({viewport:{width:1440,height:1120}}), errors=[];
  page.on('pageerror',e=>errors.push(String(e)));
  try {
    await page.goto(process.env.FIRE_QA_URL||'http://127.0.0.1:8766/games/fire/v2/');
    await page.waitForFunction(()=>!document.getElementById('btn-online').disabled,null,{timeout:60000});
    await page.getByRole('button',{name:/Select V8-R/}).click();
    await page.getByRole('button',{name:'Race online ↗'}).click();
    await page.locator('#handle').fill('FireQA');
    await page.locator('#newlobby').fill(`qa-${Date.now().toString(36)}`);
    await page.waitForFunction(()=>!document.getElementById('host-chip').textContent.includes('no server'),null,{timeout:20000});
    await page.getByRole('button',{name:'Create race'}).click();
    await page.waitForSelector('#ember-root canvas',{timeout:20000});
    await page.waitForFunction(()=>document.getElementById('hud-car').textContent==='V8-R');
    await page.waitForFunction(()=>document.getElementById('race-time').textContent!=='00:00.00',null,{timeout:20000});
    await page.locator('#ember-root canvas').focus();
    await page.keyboard.down('w');await page.waitForTimeout(2200);
    const speed=Number(await page.locator('#speed .n').innerText());
    assert.ok(speed>35,`online car failed to move: ${speed}`);
    await page.keyboard.down('Shift'); await page.waitForTimeout(100); await page.keyboard.up('Shift'); await page.waitForTimeout(250);
    assert.equal(await page.locator('.pip.on').count(),2);
    await page.keyboard.up('w');
    await page.getByRole('button',{name:'SOUND ON'}).click();
    assert.equal(await page.evaluate(()=>document.activeElement.tagName),'CANVAS','sound steals controls');
    await page.screenshot({path:path.join(output,'online-race.png'),fullPage:true});
    assert.deepEqual(errors,[]);
    const result={passed:true,speed_kmh:speed,vehicle:'V8-R',errors,wall_seconds:(Date.now()-started)/1000};
    fs.writeFileSync(path.join(output,'online-result.json'),JSON.stringify(result,null,2));console.log(JSON.stringify(result));
  } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exitCode=1;});
