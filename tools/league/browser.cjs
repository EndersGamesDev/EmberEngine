// Real League WASM, DOM and private WebSocket server in a headless browser.
// Synthetic events stay inside the disposable page; no desktop input or foreground changes.
'use strict';
const assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),os=require('node:os');
const {spawn}=require('node:child_process');
const {gameVersion,safePath}=require('./publish.cjs');
const selected=gameVersion(process.env.LEAGUE_GAME_VERSION||'v1');
const {chromium}=require(process.env.EMBER_QA_PLAYWRIGHT||'playwright');
const root=process.cwd(),web=path.join(root,'web'),out=path.join(root,selected==='v1'?'target/league-browser':`target/league-browser-${selected}`);
const entry=`games/league/${selected}`;
const origin='http://127.0.0.1:8093',ws='ws://127.0.0.1:7793';
const started=Date.now(),report={gameVersion:selected,checks:[],errors:[],screenshots:[]};
let browser,server,game;
const fixturePeers=new Set();
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const check=(ok,name)=>{assert(ok,name);report.checks.push(name);console.log('PASS '+name);};
async function page({holdWasm=false}={}){
  const p=await browser.newPage({viewport:{width:1440,height:1000}});
  p.on('pageerror',e=>report.errors.push(e.message));
  p.on('response',response=>{if(response.status()>=400)report.errors.push(`HTTP ${response.status()}: ${response.url()}`);});
  await p.addInitScript(()=>{window.focus=()=>{};Element.prototype.setPointerCapture=()=>{};});
  let releaseWasm;
  if(holdWasm){
    const gate=new Promise(resolve=>{releaseWasm=resolve;});
    await p.route('**/*.wasm',async route=>{await gate;await route.continue();});
  }
  await p.goto(`${origin}/${entry}/`,{waitUntil:holdWasm?'commit':'load'});
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
// Fixture peers use the real private server protocol; Quick match still runs
// through the shipped page, lobby discovery, WASM client and draft screen.
async function fixturePeer(handle,proto){
  const socket=new WebSocket(ws),messages=[],waiters=[];
  socket.addEventListener('message',event=>{
    const message=JSON.parse(event.data);
    const at=waiters.findIndex(waiter=>waiter.type===message.t||message.t==='rejected');
    if(at<0){messages.push(message);return;}
    const waiter=waiters.splice(at,1)[0];clearTimeout(waiter.timer);
    if(message.t==='rejected')waiter.reject(new Error(`Fixture ${handle}: ${message.reason}`));
    else waiter.resolve(message);
  });
  const receive=type=>new Promise((resolve,reject)=>{
    const at=messages.findIndex(message=>message.t===type||message.t==='rejected');
    if(at>=0){const message=messages.splice(at,1)[0];if(message.t==='rejected')reject(new Error(message.reason));else resolve(message);return;}
    const waiter={type,resolve,reject,timer:null};
    waiter.timer=setTimeout(()=>{const at=waiters.indexOf(waiter);if(at>=0)waiters.splice(at,1);reject(new Error(`Fixture ${handle}: timed out waiting for ${type}`));},6000);
    waiters.push(waiter);
  });
  const peer={
    async request(message,type){const reply=receive(type);socket.send(JSON.stringify(message));return reply;},
    async close(){
      fixturePeers.delete(peer);
      if(socket.readyState===WebSocket.CLOSED)return;
      await new Promise(resolve=>{socket.addEventListener('close',resolve,{once:true});socket.close();});
    }
  };
  fixturePeers.add(peer);
  await new Promise((resolve,reject)=>{socket.addEventListener('open',resolve,{once:true});socket.addEventListener('error',reject,{once:true});});
  await peer.request({t:'hello',proto,handle},'welcome');
  return peer;
}
function recordJoined(p){
  const joined=[];
  p.on('websocket',socket=>socket.on('framereceived',event=>{
    try{const message=JSON.parse(String(event.payload));if(message.t==='joined')joined.push(message);}catch{}
  }));
  return joined;
}
async function quickMatch(mode){
  const peers=[],pages=[];
  try{
    const creator=await page();pages.push(creator);
    const proto=await creator.evaluate(()=>window.qaWasm.proto_version());
    const seed=async(name,roomMode,password=null)=>{
      const peer=await fixturePeer(name,proto);peers.push(peer);
      await peer.request({t:'create_lobby',name,mode:roomMode,password},'joined');
      return peer;
    };
    const full=`quick${mode}-full`,locked=`quick${mode}-locked`,wrong=`quick${mode}-wrong`;
    const inspector=await seed(full,mode);
    for(let i=1;i<mode*2;i++){
      const peer=await fixturePeer(`quick${mode}-fill${i}`,proto);peers.push(peer);
      await peer.request({t:'join_lobby',name:full,password:null},'joined');
    }
    await seed(locked,mode,'fixture-only');
    await seed(wrong,mode===1?3:1);
    const before=(await inspector.request({t:'list_lobbies'},'lobbies')).lobbies;
    check(before.find(l=>l.name===full)?.players===mode*2,`quick ${mode}v${mode}: full room fixture occupies every seat`);
    check(before.find(l=>l.name===locked)?.has_password===true,`quick ${mode}v${mode}: locked room fixture is advertised`);
    check(before.find(l=>l.name===wrong)?.mode!==mode,`quick ${mode}v${mode}: opposite mode fixture is advertised`);
    assert(before.every(l=>l.racing||l.has_password||l.players>=l.cap||l.mode!==mode),'Expected no eligible lobby before Quick match');
    const createdJoins=recordJoined(creator);
    await creator.evaluate(mode=>{document.getElementById('newmode').value=String(mode);},mode);
    await click(creator,'#btn-quick');
    await waitState(creator,()=>window.qaState().connected&&window.qaState().roster.length>0);
    const created=createdJoins.at(-1),createdState=await state(creator);
    check(created&&created.mode===mode&&!before.some(l=>l.name===created.lobby),`quick ${mode}v${mode}: creates selected mode when only full, locked or wrong-mode rooms exist`);
    check(createdState.slot===0&&createdState.mode===mode&&createdState.roster.length===mode*2,`quick ${mode}v${mode}: new room opens its host draft with the correct team size`);
    const guest=await page();pages.push(guest);const guestJoins=recordJoined(guest);
    await guest.evaluate(mode=>{document.getElementById('newmode').value=String(mode);},mode);
    await click(guest,'#btn-quick');
    await waitState(guest,()=>window.qaState().connected&&window.qaState().roster.length>0);
    const guestState=await state(guest);
    check(guestJoins.at(-1)?.lobby===created.lobby&&guestState.slot===1&&guestState.mode===mode,`quick ${mode}v${mode}: joins the existing eligible room instead of creating another`);
    const after=(await inspector.request({t:'list_lobbies'},'lobbies')).lobbies;
    check(after.length===before.length+1&&after.find(l=>l.name===created.lobby)?.players===2,`quick ${mode}v${mode}: both browser players share one new lobby`);
    check([full,locked,wrong].every(name=>after.find(l=>l.name===name)?.players===before.find(l=>l.name===name).players),`quick ${mode}v${mode}: excluded rooms retain their original players`);
  }finally{
    await Promise.all(pages.map(p=>p.close()));
    await Promise.all(peers.map(peer=>peer.close()));
  }
}
async function main(){
  os.setPriority(0,os.constants.priority.PRIORITY_LOW);fs.mkdirSync(out,{recursive:true});
  const catalog=JSON.parse(fs.readFileSync(path.join(web,'games.json')));
  const proto=catalog.games.find(game=>game.id==='league')?.versions.find(version=>version.v===selected&&version.path===`${entry}/`)?.proto;
  assert(Number.isInteger(proto)&&proto>0,'Selected League version is missing from the local catalog');
  safePath(web,`${entry}/index.html`);
  server=http.createServer((req,res)=>{
    let url;try{url=new URL(req.url,origin);}catch{res.writeHead(400).end();return;}
    if(url.pathname==='/server.json'){res.setHeader('Content-Type','application/json');res.end(JSON.stringify({hosts:[{name:'league-qa',league_ws:ws,league_proto:proto}],mirrors:[]}));return;}
    if(url.pathname==='/favicon.ico'){res.writeHead(204).end();return;}
    let rel=url.pathname.endsWith('/')?url.pathname+'index.html':url.pathname;
    rel=rel.slice(1);
    if(rel.startsWith(`${entry}/pkg/`)){
      const bundle=rel.slice(`${entry}/pkg/`.length);
      if(!['league.js','league_bg.wasm'].includes(bundle)){res.writeHead(404).end();return;}
      rel='pkg/'+bundle;
    }
    let f;try{f=safePath(web,rel);if(!fs.lstatSync(f).isFile())throw new Error('Not a file');}catch{res.writeHead(404).end();return;}
    res.setHeader('Content-Type',({'.wasm':'application/wasm','.js':'text/javascript','.mjs':'text/javascript','.css':'text/css','.html':'text/html','.json':'application/json','.png':'image/png','.jpg':'image/jpeg','.jpeg':'image/jpeg','.webp':'image/webp','.svg':'image/svg+xml','.woff':'font/woff','.woff2':'font/woff2'})[path.extname(f).toLowerCase()]||'application/octet-stream');
    res.end(fs.readFileSync(f));
  });
  await new Promise((r,j)=>{server.once('error',j);server.listen(8093,'127.0.0.1',r);});
  game=spawn(path.join(root,'target/release/league-server.exe'),['127.0.0.1:7793','--name','league-qa'],{windowsHide:true});
  game.on('error',e=>report.errors.push(e.message));game.stderr.on('data',()=>{});game.stdout.on('data',()=>{});
  await pause(600);assert(game.exitCode===null,'Private server failed to start');
  browser=await chromium.launch({channel:'msedge',headless:true,args:['--disable-webgpu','--disable-features=WebGPU','--enable-webgl','--ignore-gpu-blocklist']});
  for(let champ=0;champ<5;champ++)await practice(1,champ);
  await practice(3,0);await online();
  await quickMatch(1);await quickMatch(3);
  check(report.errors.length===0,'no uncaught browser errors');report.passed=true;
}
main().catch(e=>{report.failure=e.stack;console.error(e);process.exitCode=1;}).finally(async()=>{
  if(report.failure&&browser){for(const [i,p] of browser.contexts().flatMap(c=>c.pages()).entries()){try{await screenshot(p,'failure-'+i);console.log(await p.evaluate(()=>({body:document.body.innerText,state:window.qaState?.()})));}catch{}}}
  if(browser)await browser.close();await Promise.all([...fixturePeers].map(peer=>peer.close()));if(game)game.kill();if(server)await new Promise(r=>server.close(r));
  report.elapsedSeconds=(Date.now()-started)/1000;fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report,null,2));
});
