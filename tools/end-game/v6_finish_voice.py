"""Finish cached warden takes as small browser-compatible PCM WAV assets."""
from pathlib import Path
import argparse
import hashlib
import json
import math
import os
import time
import wave
import numpy as np
from scipy import signal


def read_wav(path):
    with wave.open(str(path), 'rb') as wav:
        assert wav.getparams()[:3] == (1, 2, 24000)
        return np.frombuffer(wav.readframes(wav.getnframes()), dtype='<i2').astype(np.float64)/32768


def finish(samples, is_death):
    # A 2% playback-rate drop supplies a common low register without a vocoder.
    samples = signal.resample_poly(samples, 50, 49)
    rate = 24000
    samples = signal.sosfilt(signal.butter(2, 55, 'highpass', fs=rate, output='sos'), samples)
    samples = signal.sosfilt(signal.butter(3, 7600, 'lowpass', fs=rate, output='sos'), samples)
    chest = signal.sosfilt(signal.butter(2, 220, 'lowpass', fs=rate, output='sos'), samples)
    samples = samples + chest * .10
    samples *= .65/max(float(np.abs(samples).max()), 1e-8)
    # Gentle saturation and 2.5% modulation add grit while retaining consonants.
    samples = np.tanh(samples * 1.20)/math.tanh(1.20)
    t = np.arange(len(samples))/rate
    depth = .025 + (.025*np.maximum(0, 1-t/.75) if is_death else 0)
    samples *= 1-depth + depth*np.sin(2*math.pi*39*t)
    window = 240
    energy = np.sqrt(np.convolve(samples*samples, np.ones(window)/window, mode='same'))
    active = np.flatnonzero(energy > max(float(energy.max())*.015, .0015))
    assert len(active)
    begin = max(0, int(active[0])-int(.160*rate))
    end = min(len(samples), int(active[-1])+int(.055*rate))
    samples = samples[begin:end]
    fade = min(288, len(samples)//2)
    samples[:fade] *= np.linspace(0, 1, fade)
    samples[-fade:] *= np.linspace(1, 0, fade)
    samples *= (10**(-2.5/20))/max(float(np.abs(samples).max()), 1e-8)
    samples = np.pad(samples, (int(.025*rate), int(.070*rate)))
    assert np.isfinite(samples).all()
    return np.round(samples*32767).astype('<i2')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--raw', required=True, type=Path)
    parser.add_argument('--out', type=Path, default=Path(__file__).resolve().parents[2]/'assets/end-game/v6')
    args = parser.parse_args()
    if os.name == 'nt':
        import ctypes
        ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(), 0x40)
    began = time.perf_counter()
    source = json.loads((args.raw/'generation.json').read_text())
    args.out.mkdir(parents=True, exist_ok=True)
    rows = []
    for item in source['files']:
        raw_path = args.raw/item['file']
        assert hashlib.sha256(raw_path.read_bytes()).hexdigest() == item['sha256']
        samples = finish(read_wav(raw_path), item['id']=='death')
        path = args.out/f"warden-{item['id']}.wav"
        with wave.open(str(path), 'wb') as wav:
            wav.setparams((1, 2, 24000, 0, 'NONE', 'not compressed'))
            wav.writeframes(samples.tobytes())
        rows.append(dict(id=item['id'],file=path.name,text=item['text'],source_sha256=item['sha256'],
                         bytes=path.stat().st_size,frames=len(samples),seconds=round(len(samples)/24000,5),
                         sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
    manifest = dict(schema_version=1, format=dict(container='WAV',encoding='PCM signed 16-bit little-endian',
                    channels=1,sample_rate=24000), files=rows, total_bytes=sum(r['bytes'] for r in rows),
                    provenance=source, processing=dict(script='tools/end-game/v6_finish_voice.py',
                    pitch_rate=.98,pitch_semitones=round(12*math.log2(.98),4),highpass_hz=55,lowpass_hz=7600,
                    bass_body_gain=.10,saturation_drive=1.20,rasp_hz=39,rasp_depth=.025,death_initial_rasp_depth=.050,
                    peak_dbfs=-2.5,fade_seconds=.012,trim_pre_roll_seconds=.160,trim_post_roll_seconds=.055,
                    leading_pad_seconds=.025,trailing_pad_seconds=.070),
                    processing_seconds=round(time.perf_counter()-began,4),
                    verification='Run tools/end-game/v6_verify_voice.py for decoded waveform checks. Speech phonemes/graphemes are recorded; no speaker playback or in-game audio review was performed by the generator.')
    (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8',newline='\n')
    print(json.dumps(dict(files=rows,total_bytes=manifest['total_bytes'],processing_seconds=manifest['processing_seconds']),indent=2))


if __name__ == '__main__':
    main()
