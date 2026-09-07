// Exact public bytes for the complete selected release, then real public matches.
'use strict';
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {execFileSync}=require('node:child_process');
const {gameVersion,safeRelative,treeFiles,proveScope,hash}=require('./publish.cjs');
const base='https://endersgamesdev.github.io/EmberEngine/';
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
function frozenArtifacts(root,revision,selected,scope,previous){
  const versions=scope.frozenVersions.filter(row=>/^games\/league\/v[1-9][0-9]{0,5}$/.test(row.path));
  for(const row of versions)assert.equal(row.type,'tree',`Frozen version is not a directory: ${row.path}`);
  for(const old of previous.games.find(game=>game.id==='league')?.versions||[]){
    const version=gameVersion(old.v);
    assert.equal(old.path,`games/league/${version}/`,'Unexpected frozen catalog path');
    if(version!==selected)assert(versions.some(row=>row.path===old.path.slice(0,-1)),`Frozen ${version} artifact proof is missing`);
  }
  return versions.flatMap(row=>{
    const files=treeFiles(root,revision,row.path).map(file=>file.file);
    assert(files.includes(`${row.path}/index.html`)&&files.includes(`${row.path}/pkg/league_bg.wasm`),`Frozen artifact proof is incomplete: ${row.path}`);
    return files;
  }).sort();
}
function verifyHost(book,report,version){
  const protocol=version.proto,stamp=report.liveWelcome.commit;
  assert(Number.isInteger(protocol)&&protocol>0,'Invalid selected protocol');
  assert.equal(report.liveWelcome.proto,protocol,'Proven server protocol differs from selected catalog version');
  assert(/^[0-9a-f]{7,40}$/.test(stamp)&&report.sourceCommit.startsWith(stamp),'Invalid live build stamp');
  const hosts=book.hosts.filter(value=>value.name===report.host);
  assert.equal(hosts.length,1,'Public host is missing or duplicated');
  const host=hosts[0];
  assert(host.league_ws&&host.league_commit===stamp,'Public host book is stale');
  assert.equal(host.league_proto,protocol,'Public host protocol is stale');
  return host;
}
async function get(file){
  safeRelative(file);
  const url=new URL(file,base);url.searchParams.set('proof',String(Date.now()));
  const response=await fetch(url,{cache:'no-store',signal:AbortSignal.timeout(20000)});
  assert(response.ok,`${file}: HTTP ${response.status}`);return Buffer.from(await response.arrayBuffer());
}
async function main(){
  os.setPriority(0,os.constants.priority.PRIORITY_LOW);
  const root=process.cwd(),started=Date.now(),report=JSON.parse(fs.readFileSync('target/league-publish/results.json'));
  const selected=gameVersion(report.gameVersion||'v1'),entry=`games/league/${selected}`;
  if(process.env.LEAGUE_GAME_VERSION)assert.equal(gameVersion(process.env.LEAGUE_GAME_VERSION),selected,'Requested version differs from the publish report');
  assert(report.pushed&&report.pagesCommit,'No successful Pages push recorded');
  for(const revision of [report.pagesCommit,report.base,report.sourceCommit])assert(/^[0-9a-f]{40,64}$/.test(revision),'Invalid release revision');
  if(report.entry)assert.equal(report.entry,entry,'Publish report has a mismatched entry');
  const git=(...args)=>execFileSync('git',['-C',root,...args],{windowsHide:true,maxBuffer:64*1024*1024});
  const scope=proveScope(root,report.base,report.pagesCommit,entry);
  const release=treeFiles(root,report.pagesCommit,entry).map(row=>row.file).sort();
  for(const required of ['index.html','version.json','pkg/league.js','pkg/league_bg.wasm'])assert(release.includes(`${entry}/${required}`),`Missing published artifact: ${required}`);
  const paths=['games.json',...release];
  if(report.files){
    assert.deepEqual(report.files.map(row=>row.file).sort(),[...paths].sort(),'Release manifest differs from published tree');
    for(const row of report.files)assert.equal(hash(git('show',`${report.pagesCommit}:${row.file}`)),row.sha256,`Publish report hash differs: ${row.file}`);
  }
  const previous=JSON.parse(git('show',`${report.base}:games.json`));
  const frozen=frozenArtifacts(root,report.base,selected,scope,previous),frozenSet=new Set(frozen);
  const results=[],frozenFiles=[],bytes=new Map();
  // Fetch every image, CSS, JS and sidecar, not just the entry page and bundle.
  // Fetch every frozen version too: unchanged Git trees do not prove CDN bytes.
  for(const file of [...paths,...frozen]){
    const revision=frozenSet.has(file)?report.base:report.pagesCommit;
    const expected=hash(git('show',`${revision}:${file}`));let actual,download;
    for(let attempt=0;attempt<8;attempt++){
      try{download=await get(file);actual=hash(download);if(actual===expected)break;}catch(error){if(attempt===7)throw error;}
      if(attempt<7)await sleep(10000);
    }
    assert.equal(actual,expected,`${file}: public bytes differ`);bytes.set(file,download);
    (frozenSet.has(file)?frozenFiles:results).push({file,sha256:actual});console.log('VERIFIED '+file);
  }
  const catalog=JSON.parse(bytes.get('games.json'));
  assert.deepEqual(catalog.games.filter(game=>game.id!=='league'),previous.games.filter(game=>game.id!=='league'));
  const current=catalog.games.find(game=>game.id==='league');
  const version=current?.versions.find(version=>version.v===selected&&version.path===`${entry}/`&&version.live);
  assert(version,'Selected version is not published live');
  for(const old of previous.games.find(game=>game.id==='league')?.versions||[])assert(current.versions.some(version=>version.v===old.v&&version.path===old.path),'A frozen League catalog version disappeared');
  const book=JSON.parse(await get('server.json')),host=verifyHost(book,report,version),stamp=report.liveWelcome.commit;
  const probes=[];
  for(const mode of [1,3]){
    const log=execFileSync(path.join(root,'target/release/examples/wsprobe.exe'),[host.league_ws,`public-${selected}-${Date.now()}-${mode}`,'--mode',String(mode),'--expect-commit',stamp],{windowsHide:true,timeout:35000}).toString();
    assert(log.includes(`wsprobe: league protocol v${version.proto} healthy`),'Probe protocol differs from selected release');
    probes.push({mode,log});console.log(log.trim());
  }
  const proof={passed:true,gameVersion:selected,sourceCommit:report.sourceCommit,pagesCommit:report.pagesCommit,url:base+entry+'/',files:results,frozenFiles,frozenVersions:scope.frozenVersions,probes,peerTreesUnchanged:true,unchangedOutsideRelease:true,elapsedSeconds:(Date.now()-started)/1000};
  fs.writeFileSync('target/league-publish/public-proof.json',JSON.stringify(proof,null,2)+'\n');console.log(JSON.stringify(proof,null,2));
}
module.exports={frozenArtifacts,verifyHost};
if(require.main===module)main().catch(error=>{console.error(error);process.exitCode=1;});
