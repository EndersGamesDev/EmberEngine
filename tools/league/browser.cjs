// Real League WASM, DOM and private WebSocket server in a headless browser.
// Synthetic events stay inside the disposable page; no desktop input or foreground changes.
'use strict';
const assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),os=require('node:os');
const {spawn}=require('node:child_process');
const {chromium}=require(process.env.EMBER_QA_PLAYWRIGHT||'playwright');
const root=process.cwd(),web=path.join(root,'web'),out=path.join(root,'target/league-browser');
const origin='http://127.0.0.1:8093',ws='ws://127.0.0.1:7793';
const started=Date.now(),report={checks:[],errors:[],screenshots:[]};
let browser,server,game;
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const check=(ok,name)=>{assert(ok,name);report.checks.push(name);console.log('PASS '+name);};
async function page({holdWasm=false}={}){
  const p=await browser.newPage({viewport:{width:1440,height:1000}});
  p.on('pageerror',e=>report.errors.push(e.message));
  await p.addInitScript(()=>{window.focus=()=>{};Element.prototype.setPointerCapture=()=>{};});
  let releaseWasm;
  if(holdWasm){
    const gate=new Promise(resolve=>{releaseWasm=resolve;});
    await p.route('**/*.wasm',async route=>{await gate;await route.continue();});
  }
  await p.goto(origin+'/games/league/v1/',{waitUntil:holdWasm?'commit':'load'});
  if(holdWasm){
    await p.waitForFunction(()=>document.body?.dataset.leagueBoot==='loading');
    check(await p.evaluate(()=>['btn-practice','btn-practice3','btn-create','btn-quick'].every(id=>document.getElementById(id).disabled)),'launch controls stay disabled while WASM is loading');
    await p.evaluate(()=>{for(const id of ['btn-practice','btn-practice3','btn-create','btn-quick'])document.getElementById(id).click();});
    check(await p.locator('#ember-root canvas').count()===0,'clicking launch controls during loading cannot start a partial engine');
    releaseWasm();
  }
  await p.waitForFunction(()=>['ready','failed'].includes(document.body.dataset.leagueBoot));
  await p.evaluate(async()=>{
    window.qaWasm=await window.leagueReady;
    if(!window.qaWasm)throw new Error(document.getElementById('engine-note').textContent);
    window.qaState=()=>JSON.parse(window.qaWasm.state_json());
  });
  if(holdWasm)check((await state(p)).phase==='select','readiness exposes the initialized WASM API after delayed loading');
  return p;
}
async function click(p,s){
  await p.waitForFunction(s=>{const e=document.querySelector(s);return e&&!e.disabled&&e.getBoundingClientRect().width>0&&!e.classList.contains('hidden');},s);
  await p.evaluate(s=>document.querySelector(s).click(),s);
}
const state=p=>p.evaluate(()=>window.qaState());
const command=(p,c)=>p.evaluate(c=>window.qaWasm.cmd_json(JSON.stringify(c)),c);
async function waitState(p,predicate){await p.waitForFunction(predicate,null,{timeout:20000});return state(p);}
async function key(p,code){
  for(const type of ['keydown','keyup']){
    await p.evaluate(({type,code})=>document.querySelector('#ember-root canvas').dispatchEvent(new KeyboardEvent(type,{code,key:code.startsWith('Key')?code.slice(3).toLowerCase():code,bubbles:true,cancelable:true})),{type,code});
    await pause(80);
  }
}
async function motion(p,xf=.62,yf=.45,clickGround=false){
  await p.evaluate(({xf,yf,clickGround})=>{
    const c=document.querySelector('#ember-root canvas'),r=c.getBoundingClientRect();
    const init={bubbles:true,cancelable:true,pointerType:'mouse',pointerId:1,isPrimary:true,clientX:r.left+r.width*xf,clientY:r.top+r.height*yf,button:-1,buttons:0};
    const e=new PointerEvent('pointermove',init);Object.defineProperty(e,'getCoalescedEvents',{value:()=>[e]});c.dispatchEvent(e);
    if(clickGround)c.dispatchEvent(new PointerEvent('pointerdown',{...init,button:2,buttons:2}));
  },{xf,yf,clickGround});
  await pause(100);
  if(clickGround)await p.evaluate(()=>document.querySelector('#ember-root canvas').dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerType:'mouse',pointerId:1,button:2,buttons:0})));
}
async function screenshot(p,name){const f=path.join(out,name+'.png');await p.screenshot({path:f,fullPage:true});report.screenshots.push(f);}
async function practice(mode,champ){
  const p=await page({holdWasm:mode===1&&champ===0});
  await click(p,mode===1?'#btn-practice':'#btn-practice3');
  await waitState(p,()=>window.qaState().connected&&window.qaState().phase==='select');
  const cards=await p.locator('#cards .card').count();check(cards===5,`practice ${mode}v${mode}: five champion cards`);
  await click(p,`#cards [data-c="${champ}"]`);
  await p.waitForFunction(champ=>window.qaState().roster[0].champ===champ,champ);
  if(champ===0&&mode===1){
    await click(p,'#runebox [data-r="0"]');
    await p.waitForFunction(()=>document.getElementById('btn-start').disabled);
    check(true,'incomplete rune page prevents starting');
    await click(p,'#runebox [data-r="5"]');
    await p.waitForFunction(()=>window.qaState().roster[0].runes.includes(5));
    check(await p.evaluate(()=>JSON.parse(localStorage.getItem('ember-league-pages')).A.includes(5)),'rune changes save to selected page');
  }
  if(champ===0&&mode===1)await screenshot(p,'draft');
  await click(p,'#btn-start');
  const s=await waitState(p,()=>window.qaState().phase==='live'&&window.qaState().me);
  check(s.roster.length===mode*2,`practice ${mode}v${mode}: complete roster`);
  check(s.cores.length===2&&s.cores.every(c=>c.hp>0&&c.mh>0),`champion ${champ}: visible core health`);
  check(s.me.lv===1&&s.me.pt===1,`champion ${champ}: level one skill point`);
  const ability=(champ===0||champ===2)?1:0;
  await click(p,`#abils [data-abil="${ability}"] .up`);
  await p.waitForFunction(i=>window.qaState().me.rk[i]===1,ability);
  check(true,`champion ${champ}: ability learned`);
  await motion(p);
  const mana=(await state(p)).me.mn;
  await key(p,ability===0?'KeyQ':'KeyW');
  await pause(250);
  const after=await state(p);
  check(after.me.cd[ability]>0||after.me.mn<mana,`champion ${champ}: learned ability activates through keyboard`);
  check(after.me.stats.attackSpeed>0&&after.me.stats.attackSpeed<2,`champion ${champ}: attack speed uses correct units`);
  await key(p,'KeyD');
  await p.waitForFunction(()=>window.qaState().me.scd[0]>0);
  check(true,`champion ${champ}: D spell activates`);
  await key(p,'KeyB');await p.waitForFunction(()=>getComputedStyle(document.getElementById('shop')).display!=='none');
  check(true,`champion ${champ}: shop opens`);
  const items=await p.evaluate(()=>JSON.parse(window.qaWasm.data_json()).items);
  const potion=items.find(i=>i.cost<=100)||items.find(i=>i.cost<=500);
  if(potion){await click(p,`#shop [data-buy="${potion.id}"]`);await pause(150);check((await state(p)).me.g<after.me.g,`champion ${champ}: buying spends gold`);}
  await key(p,'KeyB');await p.waitForFunction(()=>getComputedStyle(document.getElementById('shop')).display==='none');
  check(true,`champion ${champ}: shop closes`);
  if(potion?.charges){
    const charges=(await state(p)).me.charges[0];
    await key(p,'Digit1');
    await p.waitForFunction(n=>window.qaState().me.charges[0]<n,charges);
    check(true,`champion ${champ}: item key consumes a potion charge`);
  }
  const start=await state(p);await motion(p,.73,.47,true);await pause(300);
  const moved=await state(p);
  check(Math.hypot(moved.me.x-start.me.x,moved.me.z-start.me.z)>.1,`champion ${champ}: ground click moves the champion`);
  if(mode===3){await motion(p,.55,.5,true);await pause(1000);await screenshot(p,'squad');}
  else if(champ===1)await screenshot(p,'emberknight');
  await p.close();
}
async function online(){
  const a=await page(),b=await page();
  await a.evaluate(()=>{document.getElementById('newlobby').value='browser-duel';document.getElementById('newmode').value='1';});
  await click(a,'#btn-create');
  await waitState(a,()=>window.qaState().connected&&window.qaState().roster.length===2);
  await b.evaluate(()=>{sessionStorage.setItem('ember-pending',JSON.stringify({game:'league',lobby:'browser-duel',ws:'ws://127.0.0.1:7793'}));});
  // Use the actual lobby join button, which also exercises discovery and handover-free entry.
  await click(b,'#btn-refresh');
  await b.waitForFunction(()=>document.querySelector('#lobbies button'));
  await click(b,'#lobbies button');
  await waitState(b,()=>window.qaState().connected&&window.qaState().slot===1);
  check((await state(b)).slot===1,'online guest retains seat one in draft');
  await click(a,'#cards [data-c="0"]');
  await click(b,'#cards [data-c="0"]');
  await a.waitForFunction(()=>window.qaState().roster.filter(r=>r.picked).length===2);
  check(true,'opposing teams can select the same champion');
  await click(a,'#btn-start');
  await waitState(a,()=>window.qaState().phase==='live'&&window.qaState().me);
  await waitState(b,()=>window.qaState().phase==='live'&&window.qaState().me);
  check(true,'two real browser clients enter an authoritative online match');
  await command(b,{rank:0});await b.waitForFunction(()=>window.qaState().me.rk[0]===1);
  await screenshot(b,'online-guest');
  await a.close();await pause(250);
  check((await state(b)).connected,'guest remains connected after host leaves');
  await b.close();
}
async function main(){
  os.setPriority(0,os.constants.priority.PRIORITY_LOW);fs.mkdirSync(out,{recursive:true});
  server=http.createServer((req,res)=>{
    let url;try{url=new URL(req.url,origin);}catch{res.writeHead(400).end();return;}
    if(url.pathname==='/server.json'){res.setHeader('Content-Type','application/json');res.end(JSON.stringify({hosts:[{name:'league-qa',league_ws:ws,league_proto:1}],mirrors:[]}));return;}
    if(url.pathname==='/favicon.ico'){res.writeHead(204).end();return;}
    let rel=url.pathname.endsWith('/')?url.pathname+'index.html':url.pathname;
    if(rel.startsWith('/games/league/v1/pkg/'))rel='/pkg/'+path.basename(rel);
    const f=path.resolve(web,'.'+rel);
    if(!f.startsWith(web+path.sep)||!fs.existsSync(f)){res.writeHead(404).end();return;}
    res.setHeader('Content-Type',({'.wasm':'application/wasm','.js':'text/javascript','.html':'text/html','.json':'application/json'})[path.extname(f)]||'application/octet-stream');
    res.end(fs.readFileSync(f));
  });
  await new Promise((r,j)=>{server.once('error',j);server.listen(8093,'127.0.0.1',r);});
  game=spawn(path.join(root,'target/release/league-server.exe'),['127.0.0.1:7793','--name','league-qa'],{windowsHide:true});
  game.on('error',e=>report.errors.push(e.message));game.stderr.on('data',()=>{});game.stdout.on('data',()=>{});
  await pause(600);assert(game.exitCode===null,'Private server failed to start');
  browser=await chromium.launch({channel:'msedge',headless:true,args:['--disable-webgpu','--disable-features=WebGPU','--enable-webgl','--ignore-gpu-blocklist']});
  for(let champ=0;champ<5;champ++)await practice(1,champ);
  await practice(3,0);await online();
  check(report.errors.length===0,'no uncaught browser errors');report.passed=true;
}
main().catch(e=>{report.failure=e.stack;console.error(e);process.exitCode=1;}).finally(async()=>{
  if(report.failure&&browser){for(const [i,p] of browser.contexts().flatMap(c=>c.pages()).entries()){try{await screenshot(p,'failure-'+i);console.log(await p.evaluate(()=>({body:document.body.innerText,state:window.qaState?.()})));}catch{}}}
  if(browser)await browser.close();if(game)game.kill();if(server)await new Promise(r=>server.close(r));
  report.elapsedSeconds=(Date.now()-started)/1000;fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
});
