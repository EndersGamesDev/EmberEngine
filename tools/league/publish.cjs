// Publish only League and its catalog entry. All other Pages files stay byte-identical.
// Requires clean committed source, tested WASM hash and an already proven public host.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const {execFileSync} = require('node:child_process');
const root = process.cwd(), started = Date.now();
const args = process.argv.slice(2);
const arg = name => args.find(x => x.startsWith(`--${name}=`))?.slice(name.length + 3);
const git = (cwd, ...a) => execFileSync('git', ['-c','core.autocrlf=false','-C',cwd,...a], {windowsHide:true,maxBuffer:64*1024*1024});
const txt = (cwd, ...a) => git(cwd,...a).toString().trim();
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const entry='games/league/v1';
const allowed=['games.json',`${entry}/index.html`,`${entry}/version.json`,`${entry}/pkg/league.js`,`${entry}/pkg/league_bg.wasm`];
async function welcome(url) {
  return new Promise((resolve,reject) => {
    const ws = new WebSocket(url), timer=setTimeout(()=>{ws.close();reject(new Error('League welcome timed out'));},15000);
    ws.addEventListener('open',()=>ws.send(JSON.stringify({t:'hello',proto:1,handle:'release-check'})));
    ws.addEventListener('error',()=>{clearTimeout(timer);reject(new Error('League public socket failed'));});
    ws.addEventListener('message',e=>{const m=JSON.parse(e.data); if(m.t==='welcome'){clearTimeout(timer);ws.close();resolve(m);}});
  });
}
async function main(){
  os.setPriority(0,os.constants.priority.PRIORITY_LOW);
  assert(args.every(a=>a==='--push'||/^--(?:build-commit|wasm-sha256)=/.test(a)),'Unknown argument');
  assert.equal(txt(root,'status','--porcelain'),'','Source must be clean');
  const commit=txt(root,'rev-parse','HEAD');
  assert.equal(arg('build-commit'),commit,'Build revision differs from HEAD');
  const wasm=fs.readFileSync(path.join(root,'web/pkg/league_bg.wasm'));
  assert.equal(hash(wasm),arg('wasm-sha256'),'Tested WASM changed');
  assert(WebAssembly.validate(wasm),'Invalid WASM');
  git(root,'fetch','origin','main','gh-pages');
  assert.equal(txt(root,'rev-parse','origin/main'),commit,'Publish current main only');
  const base=txt(root,'rev-parse','origin/gh-pages'), read=f=>git(root,'show',`${base}:${f}`);
  const book=JSON.parse(read('server.json'));
  const validCommit=v=>typeof v==='string'&&/^[0-9a-f]{7,40}$/.test(v);
  const host=book.hosts.find(h=>h.league_proto===1 && h.league_ws && validCommit(h.league_commit) && commit.startsWith(h.league_commit));
  assert(host,'Published address book must contain this League build');
  const live=await welcome(host.league_ws);
  assert.equal(live.proto,1); assert(validCommit(live.commit)&&commit.startsWith(live.commit),'Wrong public League build');
  const catalog=JSON.parse(read('games.json')), priorPeers=catalog.games.filter(g=>g.id!=='league');
  const source=JSON.parse(fs.readFileSync(path.join(root,'web/games.json'))).games.find(g=>g.id==='league');
  assert(source && source.versions[0].path===`${entry}/`);
  const at=catalog.games.findIndex(g=>g.id==='league');
  if(at<0) catalog.games.splice(1,0,source); else catalog.games[at]=source;
  assert.deepEqual(catalog.games.filter(g=>g.id!=='league'),priorPeers);
  const temporary=fs.mkdtempSync(path.join(os.tmpdir(),'ember-league-pages-')), worktree=path.join(temporary,'pages');
  git(root,'worktree','add','--detach','--no-checkout',worktree,base);
  git(worktree,'sparse-checkout','set','--no-cone','/games.json',`/${entry}/`);
  git(worktree,'read-tree','-mu','HEAD');
  assert.equal(txt(worktree,'write-tree'),txt(root,'rev-parse',`${base}^{tree}`));
  const version={version:live.version,commit,built:new Date().toISOString(),wasmSha256:hash(wasm)};
  const writes={
    'games.json':JSON.stringify(catalog,null,2)+'\n',
    [`${entry}/index.html`]:fs.readFileSync(path.join(root,'web',entry,'index.html')),
    [`${entry}/version.json`]:JSON.stringify(version,null,2)+'\n',
    [`${entry}/pkg/league.js`]:fs.readFileSync(path.join(root,'web/pkg/league.js')),
    [`${entry}/pkg/league_bg.wasm`]:wasm,
  };
  for(const [name,bytes] of Object.entries(writes)){const file=path.join(worktree,name);fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,bytes);}
  git(worktree,'add','--',...allowed);
  const changes=txt(worktree,'diff','--cached','--name-status').split('\n').filter(Boolean);
  for(const change of changes){const [status,file]=change.split('\t');assert(['A','M'].includes(status)&&allowed.includes(file),`Unexpected change: ${change}`);}
  assert(changes.length,'No League changes');
  assert.equal(txt(root,'status','--porcelain'),'','Source changed while preparing');
  assert.equal(hash(fs.readFileSync(path.join(root,'web/pkg/league_bg.wasm'))),arg('wasm-sha256'));
  assert.equal(txt(root,'ls-remote','origin','refs/heads/gh-pages').split(/\s+/)[0],base,'Concurrent Pages update: prepare again');
  const report={sourceCommit:commit,base,worktree,version,host:host.name,liveWelcome:live,changes,pushed:false};
  if(args.includes('--push')){git(worktree,'commit','-m',`Publish UltimateLegue v1 ${commit.slice(0,8)}; preserve other games`);git(worktree,'push','origin','HEAD:gh-pages');report.pagesCommit=txt(worktree,'rev-parse','HEAD');report.pushed=true;}
  report.elapsedSeconds=(Date.now()-started)/1000;
  fs.mkdirSync(path.join(root,'target/league-publish'),{recursive:true});
  fs.writeFileSync(path.join(root,'target/league-publish/results.json'),JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify(report,null,2));
}
main().catch(e=>{console.error(e);process.exitCode=1;});
