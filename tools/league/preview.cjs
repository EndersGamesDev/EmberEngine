// Disposable terminal preview. Owns only its temporary HTTP/League processes.
'use strict';
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),os=require('node:os');
const {spawn}=require('node:child_process');
const root=path.resolve(__dirname,'../..');

async function startPreview({port=8094,gamePort=7794,gameVersion='v2',protocol=gameVersion==='v3'?2:1,uiDirectory=path.join(root,`web/games/league/${gameVersion}`),online=true}={}){
  if(!/^v[1-9][0-9]*$/.test(gameVersion)||!Number.isInteger(protocol)||protocol<1)throw new Error('Invalid preview version/protocol');
  os.setPriority(0,os.constants.priority.PRIORITY_LOW);
  const web=path.join(root,'web'),origin=`http://127.0.0.1:${port}`,ws=`ws://127.0.0.1:${gamePort}`;
  let game;
  if(online){
    game=spawn(path.join(root,'target/release/league-server.exe'),[`127.0.0.1:${gamePort}`,'--name','league-preview'],{windowsHide:true,stdio:'ignore'});
    await new Promise((resolve,reject)=>{game.once('error',reject);setTimeout(()=>game.exitCode===null?resolve():reject(new Error('Preview game server did not start')),400);});
  }
  const server=http.createServer((req,res)=>{
    let url;try{url=new URL(req.url,origin);}catch{res.writeHead(400).end();return;}
    if(url.pathname==='/server.json'){res.setHeader('Content-Type','application/json');res.end(JSON.stringify({hosts:online?[{name:'preview',league_ws:ws,league_proto:protocol}]:[],mirrors:[]}));return;}
    if(url.pathname==='/favicon.ico'){res.writeHead(204).end();return;}
    let rel;try{rel=decodeURIComponent(url.pathname);}catch{res.writeHead(400).end();return;}
    if(rel.endsWith('/'))rel+='index.html';
    let base=web;
    const ui=rel.match(new RegExp(`^/games/league/${gameVersion}/(index\\.html|ui\\.css|ui\\.js)$`));
    if(ui){base=path.resolve(uiDirectory);rel='/'+ui[1];}
    else if(rel.startsWith(`/games/league/${gameVersion}/pkg/`))rel='/pkg/'+path.basename(rel);
    const file=path.resolve(base,'.'+rel);
    if(!file.startsWith(base+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404).end();return;}
    const types={'.html':'text/html; charset=utf-8','.js':'text/javascript','.css':'text/css','.json':'application/json','.wasm':'application/wasm','.webp':'image/webp','.svg':'image/svg+xml','.png':'image/png','.mp4':'video/mp4','.webm':'video/webm','.vtt':'text/vtt; charset=utf-8','.wav':'audio/wav'};
    const size=fs.statSync(file).size;
    res.setHeader('Content-Type',types[path.extname(file)]||'application/octet-stream');res.setHeader('Cache-Control','no-store');
    res.setHeader('Accept-Ranges','bytes');
    let start=0,end=size-1,status=200;
    if(req.method==='GET'&&req.headers.range){
      const range=/^bytes=(\d*)-(\d*)$/.exec(req.headers.range.trim());
      if(range&&(range[1]||range[2])){
        if(range[1]){start=Number(range[1]);end=range[2]?Number(range[2]):end;}
        else {const suffix=Number(range[2]);start=Math.max(0,size-suffix);end=size-1;}
      }
      if(!range||(!range[1]&&!range[2])||range.slice(1).some(value=>value&&!Number.isSafeInteger(Number(value)))||!Number.isSafeInteger(start)||!Number.isSafeInteger(end)||start<0||start>=size||end<start){
        res.writeHead(416,{'Content-Range':`bytes */${size}`,'Content-Length':'0'}).end();return;
      }
      end=Math.min(end,size-1);status=206;res.setHeader('Content-Range',`bytes ${start}-${end}/${size}`);
    }
    res.writeHead(status,{'Content-Length':String(Math.max(0,end-start+1))});
    if(req.method==='HEAD'||size===0){res.end();return;}
    const stream=fs.createReadStream(file,{start,end});
    res.on('close',()=>stream.destroy());
    stream.on('error',error=>res.destroy(error));stream.pipe(res);
  });
  try{await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(port,'127.0.0.1',resolve);});}
  catch(error){game?.kill();throw error;}
  return {origin,ws,async close(){game?.kill();await new Promise(resolve=>server.close(resolve));}};
}
module.exports={startPreview};
if(require.main===module){
  startPreview().then(preview=>{
    console.log(`UltimateLegue v2 preview: ${preview.origin}/games/league/v2/`);
    for(const signal of ['SIGINT','SIGTERM'])process.once(signal,()=>preview.close().then(()=>process.exit(0)));
  }).catch(error=>{console.error(error);process.exitCode=1;});
}
