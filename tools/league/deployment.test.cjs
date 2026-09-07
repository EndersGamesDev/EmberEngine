// Offline fixtures only: no public origin, scheduled task, server or tunnel.
'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {execFileSync}=require('node:child_process');
const {parseArgs,validateTarget}=require('./publish-host.cjs');
const {frozenArtifacts,verifyHost}=require('./prove-live.cjs');
const {proveScope}=require('./publish.cjs');
const root=path.resolve(__dirname,'../..'),stamp='a'.repeat(40);
os.setPriority(0,os.constants.priority.PRIORITY_LOW);
const git=(cwd,...args)=>execFileSync('git',['-c','core.autocrlf=false','-C',cwd,...args],{windowsHide:true,stdio:'pipe',env:{...process.env,GIT_ALLOW_PROTOCOL:'file'}}).toString().trim();
function write(root,file,content){const target=path.join(root,file);fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,content);}
function cleanup(target,prefix){
  const absolute=path.resolve(target),parent=path.resolve(os.tmpdir());
  assert.equal(path.dirname(absolute),parent);assert(path.basename(absolute).startsWith(prefix));
  fs.rmSync(absolute,{recursive:true,force:true});
}
function fixture(t){
  const temp=fs.mkdtempSync(path.join(os.tmpdir(),'league-deploy-test-'));
  t.after(()=>cleanup(temp,'league-deploy-test-'));
  git(temp,'init','-q');git(temp,'config','user.name','League fixture');git(temp,'config','user.email','fixture@example.invalid');
  return temp;
}
const baseArgs=['wss://league.example','r1700',stamp];
test('legacy publisher CLI defaults and explicit protocol 2',()=>{
  assert.deepEqual(parseArgs(baseArgs),{url:baseArgs[0],version:'r1700',commit:stamp,name:'dusky-osprey',protocol:1,prepare:false});
  assert.equal(parseArgs([...baseArgs,'dusky-osprey','--prepare']).prepare,true);
  assert.equal(parseArgs([...baseArgs,'dusky-osprey-league-v3','--prepare','--protocol=2']).protocol,2);
  for(const args of [['--protocol=0'],['--protocol=-1'],['--protocol=65536'],['--protocol=2','--protocol=1'],['--prepare','--prepare'],['--unknown']])assert.throws(()=>parseArgs([...baseArgs,...args]));
});
test('protocol 2 cannot replace the protocol 1 host',()=>{
  const book={hosts:[{name:'dusky-osprey',league_ws:'wss://old.example',league_proto:1}]};
  assert.doesNotThrow(()=>validateTarget(book,{name:'dusky-osprey',protocol:1}));
  assert.throws(()=>validateTarget(book,{name:'dusky-osprey',protocol:2}),/separate host/);
  assert.doesNotThrow(()=>validateTarget(book,{name:'dusky-osprey-league-v3',protocol:2}));
});
test('canonical writer creates independent v3 while preserving peer book fields',t=>{
  const temp=fixture(t),remote=path.join(temp,'remote.git'),repo=path.join(temp,'repo');
  git(temp,'init','--bare','-q',remote);git(temp,'init','-q',repo);
  git(repo,'config','user.name','League fixture');git(repo,'config','user.email','fixture@example.invalid');
  git(repo,'checkout','-q','-b','gh-pages');
  const before={v:'old',proto:22,ws:'wss://pinned-arena.example',fire_proto:1,fire_ws:'wss://pinned-fire.example',league_proto:1,league_ws:'wss://old.example',mirrors:['https://mirror.example/'],custom:{keep:true},hosts:[
    {name:'dusky-osprey',league_ws:'wss://old.example',league_proto:1,league_version:'r1590',league_commit:'b'.repeat(40),ws:'wss://arena.example',proto:22,version:'r1590',fire_ws:'wss://fire.example',fire_proto:1,updated:'old'},
    {name:'another-host',ws:'wss://another.example',proto:22,version:'r1600',custom:'preserve'}]};
  write(repo,'server.json',JSON.stringify(before));write(repo,'unrelated.txt','peer bytes');
  for(const file of ['tools/league/publish-host.cjs','deploy/publish-host.sh'])write(repo,file,fs.readFileSync(path.join(root,file)));
  git(repo,'add','.');git(repo,'commit','-qm','fixture');git(repo,'remote','add','origin',remote);git(repo,'push','-q','origin','gh-pages');
  const base=git(repo,'rev-parse','HEAD'),run=args=>{
    const output=execFileSync(process.execPath,[path.join(repo,'tools/league/publish-host.cjs'),...args],{cwd:repo,windowsHide:true,stdio:'pipe',env:{...process.env,GIT_ALLOW_PROTOCOL:'file'}}).toString();
    const report=JSON.parse(output.trim().split('\n').at(-1));
    t.after(()=>{const parent=path.dirname(report.worktree);assert.equal(path.basename(report.worktree),'pages');cleanup(parent,'ember-league-book-');});
    return {report,book:JSON.parse(fs.readFileSync(path.join(report.worktree,'server.json')))};
  };
  const v3=run([...baseArgs,'dusky-osprey-league-v3','--protocol=2','--prepare']);
  assert.equal(v3.report.protocol,2);assert.equal(v3.report.preparedOnly,true);
  assert.deepEqual(v3.book.hosts.slice(0,2),before.hosts);
  for(const key of Object.keys(before).filter(key=>!['v','hosts'].includes(key)))assert.deepEqual(v3.book[key],before[key],key);
  assert.deepEqual(v3.book.hosts[2],{name:'dusky-osprey-league-v3',league_ws:baseArgs[0],league_proto:2,league_version:'r1700',league_commit:stamp,updated:v3.book.hosts[2].updated});
  assert.equal(git(repo,'ls-remote','origin','refs/heads/gh-pages').split(/\s/)[0],base,'prepare must not publish');
  assert.equal(git(v3.report.worktree,'diff','--cached','--name-only'),'server.json');
  const legacy=run([...baseArgs,'dusky-osprey','--prepare']);
  assert.equal(legacy.book.hosts[0].league_proto,1);assert.equal(legacy.book.ws,before.ws);assert.equal(legacy.book.fire_ws,before.fire_ws);
  assert.deepEqual(legacy.book.hosts[1],before.hosts[1]);assert.equal(legacy.book.hosts[0].version,'r1590');
  assert.throws(()=>run([...baseArgs,'dusky-osprey','--protocol=2','--prepare']));
  assert.equal(git(repo,'ls-remote','origin','refs/heads/gh-pages').split(/\s/)[0],base);
});
test('v3 proof enumerates every frozen v1/v2 artifact and rejects omissions',t=>{
  const temp=fixture(t),catalog={games:[{id:'league',versions:[{v:'v2',path:'games/league/v2/',proto:1},{v:'v1',path:'games/league/v1/',proto:1}]}]};
  const expected=[];
  for(const version of ['v1','v2'])for(const file of ['index.html','pkg/league_bg.wasm','pkg/league.js','assets/champion.svg']){
    const name=`games/league/${version}/${file}`;write(temp,name,`${version}:${file}`);expected.push(name);
  }
  write(temp,'games.json',JSON.stringify(catalog));git(temp,'add','.');git(temp,'commit','-qm','frozen versions');
  const base=git(temp,'rev-parse','HEAD');write(temp,'games/league/v3/index.html','new');git(temp,'add','.');git(temp,'commit','-qm','v3');
  const scope=proveScope(temp,base,'HEAD','games/league/v3');
  assert.deepEqual(frozenArtifacts(temp,base,'v3',scope,catalog),expected.sort());
  assert.throws(()=>frozenArtifacts(temp,base,'v3',{frozenVersions:scope.frozenVersions.filter(row=>!row.path.endsWith('/v2'))},catalog),/v2 artifact proof is missing/);
  const changed=structuredClone(catalog);changed.games[0].versions[0].path='games/league/v2-redirect/';
  assert.throws(()=>frozenArtifacts(temp,base,'v3',scope,changed),/catalog path/);
  write(temp,'games/league/v2/assets/champion.svg','changed');git(temp,'add','.');git(temp,'commit','-qm','bad frozen mutation');
  assert.throws(()=>proveScope(temp,base,'HEAD','games/league/v3'),/Unexpected Pages change/);
});
test('public proof requires agreement between catalog, welcome and named host',()=>{
  const report={host:'dusky-osprey-league-v3',sourceCommit:stamp,liveWelcome:{proto:2,commit:stamp}},version={proto:2};
  const host={name:report.host,league_ws:'wss://new.example',league_proto:2,league_commit:stamp},book={hosts:[host]};
  assert.equal(verifyHost(book,report,version),host);
  assert.throws(()=>verifyHost(book,report,{proto:1}),/server protocol/);
  assert.throws(()=>verifyHost({hosts:[{...host,league_proto:1}]},report,version),/host protocol/);
  assert.throws(()=>verifyHost({hosts:[host,host]},report,version),/duplicated/);
});
