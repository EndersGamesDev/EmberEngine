"""Publish only End Game and its launcher card; preserve all other live games.

Run after a clean committed build: python tools/end-game/publish.py --publish
Without --publish, assemble and report the exact scoped diff only.
"""
from pathlib import Path
import argparse
import hashlib
import json
import shutil
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
SLOT = Path('games/end-game/v9')
FILES = ['index.html', 'main.js', 'quality.js', 'style.css', 'cover.png', 'prologue.mp4', 'ambience.wav', 'dialogue.js', 'warden-movement.wav', 'warden-unlocking.wav', 'warden-sword.wav', 'warden-death.wav']
ACCENT = '.card[data-game="end-game"]::before { height: 190px; opacity: .78; background: linear-gradient(0deg, #0b1426, transparent), url("games/end-game/v1/cover.png") center / cover; }'


def git(*args, cwd=ROOT):
    return subprocess.check_output(['git', *args], cwd=cwd, text=True, encoding='utf-8').strip()


def assemble(source, destination, commit):
    page = destination / SLOT
    page.mkdir(parents=True, exist_ok=True)
    for name in FILES:
        shutil.copyfile(source / 'web' / SLOT / name, page / name)
    (page / 'pkg').mkdir(exist_ok=True)
    for name in ['end_game.js', 'end_game_bg.wasm']:
        shutil.copyfile(source / 'web' / SLOT / 'pkg' / name, page / 'pkg' / name)
    manifest = {name: hashlib.sha256((page / name).read_bytes()).hexdigest() for name in FILES + ['pkg/end_game.js', 'pkg/end_game_bg.wasm']}
    (page / 'version.json').write_text(json.dumps({'game': 'end-game', 'version': '9.0.0', 'source': commit, 'files': manifest}, indent=2) + '\n', encoding='utf-8', newline='\n')
    catalog_path = destination / 'games.json'
    current = json.loads(catalog_path.read_text(encoding='utf-8'))
    source_catalog = json.loads((source / 'web/games.json').read_text(encoding='utf-8'))
    game = next(g for g in source_catalog['games'] if g['id'] == 'end-game')
    current['games'] = [g for g in current['games'] if g['id'] != 'end-game']
    current['games'].insert(1, game)
    catalog_path.write_text(json.dumps(current, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    landing = destination / 'index.html'
    text = landing.read_text(encoding='utf-8')
    if ACCENT not in text:
        if '</style>' not in text:
            raise RuntimeError('Landing page style structure changed; inspect before publishing')
        text = text.replace('</style>', ACCENT + '\n</style>', 1)
        landing.write_text(text, encoding='utf-8', newline='\n')


def main():
    started = time.perf_counter()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--publish', action='store_true')
    args = parser.parse_args()
    if git('status', '--porcelain'):
        raise RuntimeError('Commit the validated source before staging a release')
    commit = git('rev-parse', 'HEAD')
    for name in FILES + ['pkg/end_game.js', 'pkg/end_game_bg.wasm']:
        if not (ROOT / 'web' / SLOT / name).is_file():
            raise RuntimeError(f'Missing built file: {name}')
    if (ROOT / 'web' / SLOT / 'pkg/end_game_bg.wasm').read_bytes()[:4] != b'\0asm':
        raise RuntimeError('Invalid WASM artifact')
    git('fetch', 'origin', 'gh-pages')
    # Retain the isolated output for inspection; never delete a computed tree.
    stage = Path(tempfile.mkdtemp(prefix='end-game-v9-pages-')) / 'pages'
    git('worktree', 'add', '--detach', str(stage), 'FETCH_HEAD')
    assemble(ROOT, stage, commit)
    git('add', '--', str(SLOT), 'games.json', 'index.html', cwd=stage)
    changes = git('diff', '--cached', '--name-only', cwd=stage).splitlines()
    if not changes:
        print('No changes to publish')
        return
    if any(p not in ['games.json', 'index.html'] and not p.startswith(SLOT.as_posix() + '/') for p in changes):
        raise RuntimeError('Refusing changes outside End Game and its launcher entry')
    print(git('diff', '--cached', '--stat', cwd=stage))
    if args.publish:
        git('commit', '-m', f'feat(end-game): publish castle exploration from {commit[:12]}', cwd=stage)
        # An overlapping publication rejects this normal push; it is never forced.
        git('push', 'origin', 'HEAD:gh-pages', cwd=stage)
        print('Published source', commit)
    print('Staging directory:', stage)
    print('Wall seconds:', round(time.perf_counter() - started, 2))


if __name__ == '__main__':
    main()
