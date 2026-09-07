// Prepared release proof. Run only after the publisher verifies the public deployment.
// Headless Edge and synthetic events stay inside this disposable page.
'use strict';
const assert=require('node:assert/strict');
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),crypto=require('node:crypto');
const {gameVersion}=require('./publish.cjs');
const selected=gameVersion(process.env.LEAGUE_GAME_VERSION||'v1');
const root=process.cwd(),out=path.join(root,selected==='v1'?'target/league-public-browser':`target/league-public-browser-${selected}`);
function proofBaseUrl(override){
  if(override===undefined)return 'https://endersgamesdev.github.io/EmberEngine/';
  assert(typeof override==='string'&&override.trim(),'LEAGUE_PUBLIC_BASE_URL must be a nonempty loopback URL');
  const url=new URL(override);
  assert(['http:','https:'].includes(url.protocol)&&['127.0.0.1','localhost','[::1]'].includes(url.hostname),'LEAGUE_PUBLIC_BASE_URL is test-only and must use loopback');
  assert(!url.username&&!url.password&&!url.search&&!url.hash,'Loopback proof URL must not contain credentials, a query or a fragment');
  if(!url.pathname.endsWith('/'))url.pathname+='/';
  return url.href;
}
const hubUrl=proofBaseUrl(process.env.LEAGUE_PUBLIC_BASE_URL);
const gameUrl=new URL(`games/league/${selected}/`,hubUrl).href;
const started=Date.now(),report={gameVersion:selected,testOnlyLoopback:process.env.LEAGUE_PUBLIC_BASE_URL!==undefined,hubUrl,gameUrl,checks:[],errors:[],screenshots:[],passed:false};
const lobbyName=`proof-${Date.now().toString(36)}-${crypto.randomBytes(3).toString('hex')}`;
// Never include the password in logs, result JSON, or screenshots of field values.
const lobbyPassword=crypto.randomBytes(20).toString('hex');
let browser,gamePage;
const check=(condition,name)=>{assert(condition,name);report.checks.push(name);console.log('PASS '+name);};
const snapshot=p=>p.evaluate(()=>JSON.parse(window.proofWasm.state_json()));
async function click(p,selector){
  await p.waitForFunction(selector=>{const e=document.querySelector(selector);return e&&!e.disabled&&e.getBoundingClientRect().width>0&&getComputedStyle(e).display!=='none';},selector,{timeout:30000});
  await p.evaluate(selector=>document.querySelector(selector).click(),selector);
}
async function capture(p,name){
  const file=path.join(out,name+'.png');
  await p.screenshot({path:file,fullPage:true});report.screenshots.push(file);
}
// Observed authoritative progress guarantees the engine had frames to consume
// a physical press even when its renderer is slower than the old 80ms pulse.
async function advance(p,seconds=.15){
  const before=await snapshot(p);
  assert(before.connected&&before.phase==='live'&&before.me?.alive,'Proof needs a live connected champion');
  await p.waitForFunction(target=>{
    const s=JSON.parse(window.proofWasm.state_json());
    if(!s.connected||s.phase!=='live'||!s.me?.alive)throw new Error('Proof match stopped while waiting for input processing');
    return s.secs>=target;
  },before.secs+seconds,{timeout:15000});
  return snapshot(p);
}
async function press(p,code,selector='#ember-root canvas'){
  for(const type of ['keydown','keyup']){
    await p.evaluate(({type,code,selector})=>document.querySelector(selector).dispatchEvent(new KeyboardEvent(type,{code,key:code.startsWith('Key')?code.slice(3).toLowerCase():code,bubbles:true,cancelable:true})),{type,code,selector});
    await advance(p,type==='keydown'?.15:.1);
  }
}
async function point(p,button=null){
  await p.evaluate(button=>{
    const c=document.querySelector('#ember-root canvas'),r=c.getBoundingClientRect();
    const init={bubbles:true,cancelable:true,pointerType:'mouse',pointerId:1,isPrimary:true,clientX:r.left+r.width*.72,clientY:r.top+r.height*.47,button:-1,buttons:0};
    const move=new PointerEvent('pointermove',init);Object.defineProperty(move,'getCoalescedEvents',{value:()=>[move]});c.dispatchEvent(move);
    if(button!==null)c.dispatchEvent(new PointerEvent('pointerdown',{...init,button,buttons:button===2?2:1}));
  },button);
  await advance(p);
  if(button!==null){
    await p.evaluate(button=>document.querySelector('#ember-root canvas').dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerType:'mouse',pointerId:1,button,buttons:0})),button);
    await advance(p,.1);
  }
}
async function visible(p,id,expected=true){
  await p.waitForFunction(({id,expected})=>Boolean(document.getElementById(id)?.getClientRects().length)===expected,{id,expected},{timeout:15000});
}
async function v3Controls(p,commands){
  const bindings=await p.evaluate(()=>JSON.parse(window.proofWasm.bindings_json()));
  check(bindings.attackMove==='KeyA'&&bindings.stop==='KeyS','V3 publishes default A attack-move and S Stop bindings');
  const beforeRight=await snapshot(p),moveCount=commands.filter(c=>c.a==='move').length;
  await point(p,2);
  await p.waitForFunction(before=>{const s=JSON.parse(window.proofWasm.state_json());return Math.hypot(s.me.x-before.me.x,s.me.z-before.me.z)>.1;},beforeRight,{timeout:15000});
  check(commands.filter(c=>c.a==='move').length===moveCount+1,'one right-click moves through the authoritative Move command');
  await press(p,'KeyS');await advance(p);
  await point(p);
  const beforeAttackMove=await snapshot(p),attackMoves=commands.filter(c=>c.a==='attack_move').length;
  await press(p,'KeyA');
  await p.waitForFunction(before=>{const s=JSON.parse(window.proofWasm.state_json());return Math.hypot(s.me.x-before.me.x,s.me.z-before.me.z)>.1;},beforeAttackMove,{timeout:15000});
  check(commands.filter(c=>c.a==='attack_move').length===attackMoves+1,'one A press sends AttackMove and moves the authoritative champion');
  await press(p,'KeyS');await advance(p);
  await press(p,'Escape');await visible(p,'pause');
  await click(p,'#btn-help');await visible(p,'help');
  check(await p.evaluate(()=>{
    const help=document.getElementById('help');
    return help.innerText.includes('Right-click')&&help.innerText.includes('Attack-move')&&document.querySelectorAll('#guide-kits details').length===5&&document.querySelectorAll('#guide-kits article').length===20&&document.getElementById('guide-map').innerText.includes('core');
  }),'Escape guide contains controls, objectives, five champion kits and all 20 abilities');
  await capture(p,'public-guide');
  await press(p,'Escape','#help');await visible(p,'help',false);await visible(p,'pause');
  check(true,'Escape returns from the guide to the match menu');
  await click(p,'#btn-keys3');await visible(p,'keys');
  check(await p.evaluate(()=>{
    const row=document.querySelector('#key-rows .krow[data-act="attackMove"]');
    return row?.getClientRects().length&&/attack[ -]move/i.test(row.innerText)&&row.querySelector('.kset')&&row.querySelector('.kchip').textContent==='A';
  }),'Escape keybindings expose the bindable Attack-move action');
  await press(p,'Escape','#keys');await visible(p,'keys',false);await visible(p,'pause');
  await press(p,'Escape','#pause');await visible(p,'pause',false);await advance(p);
  check(true,'Escape backs out of keybindings and resumes the match');
}
async function main(){
  const {chromium}=require(process.env.EMBER_QA_PLAYWRIGHT||'playwright');
  os.setPriority(0,os.constants.priority.PRIORITY_LOW);
  fs.mkdirSync(out,{recursive:true});
  browser=await chromium.launch({channel:'msedge',headless:true,args:['--disable-webgpu','--disable-features=WebGPU','--enable-webgl','--ignore-gpu-blocklist']});
  const context=await browser.newContext({viewport:{width:1440,height:1000}});
  await context.addInitScript(()=>{
    window.focus=()=>{};Element.prototype.setPointerCapture=()=>{};
    // Retain only sockets this proof page itself used, for explicit room cleanup.
    window.__leagueProofSockets=[];
    const originalSend=WebSocket.prototype.send;
    WebSocket.prototype.send=function(data){
      const result=originalSend.call(this,data);
      if(!window.__leagueProofSockets.includes(this))window.__leagueProofSockets.push(this);
      return result;
    };
  });
  const hub=await context.newPage();
  hub.on('pageerror',error=>report.errors.push('hub: '+error.message));
  await hub.goto(hubUrl,{waitUntil:'domcontentloaded',timeout:45000});
  await hub.waitForFunction(expected=>{const card=document.querySelector('[data-game="league"]');return card?.querySelector('h3')?.textContent==='UltimateLegue'&&new URL(card.querySelector('select').value,document.baseURI).href===expected;},gameUrl,{timeout:30000});
  const link=await hub.evaluate(()=>new URL(document.querySelector('[data-game="league"] select').value,document.baseURI).href);
  check(link===gameUrl,`public hub lists UltimateLegue with live ${selected} selected`);
  report.hubGameLink=link;
  await capture(hub,'public-hub');
  await Promise.all([hub.waitForURL(gameUrl),click(hub,'[data-game="league"] button')]);
  check(hub.url()===gameUrl,'hub Play button opens the published League page');
  await hub.close();
  gamePage=await context.newPage();
  gamePage.on('pageerror',error=>report.errors.push('game: '+error.message));
  gamePage.on('response',response=>{if(response.status()>=400)report.errors.push(`HTTP ${response.status()}: ${response.url()}`);});
  const creates=[],joins=[],commands=[];
  gamePage.on('websocket',socket=>{
    socket.on('framesent',event=>{
      try{const message=JSON.parse(String(event.payload));if(message.t==='create_lobby')creates.push({name:message.name,mode:message.mode,passwordProtected:typeof message.password==='string'&&message.password.length>0});if(message.t==='cmd')commands.push({a:message.a});}catch{}
    });
    socket.on('framereceived',event=>{
      try{const message=JSON.parse(String(event.payload));if(message.t==='joined')joins.push({name:message.lobby,mode:message.mode,slot:message.id});}catch{}
    });
  });
  await gamePage.goto(gameUrl,{waitUntil:'domcontentloaded',timeout:45000});
  await gamePage.waitForFunction(()=>['ready','failed'].includes(document.body.dataset.leagueBoot),null,{timeout:45000});
  await gamePage.evaluate(async()=>{
    window.proofWasm=await window.leagueReady;
    if(!window.proofWasm)throw new Error(document.getElementById('engine-note').textContent);
  });
  report.proto=await gamePage.evaluate(()=>window.proofWasm.proto_version());
  check(typeof(await snapshot(gamePage)).phase==='string','public game exposes an initialized WASM API');
  await gamePage.waitForFunction(()=>!document.getElementById('btn-create').disabled,null,{timeout:30000});
  check(await gamePage.locator('#menu').isVisible(),'public menu is visible and an online host is available');
  await capture(gamePage,'public-menu');
  await gamePage.evaluate(({name,password})=>{
    document.getElementById('handle').value='release-proof';
    document.getElementById('newlobby').value=name;
    document.getElementById('newmode').value='3';
    document.getElementById('newpass').value=password;
  },{name:lobbyName,password:lobbyPassword});
  await click(gamePage,'#btn-create');
  await gamePage.waitForFunction(()=>{const s=JSON.parse(window.proofWasm.state_json());return s.connected&&s.phase==='select'&&s.roster.length===6;},null,{timeout:30000});
  check(creates.length===1&&creates[0].name===lobbyName&&creates[0].mode===3&&creates[0].passwordProtected,'proof creates one unique password-protected 3v3 lobby');
  check(joins.length===1&&joins[0].name===lobbyName&&joins[0].slot===0,'proof joins only its own new lobby as host');
  report.lobby=lobbyName;
  await click(gamePage,'#cards [data-c="1"]');
  await gamePage.waitForFunction(()=>{const s=JSON.parse(window.proofWasm.state_json());const r=s.roster.find(r=>r.slot===s.slot);return r?.picked&&r.champ===1;});
  check(true,'EmberKnight selection reaches the authoritative roster');
  await click(gamePage,'#btn-start');
  await gamePage.waitForFunction(()=>{const s=JSON.parse(window.proofWasm.state_json());return s.connected&&s.phase==='live'&&s.me?.champ===1;},null,{timeout:30000});
  const live=await snapshot(gamePage);
  check(live.mode===3&&live.roster.length===6&&live.roster.filter(r=>!r.bot).length===1,'3v3 match starts with the proof player and five bots');
  check(live.cores.length===2&&live.cores.every(c=>c.hp>0),'live HUD receives both core health values');
  await click(gamePage,'#abils [data-abil="0"] .up');
  await gamePage.waitForFunction(()=>JSON.parse(window.proofWasm.state_json()).me.rk[0]===1);
  check(true,'Q learns through the public HUD');
  if(Number(selected.slice(1))>=3)await v3Controls(gamePage,commands);
  await gamePage.evaluate(()=>{
    const c=document.querySelector('#ember-root canvas'),r=c.getBoundingClientRect();
    const event=new PointerEvent('pointermove',{bubbles:true,cancelable:true,pointerType:'mouse',pointerId:1,isPrimary:true,clientX:r.left+r.width*.62,clientY:r.top+r.height*.43,button:-1,buttons:0});
    Object.defineProperty(event,'getCoalescedEvents',{value:()=>[event]});c.dispatchEvent(event);
  });
  await advance(gamePage);
  await press(gamePage,'KeyQ');
  await gamePage.waitForFunction(()=>JSON.parse(window.proofWasm.state_json()).me.cd[0]>0);
  const after=await snapshot(gamePage);
  check(after.connected&&after.phase==='live'&&after.me.rk[0]===1&&after.me.cd[0]>0,'EmberKnight Q casts and the authoritative match stays connected');
  await gamePage.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
  await capture(gamePage,'public-emberknight-live');
  report.finalState={mode:after.mode,slot:after.slot,phase:after.phase,connected:after.connected,champ:after.me.champ,level:after.me.lv,qRank:after.me.rk[0],qCooldown:after.me.cd[0]};
  check(report.errors.length===0,'public hub and game have no uncaught browser errors');
}
function run(){return main().catch(error=>{report.failure=error.stack;console.error(error);process.exitCode=1;}).finally(async()=>{
  if(report.failure&&gamePage&&!gamePage.isClosed())try{await capture(gamePage,'public-failure');}catch{}
  if(gamePage&&!gamePage.isClosed()){
    try{
      await gamePage.evaluate(async()=>{
        await Promise.all((window.__leagueProofSockets||[]).filter(socket=>socket.readyState===WebSocket.OPEN).map(socket=>new Promise(resolve=>{
          const timer=setTimeout(resolve,1500);
          socket.addEventListener('close',()=>{clearTimeout(timer);resolve();},{once:true});
          socket.send(JSON.stringify({t:'leave_lobby'}));
          socket.close(1000,'Release proof complete');
        })));
      });
      report.cleanup='Sent LeaveLobby and closed proof page sockets';
    }catch(error){report.errors.push('cleanup: '+error.message);}
  }
  if(browser)try{await browser.close();}catch(error){report.errors.push('browser cleanup: '+error.message);}
  report.elapsedSeconds=(Date.now()-started)/1000;
  report.passed=!report.failure&&report.errors.length===0&&report.checks.length>0;
  if(!report.passed)process.exitCode=1;
  fs.mkdirSync(out,{recursive:true});fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(report,null,2));
  console.log(JSON.stringify(report,null,2));
});
}
module.exports={proofBaseUrl};
if(require.main===module)run();
