// Publish one League version and its catalog entry; every other Pages path is frozen.
// Requires clean committed source, tested WASM bytes and an already proven public host.
'use strict';
const fs=require('node:fs'),path=require('node:path'),os=require('node:os');
const crypto=require('node:crypto'),assert=require('node:assert/strict');
const {execFileSync}=require('node:child_process');
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const git=(cwd,...args)=>execFileSync('git',['-c','core.autocrlf=false','-C',cwd,...args],{windowsHide:true,maxBuffer:64*1024*1024});
const text=(cwd,...args)=>git(cwd,...args).toString().trim();

function gameVersion(value='v1'){
  assert(/^v[1-9][0-9]{0,5}$/.test(value),`Invalid game version: ${value}`);
  return value;
}
function safeRelative(name){
  assert(typeof name==='string'&&name.length>0&&!path.isAbsolute(name),`Invalid release path: ${name}`);
  assert(name.split('/').every(part=>/^[A-Za-z0-9._-]+$/.test(part)&&part!=='.'&&part!=='..'&&!part.endsWith('.')&&part.toLowerCase()!=='.git'&&!/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part)),`Unsafe release path: ${name}`);
  return name;
}
function inside(base,file){
  const relative=path.relative(base,file);
  return relative===''||(!path.isAbsolute(relative)&&relative!=='..'&&!relative.startsWith('..'+path.sep));
}
// Check each existing component, including directory junctions, before reading,
// writing or unlinking. Never follow a link from a release tree or sparse checkout.
function safePath(base,name){
  safeRelative(name);
  base=path.resolve(base);
  const baseStat=fs.lstatSync(base);
  assert(baseStat.isDirectory()&&!baseStat.isSymbolicLink(),`Unsafe release root: ${base}`);
  const canonical=fs.realpathSync(base);
  let file=base;
  for(const part of name.split('/')){
    file=path.join(file,part);
    assert(inside(base,file),`Release path escaped root: ${name}`);
    let stat;
    try{stat=fs.lstatSync(file);}catch(error){if(error.code==='ENOENT')continue;throw error;}
    assert(!stat.isSymbolicLink(),`Release symlink/junction refused: ${file}`);
    assert(inside(canonical,fs.realpathSync(file)),`Release path escaped canonical root: ${file}`);
    assert(stat.isDirectory()||stat.isFile(),`Non-regular release path: ${file}`);
  }
  return file;
}
function collectFiles(directory,{excludePkg=false}={}){
  const files=new Map(),folded=new Set();
  function visit(prefix=''){
    const folder=prefix?safePath(directory,prefix):directory;
    for(const name of fs.readdirSync(folder).sort()){
      const relative=prefix?`${prefix}/${name}`:name;
      const file=safePath(directory,relative),stat=fs.lstatSync(file);
      const key=relative.toLowerCase();
      assert(!folded.has(key),`Case-colliding release path: ${relative}`);folded.add(key);
      if(!prefix&&name.toLowerCase()==='pkg'&&excludePkg){
        assert(name==='pkg'&&stat.isDirectory(),'The reserved pkg path must be a directory');
        continue;
      }
      if(stat.isDirectory())visit(relative);else files.set(relative,fs.readFileSync(file));
    }
  }
  assert(fs.lstatSync(directory).isDirectory()&&!fs.lstatSync(directory).isSymbolicLink(),'Invalid release directory');
  visit();return files;
}
function treeFiles(cwd,revision,prefix){
  safeRelative(prefix);
  const rows=git(cwd,'ls-tree','-rz','--full-tree',revision,'--',prefix).toString().split('\0').filter(Boolean),folded=new Set();
  return rows.map(row=>{
    const tab=row.indexOf('\t'),[mode,type,oid]=row.slice(0,tab).split(' '),file=row.slice(tab+1);
    safeRelative(file);
    assert(file.startsWith(prefix+'/'),`Unexpected tree path: ${file}`);
    assert(!folded.has(file.toLowerCase()),`Case-colliding Git path: ${file}`);folded.add(file.toLowerCase());
    assert(type==='blob'&&['100644','100755'].includes(mode),`Release link/submodule refused: ${file} (${mode})`);
    return {file,oid};
  });
}
function frozenVersions(cwd,base,candidate,entry){
  if(!git(cwd,'ls-tree','-z',base,'--','games/league').toString())return [];
  const versions=git(cwd,'ls-tree','-z',`${base}:games/league`).toString().split('\0').filter(Boolean),frozen=[];
  for(const row of versions){
    const tab=row.indexOf('\t'),[,type,oid]=row.slice(0,tab).split(' '),prefix=`games/league/${row.slice(tab+1)}`;
    if(prefix===entry)continue;
    safeRelative(prefix);
    assert.equal(text(cwd,'rev-parse',`${candidate}:${prefix}`),oid,`Frozen League version changed: ${prefix}`);
    frozen.push({path:prefix,type,oid});
  }
  return frozen;
}
function proveScope(cwd,base,candidate,entry){
  safeRelative(entry);assert(/^games\/league\/v[1-9][0-9]{0,5}$/.test(entry),'Invalid release subtree');
  const parts=git(cwd,'diff','--no-renames','--name-status','-z',base,candidate).toString().split('\0').filter(Boolean),changes=[];
  for(let index=0;index<parts.length;index+=2){
    const [status,file]=parts.slice(index,index+2);safeRelative(file);
    assert(['A','M','D'].includes(status)&&(file==='games.json'||file.startsWith(entry+'/')),`Unexpected Pages change: ${status} ${file}`);
    changes.push({status,file});
  }
  const frozen=frozenVersions(cwd,base,candidate,entry);
  if(entry!=='games/league/v1')assert(frozen.some(version=>version.path==='games/league/v1'&&version.type==='tree'),'Existing v1 must remain frozen');
  return {changes,frozenVersions:frozen};
}
function sourceFiles(root,entry){
  const source=safePath(root,`web/${entry}`);
  // Git modes catch symlinks even when Windows checked them out as ordinary files.
  const committed=treeFiles(root,'HEAD',`web/${entry}`).filter(row=>!row.file.startsWith(`web/${entry}/pkg/`)&&row.file!==`web/${entry}/version.json`);
  const files=collectFiles(source,{excludePkg:true});
  assert(files.has('index.html'),'Selected version has no index.html');
  files.delete('version.json'); // Generated from the proven build, never copied stale.
  assert.deepEqual([...files.keys()].sort(),committed.map(row=>row.file.slice(`web/${entry}/`.length)).sort(),'Release contains untracked, ignored or missing source files');
  for(const row of committed)assert.equal(hash(files.get(row.file.slice(`web/${entry}/`.length))),hash(git(root,'show',`HEAD:${row.file}`)),`Release source differs from committed bytes: ${row.file}`);
  return new Map([...files].map(([file,bytes])=>[`${entry}/${file}`,bytes]));
}
async function welcome(url,proto){
  return new Promise((resolve,reject)=>{
    const socket=new WebSocket(url),timer=setTimeout(()=>{socket.close();reject(new Error('League welcome timed out'));},15000);
    socket.addEventListener('open',()=>socket.send(JSON.stringify({t:'hello',proto,handle:'release-check'})));
    socket.addEventListener('error',()=>{clearTimeout(timer);reject(new Error('League public socket failed'));});
    socket.addEventListener('message',event=>{try{const message=JSON.parse(event.data);if(message.t==='welcome'){clearTimeout(timer);socket.close();resolve(message);}}catch(error){clearTimeout(timer);socket.close();reject(error);}});
  });
}
async function main(){
  const root=process.cwd(),started=Date.now(),args=process.argv.slice(2);
  const arg=name=>args.find(value=>value.startsWith(`--${name}=`))?.slice(name.length+3);
  const names=args.map(value=>value.split('=')[0]);assert.equal(new Set(names).size,names.length,'Duplicate argument');
  assert(args.every(value=>value==='--push'||/^--(?:build-commit|wasm-sha256|game-version)=/.test(value)),'Unknown argument');
  const selected=gameVersion(arg('game-version')),entry=`games/league/${selected}`;
  os.setPriority(0,os.constants.priority.PRIORITY_LOW);
  assert.equal(text(root,'status','--porcelain'),'','Source must be clean');
  const commit=text(root,'rev-parse','HEAD');assert.equal(arg('build-commit'),commit,'Build revision differs from HEAD');
  const wasm=fs.readFileSync(safePath(root,'web/pkg/league_bg.wasm'));
  assert.equal(hash(wasm),arg('wasm-sha256'),'Tested WASM changed');assert(WebAssembly.validate(wasm),'Invalid WASM');
  const files=sourceFiles(root,entry);
  files.set(`${entry}/pkg/league.js`,fs.readFileSync(safePath(root,'web/pkg/league.js')));
  files.set(`${entry}/pkg/league_bg.wasm`,wasm);
  git(root,'fetch','origin','main','gh-pages');
  assert.equal(text(root,'rev-parse','origin/main'),commit,'Publish current main only');
  const base=text(root,'rev-parse','origin/gh-pages'),read=file=>git(root,'show',`${base}:${file}`);
  const book=JSON.parse(read('server.json')),catalog=JSON.parse(read('games.json'));
  const source=JSON.parse(fs.readFileSync(safePath(root,'web/games.json'))).games.find(game=>game.id==='league');
  assert(source&&source.versions[0].path===`${entry}/`&&source.versions[0].v===selected&&source.versions[0].live,'Selected version must be the source catalog live version');
  const proto=source.versions[0].proto;assert(Number.isInteger(proto)&&proto>0,'Invalid League protocol');
  const validCommit=value=>typeof value==='string'&&/^[0-9a-f]{7,40}$/.test(value);
  const host=book.hosts.find(value=>value.league_proto===proto&&value.league_ws&&validCommit(value.league_commit)&&commit.startsWith(value.league_commit));
  assert(host,'Published address book must contain this League build');
  const live=await welcome(host.league_ws,proto);
  assert.equal(live.proto,proto);assert(validCommit(live.commit)&&commit.startsWith(live.commit),'Wrong public League build');
  const previousLeague=catalog.games.find(game=>game.id==='league'),priorPeers=catalog.games.filter(game=>game.id!=='league');
  for(const previous of previousLeague?.versions||[])assert(source.versions.some(version=>version.v===previous.v&&version.path===previous.path),'Existing League catalog version was dropped or moved');
  const at=catalog.games.findIndex(game=>game.id==='league');if(at<0)catalog.games.splice(1,0,source);else catalog.games[at]=source;
  assert.deepEqual(catalog.games.filter(game=>game.id!=='league'),priorPeers);
  const existing=treeFiles(root,base,entry); // Refuse published links before sparse checkout.
  const temporary=fs.mkdtempSync(path.join(os.tmpdir(),'ember-league-pages-')),worktree=path.join(temporary,'pages');
  git(root,'worktree','add','--detach','--no-checkout',worktree,base);
  git(worktree,'sparse-checkout','set','--no-cone','/games.json',`/${entry}/`);git(worktree,'read-tree','-mu','HEAD');
  assert.equal(text(worktree,'write-tree'),text(root,'rev-parse',`${base}^{tree}`));
  const version={version:live.version,gameVersion:selected,commit,built:new Date().toISOString(),wasmSha256:hash(wasm)};
  files.set(`${entry}/version.json`,Buffer.from(JSON.stringify(version,null,2)+'\n'));
  files.set('games.json',Buffer.from(JSON.stringify(catalog,null,2)+'\n'));
  for(const previous of existing)if(!files.has(previous.file))fs.unlinkSync(safePath(worktree,previous.file));
  for(const [name,bytes] of files){const file=safePath(worktree,name);fs.mkdirSync(path.dirname(file),{recursive:true});fs.writeFileSync(file,bytes);}
  git(worktree,'add','--all','--','games.json',entry);
  const candidate=text(worktree,'write-tree'),scope=proveScope(worktree,base,candidate,entry);assert(scope.changes.length,'No League changes');
  assert.deepEqual(treeFiles(worktree,candidate,entry).map(row=>row.file).sort(),[...files.keys()].filter(file=>file!=='games.json').sort(),'Staged release differs from the complete local manifest');
  for(const [file,bytes] of files)assert.equal(hash(git(worktree,'show',`${candidate}:${file}`)),hash(bytes),`Staged bytes differ: ${file}`);
  assert.equal(text(root,'status','--porcelain'),'','Source changed while preparing');
  assert.equal(hash(fs.readFileSync(safePath(root,'web/pkg/league_bg.wasm'))),arg('wasm-sha256'));
  assert.equal(hash(fs.readFileSync(safePath(root,'web/pkg/league.js'))),hash(files.get(`${entry}/pkg/league.js`)),'Generated JS changed while preparing');
  assert.equal(text(root,'ls-remote','origin','refs/heads/gh-pages').split(/\s+/)[0],base,'Concurrent Pages update: prepare again');
  const report={sourceCommit:commit,gameVersion:selected,entry,base,worktree,version,host:host.name,liveWelcome:live,...scope,files:[...files].map(([file,bytes])=>({file,sha256:hash(bytes)})),unchangedOutsideRelease:true,pushed:false};
  if(args.includes('--push')){git(worktree,'commit','-m',`Publish UltimateLegue ${selected} ${commit.slice(0,8)}; preserve frozen versions and other games`);git(worktree,'push','origin','HEAD:gh-pages');report.pagesCommit=text(worktree,'rev-parse','HEAD');report.pushed=true;}
  report.elapsedSeconds=(Date.now()-started)/1000;
  fs.mkdirSync(path.join(root,'target/league-publish'),{recursive:true});fs.writeFileSync(path.join(root,'target/league-publish/results.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report,null,2));
}
module.exports={gameVersion,safeRelative,safePath,collectFiles,treeFiles,proveScope,sourceFiles,hash};
if(require.main===module)main().catch(error=>{console.error(error);process.exitCode=1;});
