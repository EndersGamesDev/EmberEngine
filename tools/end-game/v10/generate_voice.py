"""Five CPU-only, offline, cached Kokoro V10 speech lines."""
from pathlib import Path
import argparse, hashlib, importlib.metadata, json, os, time, wave
os.environ.setdefault('HF_HOME','/home/ender/asset-forge/models/kokoro/hf-home')
os.environ.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',CUDA_VISIBLE_DEVICES='',OMP_NUM_THREADS='2',MKL_NUM_THREADS='2')
if hasattr(os,'nice'):os.nice(max(0,19-os.nice(0)))
import numpy as np
import torch
from kokoro import KPipeline
LINES=[
 ('boss_intro','A thief in my halls. Kneel before the One-Eyed Castellan!','Castellan',.93),
 ('boss_phase2','You have cracked my armor. Now face my wrath!','Castellan',.93),
 ('boss_defeat','The gate... my seal... take it, and be gone.','Castellan',.88),
 ('escape_clue',"Wake the sun in the dry fountain. Then the wolf on the wall. Ring the bell above. The Castellan's crown breaks the final chain.",'Inscription',.95),
 ('escape_ending','At last. The castle is behind you. Keep moving.','Prisoner',.97),
]
def main():
 p=argparse.ArgumentParser();p.add_argument('--out',type=Path,required=True);a=p.parse_args();a.out.mkdir(parents=True,exist_ok=True)
 start=time.perf_counter();torch.set_num_threads(2);torch.set_num_interop_threads(1);torch.manual_seed(100908);np.random.seed(100908)
 revision=(Path(os.environ['HF_HOME'])/'hub/models--hexgrad--Kokoro-82M/refs/main').read_text().strip()
 assert revision=='f3ff3571791e39611d31c381e3a41a3af07b4987'
 pipe=KPipeline(lang_code='b',repo_id='hexgrad/Kokoro-82M',device='cpu');voices={}
 voices['Castellan']=pipe.load_voice('bm_george')*.85+pipe.load_voice('bm_lewis')*.15
 voices['Inscription']=pipe.load_voice('bm_lewis')*.75+pipe.load_voice('bm_george')*.25
 voices['Prisoner']=pipe.load_voice('bm_lewis')
 rows=[]
 for key,text,speaker,speed in LINES:
  began=time.perf_counter();results=list(pipe(text,voice=voices[speaker],speed=speed));samples=np.concatenate([np.asarray(r.audio,dtype=np.float32) for r in results]);assert np.isfinite(samples).all() and 0<len(samples)<24000*30
  path=a.out/(key.replace('_','-')+'-raw.wav')
  with wave.open(str(path),'wb') as w:w.setparams((1,2,24000,0,'NONE','not compressed'));w.writeframes(np.round(np.clip(samples,-1,1)*32767).astype('<i2').tobytes())
  row=dict(id=key,text=text,speaker=speaker,speed=speed,file=path.name,phonemes=[r.phonemes for r in results],graphemes=[r.graphemes for r in results],seconds=len(samples)/24000,peak=float(np.abs(samples).max()),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),generation_seconds=round(time.perf_counter()-began,3));rows.append(row);print(json.dumps(row),flush=True)
 record=dict(schema_version=1,model='hexgrad/Kokoro-82M',model_revision=revision,device='CPU',threads=2,priority='nice 19',offline=True,seed=100908,voices={'Castellan':{'bm_george':.85,'bm_lewis':.15},'Inscription':{'bm_lewis':.75,'bm_george':.25},'Prisoner':{'bm_lewis':1}},versions={k:importlib.metadata.version(k) for k in ['kokoro','torch','misaki','numpy']},files=rows,generation_seconds=round(time.perf_counter()-start,3))
 (a.out/'generation.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n');print('V10_GENERATION_WALL_SECONDS',record['generation_seconds'],flush=True)
if __name__=='__main__':main()
