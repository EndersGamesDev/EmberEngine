"""Decode and verify shipped V6 WAVs without audio playback or network access."""
from pathlib import Path
import argparse
import hashlib
import json
import math
import os
import re
import time
import wave
import numpy as np

EXPECTED = {
    'movement': 'Shut up and go back to your place',
    'unlocking': 'Where did you get that key from?',
    'sword': 'Why you have a sword here?',
    'death': 'Arrrrrghhhh, I will get revenge!',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--assets',type=Path,default=Path(__file__).resolve().parents[2]/'assets/end-game/v6')
    args=parser.parse_args()
    if os.name=='nt':
        import ctypes
        ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(),0x40)
    started=time.perf_counter()
    manifest=json.loads((args.assets/'manifest.json').read_text())
    assert {r['id'] for r in manifest['files']}==set(EXPECTED)
    assert len(manifest['files'])==4 and manifest['total_bytes']<1000000
    for source in manifest['provenance']['files']:
        words=lambda s: re.findall(r'[a-z]+',s.lower())
        assert words(' '.join(source['graphemes']))==words(EXPECTED[source['id']])
        assert source['phonemes'] and all(source['phonemes'])
    rows=[]
    for row in manifest['files']:
        path=args.assets/row['file'];raw=path.read_bytes()
        assert row['text']==EXPECTED[row['id']]
        assert row['file']==f"warden-{row['id']}.wav"
        assert hashlib.sha256(raw).hexdigest()==row['sha256']
        assert len(raw)==row['bytes']
        with wave.open(str(path),'rb') as wav:
            assert wav.getparams()[:3]==(1,2,24000) and wav.getcomptype()=='NONE'
            assert wav.getnframes()==row['frames']
            x=np.frombuffer(wav.readframes(wav.getnframes()),dtype='<i2').astype(np.float64)/32768
        seconds=len(x)/24000;peak=float(np.abs(x).max());rms=float(np.sqrt(np.mean(x*x)))
        assert 1.5<seconds<7 and abs(seconds-row['seconds'])<.00001
        assert np.isfinite(x).all() and .70<peak<.80
        assert not np.any(np.abs(x)>=.999) and .045<rms<.30
        assert abs(float(x.mean()))<.003
        assert np.max(np.abs(x[:480]))==0 and np.max(np.abs(x[-1200:]))==0
        blocks=np.array([np.sqrt(np.mean(a*a)) for a in np.array_split(x,max(1,len(x)//480))])
        assert np.mean(blocks>.012)>.40, 'excess silence'
        rows.append(dict(file=path.name,seconds=round(seconds,5),peak_dbfs=round(20*math.log10(peak),3),
                         rms_dbfs=round(20*math.log10(rms),3),dc_offset=round(float(x.mean()),7),
                         clipped_samples=0,active_fraction=round(float(np.mean(blocks>.012)),3)))
    assert sum(r['bytes'] for r in manifest['files'])==manifest['total_bytes']
    print(json.dumps(dict(status='passed',files=rows,total_bytes=manifest['total_bytes'],
          verification_seconds=round(time.perf_counter()-started,4),
          scope='PCM format, duration, payload hashes, finite waveform, silence boundaries, DC, peak/RMS and clipping; no speaker or browser playback'),indent=2))


if __name__=='__main__':
    main()
