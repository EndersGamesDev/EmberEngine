"""Bake the reviewed V4 ground materials; source images stay in the job ledger.

Run with the existing C:/hy3d/venv/Scripts/python.exe runtime. The image jobs
are submitted serially by launch-assets.py; this script never submits jobs.
"""
from pathlib import Path
import hashlib
import json
from PIL import Image
from bake_surface import mirrored, quad_glb

ROOT = Path(__file__).resolve().parents[3]
ASSETS = ROOT / 'assets/models/league/v4'
ART = ROOT / 'web/games/league/v4/art'
ROWS = [
    ('garden', 'v4-moss-3', 1024, 10.0, 5.75, True),
    ('lane', 'v4-paving', 768, 14.0, 1.4, True),
    ('court', 'v4-plaza', 1024, 1.0, 1.0, False),
]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    ASSETS.mkdir(parents=True, exist_ok=True)
    ART.mkdir(parents=True, exist_ok=True)
    rows = []
    for name, job_name, size, tx, tz, mirror in ROWS:
        folder = ROOT / 'target/league-launch/assets' / job_name
        job = json.loads((folder / 'job.json').read_text(encoding='utf-8'))
        raw = (folder / (job_name + '.png')).read_bytes()
        assert job['sha256'] == sha(raw), 'Generator bytes differ from the durable job record'
        source = Image.open(folder / (job_name + '.png')).convert('RGB')
        texture = mirrored(source, size) if mirror else source.resize((size, size), Image.Resampling.LANCZOS)
        assert texture.mode == 'RGB'
        out = ASSETS / ('surface-' + name + '.glb')
        quad_glb(name, texture, tx, tz, out, 1.0)
        preview = ART / (name + '.webp')
        texture.save(preview, 'WEBP', quality=90, method=6)
        rows.append({
            'name': name, 'job': job['id'], 'generator': 'LAN SDXL / asset-forge',
            'spec': job['spec'], 'sourceSha256': job['sha256'], 'review': 'accepted',
            'bake': {'size': size, 'mode': 'RGB8', 'mirror2x2': mirror, 'uvTiles': [tx, tz], 'brightness': 1.0},
            'glb': out.relative_to(ROOT).as_posix(), 'glbBytes': out.stat().st_size,
            'glbSha256': sha(out.read_bytes()), 'preview': preview.relative_to(ROOT).as_posix(),
            'previewSha256': sha(preview.read_bytes()),
        })
    rejected = []
    for name, reason in [('v4-moss', 'Rock-dominated instead of soft ground'),
                         ('v4-moss-2', 'Long side-view blades and excessive saturation')]:
        job = json.loads((ROOT / 'target/league-launch/assets' / name / 'job.json').read_text())
        rejected.append({'job': job['id'], 'sha256': job['sha256'], 'spec': job['spec'], 'reason': reason})
    old = ROOT / 'assets/models/league/v2'
    kept = sum(p.stat().st_size for p in old.glob('*.glb') if not p.name.startswith('surface-'))
    total = kept + sum(row['glbBytes'] for row in rows)
    assert total <= 8 * 1024 * 1024, f'Embedded art budget exceeded: {total}'
    manifest = {'version': 'v4', 'purpose': 'actual in-game surface materials',
                'embeddedArtBytes': total, 'embeddedArtBudgetBytes': 8 * 1024 * 1024,
                'assets': rows, 'rejected': rejected}
    (ART / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8', newline='\n')
    (ASSETS / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8', newline='\n')
    print(json.dumps({'embeddedArtBytes': total, 'surfaces': [{k: row[k] for k in ['name', 'glbBytes', 'glbSha256']} for row in rows]}))


if __name__ == '__main__':
    main()
