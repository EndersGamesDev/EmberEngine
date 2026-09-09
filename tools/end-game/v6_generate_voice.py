"""Generate four offline Kokoro source takes; intended for an existing CPU worker."""
from pathlib import Path
import argparse
import hashlib
import importlib.metadata
import json
import os
import time
import wave

os.environ.setdefault('HF_HOME', '/home/ender/asset-forge/models/kokoro/hf-home')
os.environ['HF_HUB_OFFLINE'] = '1'
os.environ['TRANSFORMERS_OFFLINE'] = '1'
os.environ['CUDA_VISIBLE_DEVICES'] = ''
os.environ['OMP_NUM_THREADS'] = '2'
os.environ['MKL_NUM_THREADS'] = '2'
if hasattr(os, 'nice'):
    os.nice(max(0, 19 - os.nice(0)))

import numpy as np
import torch
from kokoro import KPipeline

LINES = [
    ('movement', 'Shut up and go back to your place', 'Shut up, and go back to your place!', 0.98),
    ('unlocking', 'Where did you get that key from?', 'Where did you get that key from?', 0.96),
    ('sword', 'Why you have a sword here?', 'Why you have a sword here?', 0.94),
    ('death', 'Arrrrrghhhh, I will get revenge!', 'Arrrrrghhhh! I will get revenge!', 0.90),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    torch.set_num_threads(2)
    torch.set_num_interop_threads(1)
    torch.manual_seed(60908)
    np.random.seed(60908)
    revision = Path(os.environ['HF_HOME'])/'hub/models--hexgrad--Kokoro-82M/refs/main'
    assert revision.read_text().strip() == 'f3ff3571791e39611d31c381e3a41a3af07b4987', 'Different cached model revision; inspect provenance before regenerating.'
    pipe = KPipeline(lang_code='b', repo_id='hexgrad/Kokoro-82M', device='cpu')
    voice = pipe.load_voice('bm_george') * 0.70 + pipe.load_voice('bm_lewis') * 0.30
    rows = []
    for key, transcript, text, speed in LINES:
        began = time.perf_counter()
        results = list(pipe(text, voice=voice, speed=speed))
        samples = np.concatenate([np.asarray(r.audio, dtype=np.float32) for r in results])
        assert np.isfinite(samples).all() and 0 < len(samples) < 24000 * 10
        path = args.out / f'warden-{key}-raw.wav'
        with wave.open(str(path), 'wb') as wav:
            wav.setparams((1, 2, 24000, 0, 'NONE', 'not compressed'))
            wav.writeframes(np.round(np.clip(samples, -1, 1) * 32767).astype('<i2').tobytes())
        row = dict(id=key, text=transcript, synthesis_text=text, speed=speed, file=path.name,
                   phonemes=[r.phonemes for r in results], graphemes=[r.graphemes for r in results],
                   seconds=len(samples)/24000, peak=float(np.abs(samples).max()),
                   sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                   generation_seconds=round(time.perf_counter()-began, 4))
        rows.append(row)
        print(json.dumps(row), flush=True)
    record = dict(schema_version=1, model='hexgrad/Kokoro-82M',
                  model_revision='f3ff3571791e39611d31c381e3a41a3af07b4987',
                  voices={'bm_george': .70, 'bm_lewis': .30}, seed=60908,
                  device='CPU', threads=2, priority='nice 19', offline=True,
                  versions={k: importlib.metadata.version(k) for k in ['kokoro','torch','misaki','numpy']},
                  files=rows, generation_seconds=round(time.perf_counter()-started, 4))
    (args.out/'generation.json').write_text(json.dumps(record, indent=2)+'\n', encoding='utf-8', newline='\n')
    print('V6_GENERATION_WALL_SECONDS', record['generation_seconds'], flush=True)


if __name__ == '__main__':
    main()
