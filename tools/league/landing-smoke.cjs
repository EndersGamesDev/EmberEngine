// Actual landing/story/media over a disposable loopback preview. No game server,
// mocked responses, native input, public writes or worker generation jobs.
// LEAGUE_LANDING_BASE_URL=https://.../games/league/ skips the local preview and
// writes target/league-landing-public instead of target/league-landing-smoke.
'use strict';
const fs=require('node:fs'),path=require('node:path'),os=require('node:os');
const assert=require('node:assert/strict');
const {chromium}=require(process.env.EMBER_QA_PLAYWRIGHT||'playwright');
const {startPreview}=require('./preview.cjs');
const {hash}=require('./publish.cjs');
const root=path.resolve(__dirname,'../..'),directory=path.join(root,'web/games/league');
const publicBase=process.env.LEAGUE_LANDING_BASE_URL?.trim();
const out=path.join(root,publicBase?'target/league-landing-public':'target/league-landing-smoke'),started=Date.now();
const report={kind:'actual-landing-and-media',mode:publicBase?'public':'local',checks:[],errors:[],screenshots:[],passed:false};
let preview,browser,landingUrl;
const contexts=[];
const normalize=value=>value.replace(/\s+/g,' ').trim();
function check(condition,message){assert(condition,message);report.checks.push(message);console.log('PASS '+message);}
async function shot(page,name){const file=path.join(out,name+'.png');await page.screenshot({path:file});report.screenshots.push(file);}
async function noOverflow(page,label){
  const geometry=await page.evaluate(()=>({viewport:innerWidth,html:document.documentElement.scrollWidth,body:document.body.scrollWidth}));
  check(geometry.html<=geometry.viewport+1&&geometry.body<=geometry.viewport+1,`${label}: no horizontal overflow`);
}
async function inView(page,selector){
  await page.locator(selector).evaluate(element=>element.scrollIntoView({behavior:'instant',block:'center'}));
  await page.waitForFunction(selector=>{
    const element=document.querySelector(selector),rect=element.getBoundingClientRect();
    return rect.bottom>0&&rect.top<innerHeight&&Number(getComputedStyle(element).opacity)>.99;
  },selector);
}
async function page(options={}){
  const context=await browser.newContext({viewport:{width:1440,height:1000},...options});contexts.push(context);
  const page=await context.newPage();page.setDefaultTimeout(15000);
  page.on('pageerror',error=>report.errors.push(error.message));
  page.on('response',response=>{if(response.status()>=400)report.errors.push(`HTTP ${response.status()}: ${response.url()}`);});
  page.on('requestfailed',request=>{if(request.failure()?.errorText!=='net::ERR_ABORTED')report.errors.push(`${request.failure()?.errorText}: ${request.url()}`);});
  await page.addInitScript(()=>{window.focus=()=>{};});
  await page.goto(landingUrl,{waitUntil:'load'});
  return page;
}
async function links(page,label){
  const targets=await page.locator('a').evaluateAll(elements=>elements.filter(element=>/^Play\b/i.test(element.textContent.trim())).map(element=>({text:element.textContent.trim(),url:element.href})));
  check(targets.length>=3&&targets.every(target=>target.url===new URL('v3/',landingUrl).href),`${label}: every Play link points to v3`);
  const hubs=await page.locator('a.brand, .site-foot a').evaluateAll(elements=>elements.filter(element=>element.classList.contains('brand')||/all ember games/i.test(element.textContent)).map(element=>({url:element.href,relative:element.getAttribute('href')})));
  check(hubs.length>=2&&hubs.every(hub=>hub.url===new URL('../../',landingUrl).href&&!/^(?:\.\.\/){3}/.test(hub.relative)),`${label}: hub links retain the GitHub Pages project prefix`);
}
async function images(page,label){
  const all=page.locator('img[src]');
  for(let index=0;index<await all.count();index++){
    const element=all.nth(index);
    if(!await element.isVisible())continue;
    await element.evaluate(image=>image.scrollIntoView({behavior:'instant',block:'center'}));
    await element.evaluate(image=>image.decode());
  }
  check(await all.evaluateAll(elements=>elements.filter(image=>image.getBoundingClientRect().width>0).every(image=>image.complete&&image.naturalWidth>0)),`${label}: visible images decode successfully`);
}
async function reader(page,champion,label,{keyboard=false}={}){
  const card=`#champ-grid .champ[data-key="${champion.id}"]`;
  await inView(page,card);
  if(keyboard){
    await page.locator(card).focus();
    check(await page.locator(card).evaluate(element=>element.tagName==='BUTTON'&&!element.disabled&&element.tabIndex>=0),`${label}: champion is a keyboard-operable button`);
    // Playwright sends this only to its own headless page, never to the desktop.
    // Unlike dispatchEvent(KeyboardEvent), this exercises native button activation.
    await page.keyboard.press('Enter');
  }else await page.locator(card).evaluate(element=>element.click());
  await page.waitForFunction(()=>!document.querySelector('#reader').classList.contains('hidden'));
  const text=normalize(await page.locator('#reader').innerText());
  for(const paragraph of champion.origin)assert(text.includes(normalize(paragraph)),`${label}: missing origin paragraph for ${champion.name}`);
  check(text.includes(normalize(champion.motivation))&&text.includes(normalize(champion.bond.text)),`${label}: ${champion.name} shows full origin, motivation and bond`);
  check(await page.locator('#reader').evaluate(element=>element.contains(document.activeElement)),`${label}: opening the reader moves focus into it`);
  await page.waitForFunction(()=>{const rect=document.querySelector('#reader-name').getBoundingClientRect();return rect.top>=0&&rect.bottom<=innerHeight;});
  check(true,`${label}: reader heading scrolls into view`);
  const image=page.locator('#reader-img');
  await image.evaluate(element=>element.decode());
  check(await image.evaluate((element,expected)=>element.naturalWidth>0&&element.src===new URL(expected,location.href).href,champion.storyImage||champion.portrait),`${label}: reader loads ${champion.name}'s actual story image`);
  await noOverflow(page,`${label} reader`);
  if(keyboard)await shot(page,`${label}-reader`);
  await page.keyboard.press('Escape');
  check(await page.locator('#reader').evaluate(element=>element.classList.contains('hidden'))&&await page.locator(card).evaluate(element=>element===document.activeElement&&element.getAttribute('aria-expanded')==='false'),`${label}: Escape closes reader and restores card focus`);
}
async function chapters(page,story,label){
  check(await page.locator('#chapters .chapter').count()===3,`${label}: three story chapters are rendered`);
  for(let index=0;index<story.chapters.length;index++){
    const chapter=story.chapters[index],selector=`#chapters .chapter:nth-child(${index+1})`;
    await inView(page,selector);
    const text=normalize(await page.locator(selector).innerText());
    check(text.includes(chapter.title)&&chapter.body.every(paragraph=>text.includes(normalize(paragraph))),`${label}: complete chapter ${chapter.number} — ${chapter.title}`);
  }
}
async function features(page,story,label){
  check(await page.locator('#features-title').isVisible(),`${label}: features heading remains visible`);
  const entries=await page.locator('#features .features > li').allInnerTexts();
  check(entries.length===6&&story.features.every((feature,index)=>normalize(entries[index]).includes(normalize(feature.title))&&normalize(entries[index]).includes(normalize(feature.text))),`${label}: all six canonical feature descriptions are rendered`);
}
async function trailer(page){
  await inView(page,'#trailer-video');
  const initial=await page.locator('#trailer-video').evaluate(v=>({time:v.currentTime,paused:v.paused,autoplay:v.autoplay,controls:v.controls,frames:v.getVideoPlaybackQuality().totalVideoFrames}));
  report.trailer=initial;
  check(initial.controls&&!initial.autoplay&&initial.paused&&initial.time<.1,'trailer provides native controls and never autoplays');
  await page.locator('#trailer-video').evaluate(async v=>{v.muted=true;if(v.textTracks[0])v.textTracks[0].mode='showing';await v.play();});
  await page.waitForFunction(()=>{const v=document.querySelector('#trailer-video');return v.readyState>=2&&Number.isFinite(v.duration);});
  Object.assign(report.trailer,await page.locator('#trailer-video').evaluate(v=>({duration:v.duration,source:v.currentSrc})));
  check(report.trailer.duration>35&&report.trailer.duration<=45,'native trailer duration is 35–45 seconds');
  await page.waitForFunction(before=>{const v=document.querySelector('#trailer-video');return v.currentTime>before.time+.7&&v.getVideoPlaybackQuality().totalVideoFrames>before.frames+3;},initial);
  const playing=await page.locator('#trailer-video').evaluate(v=>({time:v.currentTime,frames:v.getVideoPlaybackQuality().totalVideoFrames}));
  check(playing.time>initial.time+.7&&playing.frames>initial.frames+3,'trailer plays real decoded frames with advancing currentTime');
  check(!await page.locator('#trailer-note').isVisible(),'successful playback keeps the trailer failure message hidden');
  await page.waitForFunction(()=>{const v=document.querySelector('#trailer-video'),track=v.querySelector('track');return track?.readyState===2&&v.textTracks[0]?.cues?.length>0;});
  const captions=await page.locator('#trailer-video').evaluate(v=>Array.from(v.textTracks[0].cues).map(cue=>({start:cue.startTime,end:cue.endTime,text:cue.text})));
  check(captions.length>0&&captions.every(cue=>cue.text.trim()&&cue.start>=0&&cue.end>cue.start&&cue.end<=report.trailer.duration+.5),'real caption track loads valid timed cues');
  report.trailer.captions=captions;report.trailer.playing=playing;
  await shot(page,'desktop-trailer-playing');
  report.trailer.seek=await page.locator('#trailer-video').evaluate(async v=>{
    v.pause();
    await new Promise((resolve,reject)=>{const timeout=setTimeout(()=>reject(new Error('trailer seek timed out')),10000);v.addEventListener('seeked',()=>{clearTimeout(timeout);resolve();},{once:true});v.currentTime=v.duration-.7;});
    const result={time:v.currentTime,target:v.duration-.7,seekable:Array.from({length:v.seekable.length},(_,index)=>({start:v.seekable.start(index),end:v.seekable.end(index)}))};
    await v.play();
    return result;
  });
  check(Math.abs(report.trailer.seek.time-report.trailer.seek.target)<.2,'native seeking reaches the requested near-end timestamp');
  await page.waitForFunction(()=>document.querySelector('#trailer-video').ended);
  check(await page.locator('#trailer-video').evaluate(v=>v.currentTime>=v.duration-.1&&!v.error),'trailer seeks near the end and finishes successfully');
}
async function main(){
  os.setPriority(0,os.constants.priority.PRIORITY_LOW);fs.mkdirSync(out,{recursive:true});
  const storyBytes=fs.readFileSync(path.join(directory,'story.json')),story=JSON.parse(storyBytes);
  check(story.champions.length===5&&new Set(story.champions.map(champion=>champion.id)).size===5,'real story has five distinct champions');
  check(story.champions.every(champion=>Array.isArray(champion.origin)&&champion.origin.length>=3&&champion.motivation&&champion.bond?.text)&&story.chapters.length===3&&story.chapters.every(chapter=>chapter.body.length>=3),'real story contains complete origins, motivations, bonds and three full chapters');
  // These identify the local release artifacts this run expects; public byte
  // equivalence is a separate release gate, not inferred from video playback.
  report.sourceStorySha256=hash(storyBytes);report.sourceTrailerSha256=hash(fs.readFileSync(path.join(directory,'media/trailer.mp4')));
  if(publicBase){
    const parsed=new URL(publicBase);
    assert(['http:','https:'].includes(parsed.protocol)&&!parsed.username&&!parsed.password&&!parsed.search&&!parsed.hash&&parsed.pathname.endsWith('/'),'LEAGUE_LANDING_BASE_URL must be an HTTP(S) directory URL without credentials, query or fragment');
    landingUrl=parsed.href;
  }else{
    preview=await startPreview({gameVersion:'v3',online:false,port:8106});
    landingUrl=`${preview.origin}/games/league/`;
  }
  report.url=landingUrl;
  browser=await chromium.launch({channel:'msedge',headless:true});
  const desktop=await page();
  await desktop.waitForFunction(title=>document.querySelector('#chapters .chapter h3')?.textContent.trim()===title,story.chapters[0].title);
  await noOverflow(desktop,'desktop');await links(desktop,'desktop');await shot(desktop,'desktop-hero');
  check(await desktop.locator('#champ-grid .champ').count()===5,'desktop: all five champion cards are present');
  for(let index=0;index<story.champions.length;index++)await reader(desktop,story.champions[index],'desktop',{keyboard:index===0});
  await chapters(desktop,story,'desktop');await shot(desktop,'desktop-chapter');await features(desktop,story,'desktop');await trailer(desktop);await images(desktop,'desktop');
  const mobile=await page({viewport:{width:390,height:844}});
  await mobile.waitForFunction(title=>document.querySelector('#chapters .chapter h3')?.textContent.trim()===title,story.chapters[0].title);
  await noOverflow(mobile,'mobile 390px');await links(mobile,'mobile 390px');await shot(mobile,'mobile-hero');
  for(let index=0;index<story.champions.length;index++)await reader(mobile,story.champions[index],'mobile',{keyboard:index===0});
  await chapters(mobile,story,'mobile');await features(mobile,story,'mobile');await noOverflow(mobile,'mobile complete story');await images(mobile,'mobile');
  const reduced=await page({reducedMotion:'reduce',viewport:{width:390,height:844}});
  check(await reduced.evaluate(()=>matchMedia('(prefers-reduced-motion: reduce)').matches&&getComputedStyle(document.documentElement).scrollBehavior==='auto'&&Array.from(document.querySelectorAll('.fade')).every(element=>getComputedStyle(element).opacity==='1'&&getComputedStyle(element).animationName==='none')),'reduced motion keeps content visible without reveal animation or smooth scrolling');
  await noOverflow(reduced,'reduced motion');await links(reduced,'reduced motion');
  const nojs=await page({javaScriptEnabled:false,viewport:{width:390,height:844}});
  await noOverflow(nojs,'no JavaScript');await links(nojs,'no JavaScript');
  check(await nojs.locator('#hero-title').isVisible()&&await nojs.locator('#champ-grid .champ').count()===5&&await nojs.locator('#chapters .chapter').count()===3,'no JavaScript retains the hero, five champions and three readable chapters');
  check(await nojs.locator('#chapters').evaluate(element=>getComputedStyle(element).display!=='none'&&element.innerText.trim().length>600)&&await nojs.locator('#trailer-video').evaluate(v=>v.controls&&!v.autoplay),'no JavaScript retains substantial story and the native video controls');
  await chapters(nojs,story,'no JavaScript');
  await features(nojs,story,'no JavaScript');
  await images(nojs,'no JavaScript');await inView(nojs,'#story');await shot(nojs,'nojs-story');
  check(hash(fs.readFileSync(path.join(directory,'story.json')))===report.sourceStorySha256&&hash(fs.readFileSync(path.join(directory,'media/trailer.mp4')))===report.sourceTrailerSha256,'source story and trailer remain unchanged throughout this proof');
  check(report.errors.length===0,'no uncaught errors, missing assets or failed requests');report.passed=true;
}
main().catch(error=>{report.failure=error.stack;console.error(error);process.exitCode=1;}).finally(async()=>{
  if(report.failure&&browser){
    report.mediaFailures=[];
    for(const [index,page] of contexts.flatMap(context=>context.pages()).entries()){
      try{
        report.mediaFailures.push(await page.locator('#trailer-video').evaluate(v=>({url:location.href,source:v.currentSrc,readyState:v.readyState,networkState:v.networkState,time:v.currentTime,duration:Number.isFinite(v.duration)?v.duration:null,error:v.error?{code:v.error.code,message:v.error.message}:null,tracks:Array.from(v.querySelectorAll('track')).map(track=>({src:track.src,state:track.readyState}))})));
        await shot(page,`failure-${index}`);
      }catch{}
    }
  }
  await browser?.close();await preview?.close();
  report.elapsedSeconds=(Date.now()-started)/1000;fs.mkdirSync(out,{recursive:true});fs.writeFileSync(path.join(out,'results.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
});
