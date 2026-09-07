"""Record the reviewed launch media selection and its generator provenance."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MEDIA = ROOT / 'web/games/league/media'
RAW = ROOT / 'target/league-launch/assets'


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    choices = {
        'hero.webp': ('hero', 'An ivory bridge above an emerald river. Original world concept illustration.'),
        'chapter-1.webp': ('chapter-1', 'Mossy bridge and quiet river; an atmospheric setting for The Count.'),
        'chapter-2.webp': ('chapter-2', 'Knight and hooded healer in warm ivory ruins; story illustration, not gameplay.'),
        'chapter-3.webp': ('chapter-3', 'Bronze and ivory bridges above turquoise water; setting for The Beat.'),
        'emberknight-story.webp': ('emberknight-story', 'Armored flame knight, red cloak and glowing sword before a citadel.'),
        'hallow-story.webp': ('hallow-story-2', 'Selected second pass preserves the faceless hood, ivory robes and staff.'),
        'bogmaw-story.webp': ('bogmaw-story', 'Mossy ravine guardian in its river; yellow light, jade hide and a heavy silhouette.'),
        'tessera-story.webp': ('tessera-story', 'Silver-haired clockmaker in plum and bronze with a glowing timepiece.'),
    }
    assets = []
    for file, (job, review) in choices.items():
        record = read(RAW/job/'job.json')
        assert digest(Path(record['output'])) == record['sha256'], f'Raw image changed: {job}'
        assets.append({'file': file, 'sha256': digest(MEDIA/file), 'generator': 'SDXL 1.0 / ComfyUI',
                       'worker': 'specht32', 'job': record['id'], 'parameters': record['spec'],
                       'rawSha256': record['sha256'], 'derivation': 'RGB WebP, quality90, method6', 'review': review})
    original = next(a for a in read(ROOT/'web/games/league/v2/art/manifest.json')['assets'] if a['name'] == 'swarm')
    assets.append({'file': 'swarm-story.webp', 'sha256': digest(MEDIA/'swarm-story.webp'),
                   'generator': original['generator'], 'source': '../v2/art/swarm.webp', 'sourceSha256': original['sha256'],
                   'review': 'Reuse the established bronze/turquoise orb design. Two new candidates were rejected for legs or missing lens.'})
    report = read(ROOT/'target/league-launch/edit/report.json')
    assert report['motionSource'] == 'Wan 2.2 TI2V-5B', 'Final reviewed generated opening is still pending'
    for name, field in [('trailer.mp4','sha256'), ('trailer.vtt','captionsSha256'), ('trailer-poster.webp','posterSha256')]:
        assert digest(MEDIA/name) == report[field], f'Current {name} does not match the validated edit'
    motion = read(RAW/'crystalforge-motion-fixed/job.json')
    assert motion['pixel_validation']['passed'], 'Do not publish failed generator output'
    assert digest(Path(motion['output'])) == motion['sha256'], 'Reviewed raw video changed'
    intro = report['timeline'][0]
    assert intro['kind'] == 'motion' and intro['sha256'] == motion['sha256'], 'Edit uses a different generated opening'
    assert (ROOT/intro['source']).resolve() == Path(motion['output']).resolve(), 'Opening provenance path differs'
    audio = []
    for name in ['crystalforge-audio', 'ember-audio']:
        record = read(RAW/name/'job.json')
        assert digest(Path(record['output'])) == record['sha256'], f'Raw generated audio changed: {name}'
        used = next(cue for cue in report['audio']['sources'] if cue['name'] == name)
        assert used['sha256'] == record['sha256'], f'Trailer uses a different audio generation: {name}'
        audio.append({'name':name, 'generator':'MMAudio', 'worker':'knecht24', 'job':record['id'],
                      'parameters':record['spec'], 'rawSha256':record['sha256']})
    captures = read(ROOT/'target/league-launch/gameplay/manifest.json')['clips']
    capture_records = [{key:clip[key] for key in ('champion','durationSeconds','sha256','wasmSha256','simulationSeconds','aliveSampleFraction','acceptedCasts')} for clip in captures]
    for shot in report['timeline']:
        if shot['kind'] == 'gameplay':
            assert any(clip['sha256'] == shot['sha256'] for clip in captures), 'Gameplay differs from the actual capture manifest'
    trailer = {**report, 'generatorJob':motion['id'], 'generatorParameters':motion['spec'],
               'generatorRawSha256':motion['sha256'], 'generatedAudio':audio,
               'gameplayCaptures':capture_records,
               'captureEvidence': 'tools/league/trailer-gameplay.cjs; raw frame/state/inputs manifests retained in target/league-launch/gameplay',
               'editRecipe': 'tools/league/trailer-edit.py',
               'captionsSha256':digest(MEDIA/'trailer.vtt'), 'posterSha256':digest(MEDIA/'trailer-poster.webp')}
    data = {'schemaVersion':1, 'project':'UltimateLegue V3 — Crystalforge', 'assets':assets, 'trailer':trailer,
            'rejected':[
                {'job':'b4fed053c988','reason':'Wan temporal VAE decode produced black frames. Rejected; absent from published media.'},
                {'job':'dd023c7d-ed26-40f4-8afd-fe0b1f573e59','reason':'SW4RM candidate added legs; inconsistent with hovering orb.'},
                {'job':'711398f3-499c-48ac-a226-12a046283f3b','reason':'SW4RM candidate lost its defining lens and fins.'},
                {'job':'78cc4541-4d57-4b70-a895-9fc74d11ffc1','reason':'Hallow candidate added a visible face; selected a faceless second pass.'}],
            'rights':'Generated original game artwork and sound; original score and typography; genuine local game capture. No third-party game footage or music.'}
    (MEDIA/'manifest.json').write_text(json.dumps(data, indent=2, ensure_ascii=False)+'\n', encoding='utf-8', newline='\n')
    print(f'Recorded {len(assets)} reviewed images and a {report["duration"]}s trailer')


if __name__ == '__main__':
    main()
