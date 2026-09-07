// Isolated Git fixtures only: no public HTTP, game sockets, runtime or publication.
'use strict';
const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {execFileSync}=require('node:child_process');
const {proveServerCompatibility,SERVER_INPUTS}=require('./publish.cjs');
const {verifyHost}=require('./prove-live.cjs');
os.setPriority(0,os.constants.priority.PRIORITY_LOW);
const git=(root,...args)=>execFileSync('git',['-c','core.autocrlf=false','-C',root,...args],{windowsHide:true,stdio:'pipe'}).toString().trim();
const write=(root,file,bytes)=>{const dest=path.join(root,file);fs.mkdirSync(path.dirname(dest),{recursive:true});fs.writeFileSync(dest,bytes);};
function fixture(t){
  const root=fs.mkdtempSync(path.join(os.tmpdir(),'league-server-proof-'));
  t.after(()=>{
    const absolute=path.resolve(root);
    assert.equal(path.dirname(absolute),path.resolve(os.tmpdir()));
    assert(path.basename(absolute).startsWith('league-server-proof-'));
    fs.rmSync(absolute,{recursive:true,force:true});
  });
  git(root,'init','-q');git(root,'config','user.name','League fixture');git(root,'config','user.email','fixture@example.invalid');
  for(const file of SERVER_INPUTS)write(root,file.startsWith('crates/')?`${file}/src/lib.rs`:file,`unchanged ${file}\n`);
  git(root,'add','.');git(root,'commit','-qm','proven server');
  const server=git(root,'rev-parse','HEAD');
  write(root,'crates/league/src/visual.rs','client presentation only\n');
  git(root,'add','.');git(root,'commit','-qm','client visual release');
  return {root,server,client:git(root,'rev-parse','HEAD')};
}
test('default source/server equality remains exact; ancestor reuse records all five immutable inputs',t=>{
  const {root,server,client}=fixture(t);
  const current=proveServerCompatibility(root,client);
  assert.equal(current.serverCommit,client);assert.equal(current.reusedServer,false);
  const reused=proveServerCompatibility(root,client,server);
  assert.equal(reused.reusedServer,true);assert.equal(reused.ancestor,true);
  assert.deepEqual(reused.unchanged.map(row=>row.path),SERVER_INPUTS);
  assert(reused.unchanged.every(row=>/^[0-9a-f]{40}$/.test(row.oid)));
  for(const invalid of ['',server.slice(0,8),'HEAD','0'.repeat(40)])assert.throws(()=>proveServerCompatibility(root,client,invalid));
});
for(const input of SERVER_INPUTS)test(`reuse refuses changed ${input}`,t=>{
  const {root,server}=fixture(t);
  write(root,input.startsWith('crates/')?`${input}/src/lib.rs`:input,'changed server input\n');
  git(root,'add','.');git(root,'commit','-qm','incompatible source');
  assert.throws(()=>proveServerCompatibility(root,git(root,'rev-parse','HEAD'),server),new RegExp(`input changed: ${input.replaceAll('.','\\.')}`));
});
test('same server input bytes on a nonancestor branch are insufficient',t=>{
  const {root,server,client}=fixture(t);
  git(root,'checkout','-q','--detach',server);
  write(root,'unrelated.txt','sibling');git(root,'add','.');git(root,'commit','-qm','unrelated server lineage');
  const sibling=git(root,'rev-parse','HEAD');
  assert.throws(()=>proveServerCompatibility(root,client,sibling),/ancestor/);
});
test('public host accepts reused server only after a fresh proof and rejects wrong welcome/book stamps',t=>{
  const {root,server,client}=fixture(t),stamp=server.slice(0,8);
  const report={sourceCommit:client,serverCommit:server,serverCompatibility:proveServerCompatibility(root,client,server),host:'proven-v3',liveWelcome:{proto:2,commit:stamp,version:'r1'}};
  const host={name:report.host,league_ws:'wss://fixture.invalid',league_proto:2,league_commit:stamp};
  const book={hosts:[host]},version={proto:2};
  assert.equal(verifyHost(book,report,version,root),host);
  assert.throws(()=>verifyHost(book,report,version),/fresh Git compatibility/);
  assert.throws(()=>verifyHost(book,{...report,serverCommit:undefined,serverCompatibility:undefined},version),/live build stamp/,'Omitting explicit reuse must retain current-source matching');
  assert.throws(()=>verifyHost(book,{...report,liveWelcome:{...report.liveWelcome,commit:client.slice(0,8)}},version,root),/live build stamp/);
  assert.throws(()=>verifyHost({hosts:[{...host,league_commit:client.slice(0,8)}]},report,version,root),/host book is stale/);
  assert.throws(()=>verifyHost(book,{...report,serverCompatibility:{...report.serverCompatibility,unchanged:[]}},version,root),/compatibility proof differs/);
});
