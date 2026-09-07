"""Build the 44-second V3 trailer from reviewed art, genuine play and generated sound.

No game state is synthesized. Raw MediaRecorder footage and generator ledgers
stay in target; this reproducible edit publishes only the finished media.
Requires Pillow, numpy and imageio-ffmpeg (project-local target pydeps supported).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import wave

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'target/league-launch/pydeps'))
import imageio_ffmpeg
import numpy as np
from PIL import Image, ImageDraw, ImageFont

FFMPEG = imageio_ffmpeg.get_ffmpeg_exe()
WORK = ROOT / 'target/league-launch/edit'
MEDIA = ROOT / 'web/games/league/media'
ART = ROOT / 'target/league-launch/assets'
GAME = ROOT / 'target/league-launch/gameplay'
W, H, FPS, DURATION = 1280, 720, 24, 44
INK, GOLD, MINT = '#f5f0df', '#dbb978', '#80dccc'
FONT = Path('C:/Windows/Fonts')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(args, **kwargs):
    result = subprocess.run([FFMPEG, '-hide_banner', '-loglevel', 'error', '-y', *map(str, args)],
                            check=True, capture_output=True, **kwargs)
    return result.stdout


def save_json(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')


def font(size, serif=False, bold=False):
    return ImageFont.truetype(str(FONT / ('cambriab.ttf' if serif else 'segoeuib.ttf' if bold else 'segoeui.ttf')), size)


def text(draw, xy, label, size, color=INK, serif=False, bold=False, anchor=None):
    draw.text(xy, label, font=font(size, serif, bold), fill=color, anchor=anchor,
              stroke_width=0)


def overlay(name, title, subtitle, kicker, centered=False, gameplay=False):
    image = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    if centered:
        draw.rectangle((0, 0, W, H), fill=(3, 17, 19, 155))
        draw.line((530, 206, 750, 206), fill=GOLD, width=2)
        text(draw, (W/2, 237), kicker, 19, MINT, bold=True, anchor='mm')
        for i, line in enumerate(title.split('\n')):
            text(draw, (W/2, 305+i*77), line, 78 if name != 'outro' else 92, serif=True, anchor='mm')
        base = 390 if '\n' not in title else 459
        text(draw, (W/2, base+26), subtitle, 25, anchor='mm')
        if name == 'outro':
            draw.rounded_rectangle((431, 495, 849, 553), radius=3, fill=(10, 36, 37, 220), outline=GOLD, width=1)
            text(draw, (W/2, 523), 'PLAY V3 IN YOUR BROWSER', 19, GOLD, bold=True, anchor='mm')
            text(draw, (W/2, 593), '1v1 DUELS  /  3v3 SQUADS', 17, INK, anchor='mm')
    else:
        # Typography is a separate video overlay, never painted into source art.
        for y in range(365, H):
            alpha = int(225 * ((y-365)/(H-365))**0.72)
            draw.line((0, y, W, y), fill=(3, 16, 20, alpha))
        draw.line((56, 526, 118, 526), fill=GOLD, width=2)
        text(draw, (56, 540), kicker, 17, MINT, bold=True)
        text(draw, (53, 567), title, 48 if gameplay else 60, serif=True)
        text(draw, (56, 645), subtitle, 23)
    if gameplay:
        draw.rounded_rectangle((W-266, 28, W-28, 65), radius=4, fill=(3, 17, 19, 220))
        text(draw, (W-147, 46), 'ACTUAL V3 GAMEPLAY', 14, MINT, bold=True, anchor='mm')
    else:
        text(draw, (W-36, 30), 'CRYSTALFORGE CHRONICLES', 13, INK, anchor='ra')
    path = WORK / f'{name}-overlay.png'
    image.save(path)
    return path


def segment(name, source, duration, layer, kind='still', start=0):
    destination = WORK / f'{name}.mp4'
    signature = hashlib.sha256(json.dumps([sha(source), sha(layer), duration, kind, start, FPS, 'edit-v3']).encode()).hexdigest()
    ledger = WORK / f'{name}.json'
    if destination.exists() and ledger.exists() and json.loads(ledger.read_text())['signature'] == signature:
        return destination
    frames = round(duration * FPS)
    if kind == 'still':
        inputs = ['-loop', '1', '-framerate', str(FPS), '-i', source]
        visual = f"scale=1600:900:force_original_aspect_ratio=increase,crop=1600:900,zoompan=z='1.025+on*0.0002':x='iw/2-iw/zoom/2':y='ih/2-ih/zoom/2':d=1:s={W}x{H}:fps={FPS}"
    elif kind == 'motion':
        inputs = ['-i', source]
        # A forward/reverse loop keeps a short generated camera drift continuous.
        visual = f"split[forward][rev];[rev]reverse[back];[forward][back]concat=n=2:v=1:a=0,loop=loop=-1:size=50:start=0,setpts=2*N/({FPS}*TB),scale={W}:{H}:force_original_aspect_ratio=increase,crop={W}:{H},fps={FPS}"
    else:
        inputs = ['-ss', str(start), '-i', source]
        visual = f"scale={W}:{H}:force_original_aspect_ratio=increase,crop={W}:{H},fps={FPS}"
    fade = min(0.14, duration/10)
    graph = f'[0:v]{visual},setsar=1,format=rgba[base];[base][1:v]overlay=0:0:format=auto,format=yuv420p,fade=t=in:st=0:d={fade},fade=t=out:st={duration-fade}:d={fade}[out]'
    run([*inputs, '-loop', '1', '-i', layer, '-filter_complex_threads', '1', '-filter_complex', graph,
         '-map', '[out]', '-an', '-frames:v', str(frames), '-c:v', 'libx264', '-preset', 'medium',
         '-crf', '22', '-threads', '2', '-r', str(FPS), destination])
    save_json(ledger, {'signature': signature, 'source': str(source.relative_to(ROOT)), 'sourceSha256': sha(source), 'kind': kind, 'start': start, 'duration': duration})
    print(f'EDITED {name}: {duration}s', flush=True)
    return destination


def sound():
    rate = 48000
    length = DURATION * rate
    t = np.arange(length, dtype=np.float64)/rate
    mix = np.zeros((length, 2), dtype=np.float64)
    # A simple original harmonic bed, supporting the generated crystal/flame cues.
    chords = [(146.83, 174.61, 220), (116.54, 146.83, 174.61), (130.81, 174.61, 220), (130.81, 164.81, 196)]
    for index, start in enumerate(range(0, DURATION, 4)):
        count = min(5*rate, length-start*rate)
        local = np.arange(count)/rate
        envelope = np.minimum(local/1.2, 1) * np.minimum((count/rate-local)/1.5, 1)
        chord = chords[index % len(chords)]
        for channel in range(2):
            layer = sum(np.sin(2*np.pi*f*(1+channel*0.0009)*local) + 0.15*np.sin(2*np.pi*f*2*local) for f in chord)/3
            mix[start*rate:start*rate+count, channel] += layer * envelope * 0.055
    sources = []
    for name, positions, gain in [('crystalforge-audio', [0, 4, 8, 13, 18, 23, 28, 33, 38, 40], 0.18),
                                   ('ember-audio', [10.5, 15.5, 20.5, 25.5, 30.5, 35.5], 0.16)]:
        source = ART/name/(name+'.wav')
        raw = run(['-i', source, '-f', 'f32le', '-ar', str(rate), '-ac', '2', '-'])
        cue = np.frombuffer(raw, dtype=np.float32).reshape(-1, 2).astype(np.float64)
        assert np.isfinite(cue).all() and np.max(np.abs(cue)) > 0.001, f'Invalid generated audio: {name}'
        cue /= max(np.max(np.abs(cue)), 0.01)
        edge = min(int(.12*rate), len(cue)//2)
        cue[:edge] *= np.linspace(0, 1, edge)[:, None]
        cue[-edge:] *= np.linspace(1, 0, edge)[:, None]
        for when in positions:
            offset = round(when*rate)
            count = min(len(cue), length-offset)
            mix[offset:offset+count] += cue[:count]*gain
        sources.append({'name': name, 'sha256': sha(source), 'sourcePeak': float(np.max(np.abs(cue))), 'insertions': positions})
    envelope = np.minimum(t/1.2, 1) * np.minimum((DURATION-t)/2.0, 1)
    mix *= envelope[:, None]
    peak = np.max(np.abs(mix))
    mix *= 0.75/max(peak, 0.0001)
    audio = WORK/'score.wav'
    with wave.open(str(audio), 'wb') as output:
        output.setnchannels(2); output.setsampwidth(2); output.setframerate(rate)
        output.writeframes((mix*32767).astype('<i2').tobytes())
    return audio, {'sources': sources, 'peak': float(np.max(np.abs(mix))), 'rms': float(np.sqrt(np.mean(mix**2))), 'originalComposition': 'Four soft chord voicings written for this trailer; no sampled music or voice.'}


def timestamp(seconds):
    return f'00:{int(seconds)//60:02d}:{seconds%60:06.3f}'


def validate(path):
    raw = run(['-i', path, '-vf', 'fps=4,scale=160:90', '-an', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-'])
    frames = np.frombuffer(raw, np.uint8).reshape(-1, 90, 160, 3)
    means = frames.mean(axis=(1, 2, 3))
    deviations = frames.std(axis=(1, 2)).mean(axis=1)
    # Brief authored cut fades are allowed; sustained black/flat video is not.
    bad = (means < 2) | (deviations < 2)
    longest, run_length = 0, 0
    for flag in bad:
        run_length = run_length+1 if flag else 0
        longest = max(longest, run_length)
    assert len(frames) >= (DURATION-.5)*4, 'Trailer truncated'
    assert longest <= 1, 'Sustained black or flat frames'
    board = Image.new('RGB', (640, 90*6), '#071719')
    times = np.linspace(1, DURATION-1, 24)
    draw = ImageDraw.Draw(board)
    for i, when in enumerate(times):
        board.paste(Image.fromarray(frames[min(round(when*4), len(frames)-1)]), ((i%4)*160, (i//4)*90))
        draw.text(((i%4)*160+4, (i//4)*90+3), f'{when:.1f}s', fill='white')
    board.save(WORK/'contact-sheet.png')
    return {'sampledFrames': len(frames), 'fps': 4, 'meanMin': float(means.min()), 'meanMax': float(means.max()), 'longestBlackOrFlatSeconds': longest/4, 'durationSeconds': len(frames)/4}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--motion', type=Path, help='Reviewed valid Wan clip; never pass the rejected black take')
    args = parser.parse_args()
    started = time.time()
    if os.name == 'nt':
        import ctypes
        ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(), 0x40)
    WORK.mkdir(parents=True, exist_ok=True); MEDIA.mkdir(parents=True, exist_ok=True)
    story = json.loads((ROOT/'web/games/league/story.json').read_text(encoding='utf-8'))
    segments, cues, timeline = [], [], []
    elapsed = 0

    def add(name, source, duration, title, subtitle, kicker, *, kind='still', start=0, centered=False, caption=None):
        nonlocal elapsed
        layer = overlay(name, title, subtitle, kicker, centered=centered, gameplay=kind=='gameplay')
        segments.append(segment(name, source, duration, layer, kind, start))
        cues.append((elapsed+.18, elapsed+duration-.15, caption or f'{title.replace(chr(10), " ")}\n{subtitle}'))
        timeline.append({'id': name, 'in': elapsed, 'out': elapsed+duration, 'kind': kind, 'source': str(source.relative_to(ROOT)), 'sha256': sha(source)})
        elapsed += duration

    intro_source = args.motion.resolve() if args.motion else MEDIA/'hero.webp'
    if args.motion:
        pixels = run(['-i', intro_source, '-vf', 'scale=80:48', '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-'])
        frames = np.frombuffer(pixels, np.uint8).reshape(-1, 48, 80, 3)
        spatial = frames.std(axis=(1, 2)).mean(axis=1)
        movement = np.abs(np.diff(frames.astype(np.float32), axis=0)).mean()
        assert len(frames) == 25 and np.all(spatial > 2) and movement > .01, 'Opening requires 25 non-flat frames with actual motion'
    add('intro', intro_source, 5, 'The Crystalforge\nis awake.', 'A garden of light. An inheritance worth fighting for.', 'EMBER ORIGINAL', kind='motion' if args.motion else 'still', centered=True)
    add('bridge', MEDIA/'chapter-3.webp', 3, 'Two hearths. One bridge.', 'Five strangers have come to collect.', 'THE CRYSTALFORGE CHRONICLES')
    keys = {'swarm':'swarm', 'emberknight':'knight', 'hallow':'hallow', 'bogmaw':'maw', 'tessera':'tessera'}
    attacks = {'swarm':'Command drones. Cut through the lane.', 'emberknight':'Bring the flame. Hold your ground.', 'hallow':'Mend the wounded. Keep them standing.', 'bogmaw':'Pull them close. Make the river yours.', 'tessera':'Lay the trap. Take their time.'}
    hooks = {'swarm':'To close the count.', 'emberknight':'To put the fire out.', 'hallow':'To keep the hearth lit.', 'bogmaw':'To take the ravine back.', 'tessera':'To wind it up again.'}
    for index, champion in enumerate(story['champions']):
        cid = champion['id']
        add(cid+'-story', MEDIA/(cid+'-story.webp'), 2.5, champion['name'], hooks[cid], f'0{index+1} / FIVE REASONS TO FIGHT')
        add(cid+'-game', GAME/(keys[cid]+'.webm'), 2.5, champion['name'], attacks[cid], champion['role'].upper(), kind='gameplay', start=.5)
    add('battle', GAME/'knight.webm', 5, 'Your lane. Your fight.', 'Claim the Courts. Break the enemy core.', '1v1 DUELS / 3v3 SQUADS', kind='gameplay', start=5.5)
    add('outro', MEDIA/'hero.webp', 6, 'UltimateLegue', 'One lane. Five reasons to fight.', 'V3', centered=True)
    assert elapsed == DURATION
    concat = WORK/'concat.txt'
    concat.write_text(''.join(f"file '{p.as_posix()}'\n" for p in segments), encoding='utf-8', newline='\n')
    score, audio_report = sound()
    # Keep the last reviewed edit intact if encoding or frame validation fails.
    destination = WORK/'trailer-candidate.mp4'
    run(['-f', 'concat', '-safe', '0', '-i', concat, '-i', score, '-map', '0:v', '-map', '1:a', '-c:v', 'copy',
         '-c:a', 'aac', '-b:a', '160k', '-t', DURATION, '-movflags', '+faststart', destination])
    captions = 'WEBVTT\n\n' + '\n\n'.join(f'{i+1}\n{timestamp(a)} --> {timestamp(b)}\n{copy}' for i, (a,b,copy) in enumerate(cues)) + '\n'
    caption_file = WORK/'trailer-candidate.vtt'
    caption_file.write_text(captions, encoding='utf-8', newline='\n')
    poster_png = WORK/'poster.png'
    run(['-ss', '40', '-i', destination, '-frames:v', '1', poster_png])
    poster_file = WORK/'trailer-poster-candidate.webp'
    Image.open(poster_png).convert('RGB').save(poster_file, quality=91, method=6)
    proof = validate(destination)
    report = {'title':'UltimateLegue V3 — One lane. Five reasons to fight.', 'duration':DURATION,
              'resolution':[W,H], 'fps':FPS, 'videoCodec':'H.264', 'audioCodec':'AAC',
              'motionSource':'Wan 2.2 TI2V-5B' if args.motion else 'Temporary still-only preview; pending Wan repair',
              'timeline':timeline, 'audio':audio_report, 'validation':proof,
              'sha256':sha(destination), 'captionsSha256':sha(caption_file), 'posterSha256':sha(poster_file),
              'bytes':destination.stat().st_size, 'elapsedSeconds':round(time.time()-started,3)}
    # Each replacement is atomic. The final provenance gate checks all three
    # hashes, so a crash between replacements cannot bless a mixed generation.
    for source, name in [(destination, 'trailer.mp4'), (caption_file, 'trailer.vtt'), (poster_file, 'trailer-poster.webp')]:
        os.replace(source, MEDIA/name)
    save_json(WORK/'report.json', report)
    print(json.dumps(report, indent=2), flush=True)


if __name__ == '__main__':
    main()
