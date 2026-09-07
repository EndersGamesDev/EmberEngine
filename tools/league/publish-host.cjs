// Use the canonical address-book writer, then prove that only League changed.
'use strict';
const fs=require('node:fs'),path=require('node:path'),os=require('node:os'),assert=require('node:assert/strict');
const {execFileSync}=require('node:child_process');
function parseArgs(args){
  const [url,version,commit,...tail]=args;
  const name=tail.length&&!tail[0].startsWith('--')?tail.shift():'dusky-osprey';
  assert(tail.every(arg=>arg==='--prepare'||/^--protocol=[1-9][0-9]*$/.test(arg)),'Unknown argument');
  assert(tail.filter(arg=>arg==='--prepare').length<=1&&tail.filter(arg=>arg.startsWith('--protocol=')).length<=1,'Duplicate option');
  const protocol=Number(tail.find(arg=>arg.startsWith('--protocol='))?.slice(11)||1);
  assert(Number.isSafeInteger(protocol)&&protocol<=65535,'Invalid League protocol');
  assert(/^wss:\/\/[a-z0-9.-]+(?::\d+)?\/?$/.test(url),'Invalid League URL');
  assert(/^r\d+$/.test(version)&&/^[0-9a-f]{7,40}$/.test(commit),'Invalid build stamp');
  assert(/^[a-z0-9-]{3,32}$/.test(name),'Invalid host name');
  return {url,version,commit,name,protocol,prepare:tail.includes('--prepare')};
}
function validateTarget(book,{name,protocol}){
  assert(Array.isArray(book.hosts),'Address book has no host list');
  const matches=book.hosts.filter(host=>host.name===name);
  assert(matches.length<=1,'Duplicate target host');
  const old=matches[0];
  assert(!old?.league_ws||old.league_proto===protocol,'A different League protocol already uses this host name; publish a separate host');
}
const git=(cwd,...a)=>execFileSync('git',['-c','core.autocrlf=false','-C',cwd,...a],{windowsHide:true,maxBuffer:64*1024*1024});
const text=(cwd,...a)=>git(cwd,...a).toString().trim();
const without=(obj,omit)=>Object.fromEntries(Object.entries(obj).filter(([k])=>!omit(k)));
function main(){
const root=process.cwd(),started=Date.now(),options=parseArgs(process.argv.slice(2));
const {url,version,commit,name,protocol,prepare}=options;
os.setPriority(0,os.constants.priority.PRIORITY_LOW);
git(root,'fetch','origin','gh-pages');
const base=text(root,'rev-parse','origin/gh-pages'),before=JSON.parse(git(root,'show',`${base}:server.json`));
validateTarget(before,options);
const wt=path.join(fs.mkdtempSync(path.join(os.tmpdir(),'ember-league-book-')),'pages');
git(root,'worktree','add','--detach','--no-checkout',wt,base);
git(wt,'sparse-checkout','set','--no-cone','/server.json');git(wt,'read-tree','-mu','HEAD');
assert.equal(text(wt,'write-tree'),text(root,'rev-parse',`${base}^{tree}`));
const bash=process.platform==='win32'?'C:/Program Files/Git/bin/bash.exe':'bash';
execFileSync(bash,[path.join(root,'deploy/publish-host.sh').replaceAll('\\','/'),'--name',name,'--game','league','--url',url,'--proto',String(protocol),'--version',version,'--commit',commit,'--book',path.join(wt,'server.json').replaceAll('\\','/')],{windowsHide:true,stdio:'pipe'});
const after=JSON.parse(fs.readFileSync(path.join(wt,'server.json')));
// The shared writer recomputes all legacy addresses. A League release must
// preserve peers' selected legacy endpoints even when a shared timestamp changes.
for(const key of new Set([...Object.keys(before),...Object.keys(after)])){
  if((key==='ws'||key.endsWith('_ws'))&&!key.startsWith('league_')){
    if(Object.hasOwn(before,key))after[key]=before[key];else delete after[key];
  }
}
fs.writeFileSync(path.join(wt,'server.json'),JSON.stringify(after,null,2)+'\n');
const rootOmit=k=>k==='v'||k==='hosts'||k==='league_ws';
assert.deepEqual(without(after,rootOmit),without(before,rootOmit),'Non-League top-level addresses changed');
assert.deepEqual(after.hosts.filter(h=>h.name!==name),before.hosts.filter(h=>h.name!==name),'Other hosts changed');
const old=before.hosts.find(h=>h.name===name),now=after.hosts.find(h=>h.name===name);
const hostOmit=k=>k==='updated'||k==='by'||k.startsWith('league_');
if(old)assert.deepEqual(without(now,hostOmit),without(old,hostOmit),'Other games on this host changed');
assert.equal(now.league_ws,url);assert.equal(now.league_commit,commit);
assert.equal(now.league_proto,protocol);assert.equal(now.league_version,version);
git(wt,'add','server.json');
const changes=text(wt,'diff','--cached','--name-only');
if(changes&&!prepare){
  assert.equal(changes,'server.json');
  assert.equal(text(root,'ls-remote','origin','refs/heads/gh-pages').split(/\s+/)[0],base,'Concurrent Pages write; rerun against latest');
  git(wt,'commit','-m',`Publish League host ${name}; preserve other games`);git(wt,'push','origin','HEAD:gh-pages');
}
console.log(JSON.stringify({commit:text(wt,'rev-parse','HEAD'),preparedOnly:prepare,url,protocol,elapsedSeconds:(Date.now()-started)/1000,worktree:wt}));
}
module.exports={parseArgs,validateTarget};
if(require.main===module)try{main();}catch(error){console.error(error);process.exitCode=1;}
