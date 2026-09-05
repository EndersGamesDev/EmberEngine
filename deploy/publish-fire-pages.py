#!/usr/bin/env python3
"""Publish ONLY Fire V2 and its catalog entry; preserve every other game."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def main():
    started = time.monotonic()
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--push', action='store_true', help='Publish the prepared Pages commit')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    source = root / 'web/games/fire/v2'
    for name in ['fire.js', 'fire_bg.wasm']:
        if not (source / 'pkg' / name).is_file():
            raise SystemExit(f'Missing compiled artifact: {source / "pkg" / name}')
    if git(root, 'status', '--porcelain', '--untracked-files=no'):
        raise SystemExit('Commit tracked source changes before publishing.')
    stamp = git(root, 'rev-parse', '--short', 'HEAD')
    desired = next(g for g in json.loads((root / 'web/games.json').read_text(encoding='utf-8'))['games'] if g['id'] == 'fire')
    git(root, 'fetch', 'origin', 'gh-pages')
    work = Path(tempfile.mkdtemp(prefix='ember-fire-pages-'))
    try:
        git(root, 'worktree', 'add', '--detach', str(work), 'FETCH_HEAD')
        target = work / 'games/fire/v2'
        target.mkdir(parents=True, exist_ok=True)
        for name in ['index.html', 'style.css', 'race.js', 'garage.js']:
            text = (source / name).read_text(encoding='utf-8').replace('__FIRE_BUILD__', stamp)
            (target / name).write_text(text, encoding='utf-8', newline='\n')
        (target / 'pkg').mkdir(exist_ok=True)
        for name in ['fire.js', 'fire_bg.wasm']:
            shutil.copy2(source / 'pkg' / name, target / 'pkg' / name)
        (target / 'release.json').write_text(json.dumps({'commit': stamp, 'protocol': 2}) + '\n', encoding='utf-8')
        catalog = json.loads((work / 'games.json').read_text(encoding='utf-8'))
        untouched = [g for g in catalog['games'] if g['id'] != 'fire']
        for i, game in enumerate(catalog['games']):
            if game['id'] == 'fire':
                catalog['games'][i] = desired
                break
        else:
            raise SystemExit('Fire is absent from the remote catalog; refusing to replace it.')
        assert [g for g in catalog['games'] if g['id'] != 'fire'] == untouched
        (work / 'games.json').write_text(json.dumps(catalog, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
        git(work, 'add', '--', 'games/fire/v2', 'games.json')
        changed = git(work, 'diff', '--cached', '--name-only').splitlines()
        if not changed:
            print('Fire Pages already matches this build.')
            return
        if any(p != 'games.json' and not p.startswith('games/fire/v2/') for p in changed):
            raise SystemExit('Unexpected path in Fire-only publish.')
        git(work, 'commit', '-m', f'Fire Racer V2 GT Circuit ({stamp}); preserve other games')
        print(f'Prepared {len(changed)} Fire-only paths, source {stamp}.')
        if args.push:
            # A concurrent publisher wins safely: normal push rejects stale history.
            # Re-run this command to compose against the newest Pages tree.
            git(work, 'push', 'origin', 'HEAD:gh-pages')
            print('Published Fire V2; other pages and server.json preserved.')
        else:
            print('Review prepared commit:', git(work, 'rev-parse', 'HEAD'))
    finally:
        # Only our checked absolute temporary worktree is removed.
        git(root, 'worktree', 'remove', '--force', str(work))
    print(f'WALL_SECONDS={time.monotonic() - started:.2f}')


if __name__ == '__main__':
    main()
