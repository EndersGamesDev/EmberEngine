// Exact public bytes + real public matches, after the scoped publisher succeeds.
'use strict';
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const {execFileSync}=require('node:child_process');
const root=process.cwd(),started=Date.now(),report=JSON.parse(fs.readFileSync('target/league-publish/results.json'));
const base='https://endersgamesdev.github.io/EmberEngine/';
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const git=(...a)=>execFileSync('git',['-C',root,...a],{windowsHide:true,maxBuffer:64*1024*1024});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function get(file){const r=await fetch(base+file+'?proof='+Date.now(),{cache:'no-store',signal:AbortSignal.timeout(20000)});assert(r.ok,`${file}: HTTP ${r.status}`);return Buffer.from(await r.arrayBuffer());}
async function main(){
  assert(report.pushed&&report.pagesCommit,'No successful Pages push recorded');
  const paths=['games/league/v1/index.html','games/league/v1/version.json','games/league/v1/pkg/league.js','games/league/v1/pkg/league_bg.wasm'];
  const results=[];
  for(const file of paths){
    const expected=hash(git('show',`${report.pagesCommit}:${file}`));let actual;
    for(let attempt=0;attempt<8;attempt++){
      try{actual=hash(await get(file));if(actual===expected)break;}catch(e){if(attempt===7)throw e;}
      await sleep(10000);
    }
    assert.equal(actual,expected,`${file}: public bytes differ`);results.push({file,sha256:actual});console.log('VERIFIED '+file);
  }
  const catalog=JSON.parse(await get('games.json')),publishedCatalog=JSON.parse(git('show',`${report.pagesCommit}:games.json`));
  assert.deepEqual(catalog.games.find(g=>g.id==='league'),publishedCatalog.games.find(g=>g.id==='league'));
  for(const game of publishedCatalog.games.filter(g=>g.id!=='league'))assert.deepEqual(catalog.games.find(g=>g.id===game.id),game,`Peer catalog changed during release: ${game.id}`);
  const previous=JSON.parse(git('show',`${report.base}:games.json`));
  assert.deepEqual(publishedCatalog.games.filter(g=>g.id!=='league'),previous.games.filter(g=>g.id!=='league'));
  for(const prefix of ['games/arena','games/fire','games/kings','games/what-is-this','labs','index.html','hosts.js']){
    assert.equal(git('rev-parse',`${report.base}:${prefix}`).toString(),git('rev-parse',`${report.pagesCommit}:${prefix}`).toString(),`Peer bytes changed: ${prefix}`);
  }
  const book=JSON.parse(await get('server.json')),host=book.hosts.find(h=>h.name===report.host);
  const stamp=report.liveWelcome.commit;
  assert(/^[0-9a-f]{7,40}$/.test(stamp)&&report.sourceCommit.startsWith(stamp),'Invalid live build stamp');
  assert(host?.league_ws&&host.league_commit===stamp,'Public host book is stale');
  const probes=[];
  for(const mode of [1,3]){
    const log=execFileSync(path.join(root,'target/release/examples/wsprobe.exe'),[host.league_ws,'public-'+Date.now()+'-'+mode,'--mode',String(mode),'--expect-commit',stamp],{windowsHide:true,timeout:35000}).toString();
    probes.push({mode,log});console.log(log.trim());
  }
  const proof={passed:true,sourceCommit:report.sourceCommit,pagesCommit:report.pagesCommit,url:base+'games/league/v1/',files:results,probes,peerTreesUnchanged:true,elapsedSeconds:(Date.now()-started)/1000};
  fs.writeFileSync('target/league-publish/public-proof.json',JSON.stringify(proof,null,2)+'\n');console.log(JSON.stringify(proof,null,2));
}
main().catch(e=>{console.error(e);process.exitCode=1;});
