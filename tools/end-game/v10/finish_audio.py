"""Finish generated speech and author a seeded periodic wind/drone/chain ambience."""
from pathlib import Path
import ctypes, hashlib, json, math, os, time, wave
import numpy as np
from scipy import signal
if os.name=='nt':ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(),0x40)
ROOT=Path(os.environ.get('END_GAME_V10_WORK',str(Path(__file__).resolve().parent)))
OUT=Path(os.environ.get('END_GAME_V10_OUT',str(Path(os.environ.get('EMBER_ROOT',str(Path(__file__).resolve().parents[3])))/'assets/end-game/v10')))
def save(path,y,rate,peak):
 y=np.asarray(y,dtype=np.float64);assert np.isfinite(y).all();y*=peak/max(np.abs(y).max(),1e-9);pcm=np.round(y*32767).astype('<i2')
 with wave.open(str(path),'wb') as w:w.setparams((1,2,rate,0,'NONE','not compressed'));w.writeframes(pcm.tobytes())
 assert np.abs(pcm.astype(np.int32)).max()<32767
 return dict(file=path.name,bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest(),frames=len(pcm),duration=round(len(pcm)/rate,5),sample_rate=rate,channels=1,peak_dbfs=round(20*np.log10(max(np.abs(pcm.astype(float)).max()/32768,1e-9)),3),rms_dbfs=round(20*np.log10(max(np.sqrt(np.mean((pcm/32768.)**2)),1e-9)),3),clipped_samples=int((np.abs(pcm.astype(np.int32))>=32767).sum()),boundary_jump=float(abs(int(pcm[-1])-int(pcm[0]))/32768))
def main():
 start=time.perf_counter();OUT.mkdir(parents=True,exist_ok=True);source=json.loads((ROOT/'raw/generation.json').read_text());rows=[];rng=np.random.default_rng(100908)
 for item in source['files']:
  path=ROOT/'raw'/item['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==item['sha256']
  with wave.open(str(path),'rb') as w:assert w.getparams()[:3]==(1,2,24000);y=np.frombuffer(w.readframes(w.getnframes()),'<i2').astype(float)/32768
  if item['speaker']=='Castellan':
   y=signal.resample_poly(y,100,95);y=signal.sosfilt(signal.butter(2,65,'highpass',fs=24000,output='sos'),y);y=np.tanh(y*2.3)/2.3
  else:y=signal.sosfilt(signal.butter(2,180 if item['speaker']=='Inscription' else 70,'highpass',fs=24000,output='sos'),y)
  if item['speaker']=='Inscription':
   env=np.sqrt(np.maximum(0,signal.convolve(y*y,np.ones(480)/480,mode='same')));breath=signal.sosfilt(signal.butter(2,[700,6500],'bandpass',fs=24000,output='sos'),rng.normal(0,1,len(y)));y=y*.8+breath*env*.24
  fade=min(480,len(y)//2);y[:fade]*=np.linspace(0,1,fade);y[-fade:]*=np.linspace(1,0,fade);y=np.pad(y,(600,1800))
  row=save(OUT/(item['id'].replace('_','-')+'.wav'),y,24000,10**(-5/20) if item['speaker']=='Inscription' else 10**(-3/20));row.update(id=item['id'],text=item['text'],speaker=item['speaker'],source_sha256=item['sha256']);rows.append(row)
 # Exactly periodic FFT noise and integer-cycle partials give a seamless authored bed.
 rate=48000;n=rate*16;t=np.arange(n)/rate;freq=np.fft.rfftfreq(n,1/rate);phase=rng.uniform(0,2*np.pi,len(freq));shape=np.exp(-((freq-170)/370)**2)*(freq>28)/(1+(freq/650)**3);wind=np.fft.irfft(shape*np.exp(1j*phase),n);wind/=max(np.std(wind),1e-9)
 y=.022*wind*(.70+.18*np.sin(2*np.pi*t/8)+.10*np.sin(2*np.pi*t/16))
 for hz,gain in [(41,.030),(63,.016),(82,.008),(147,.004)]:y+=gain*np.sin(2*np.pi*hz*t)*(1+.12*np.sin(2*np.pi*t/16))
 for at in [1.8,6.2,11.3]:
  u=(t-at)%16;hit=np.exp(-u*5)*(u<2)
  for hz,gain in [(713,.012),(1087,.008),(1631,.006),(2401,.003)]:y+=gain*hit*np.sin(2*np.pi*hz*u)
 # Small circular echoes preserve the exact loop period.
 y=y+np.roll(y,int(.17*rate))*.18+np.roll(y,int(.39*rate))*.08
 ambient=save(OUT/'castle-ambience.wav',y,rate,10**(-12/20));ambient.update(id='castle_ambience',method='Authored seeded offline DSP, not model-generated: periodic spectral wind, integer-cycle low drone, damped metallic chain partials and circular echoes.',seed=100908,loop=True)
 assert ambient['boundary_jump']<.01
 manifest=dict(schema_version=1,files=rows,ambience=ambient,total_bytes=sum(r['bytes'] for r in rows)+ambient['bytes'],provenance=source,processing=dict(boss_pitch_rate=.95,boss_saturation_drive=2.3,inscription_breath_mix=.24,voice_peak_dbfs=-3,clue_peak_dbfs=-5),verification='Decoded PCM waveform, finite samples, durations, hashes and clipping checked. No listening, browser playback or physical-device check.',wall_seconds=round(time.perf_counter()-start,3))
 (OUT/'voice-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n')
 lines={r['id']:{k:r[k] for k in ['file','text','duration','speaker']} for r in rows};(OUT/'voice-lines.js').write_text('// Generated V10 audio contract; durations measured from PCM WAV.\nexport const CASTLE_LINES = '+json.dumps(lines,indent=2)+';\n',encoding='utf-8',newline='\n')
 print(json.dumps({k:manifest[k] for k in ['files','ambience','total_bytes','wall_seconds']},indent=2))
if __name__=='__main__':main()
