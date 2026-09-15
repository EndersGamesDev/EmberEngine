"""Retired: stage the historical End Game Pages tree, preserving every peer path.

This tool cannot run against the current tree and is kept only as the record of
how End Game was staged before the page was converted. `main` refuses rather
than pretending; `assemble` is still exercised offline by `publish_test.py`,
which is what keeps the frozen V1-V11 preservation rule under test.

Six of the files in FILES are no longer tracked sources: `main.js`,
`quality.js`, `dialogue.js`, `castle-audio.js`, `castle-ui.js` and
`voice-lines.js` are compiler output now, emitted from the templates beside
them. Repointing this tool at the emitted tree would not be enough to make it
correct, because the bytes that ship also carry the loader cache stamp that
`deploy/deploy-pages.sh` writes; reproducing that here would put a second,
drifting answer beside the assembler that already owns the question and
already assembles this page from the same inputs.
"""
from pathlib import Path
import hashlib
import json
import shutil

ROOT = Path(__file__).resolve().parents[2]
SLOT = Path('games/end-game/v12')
FILES = ['index.html', 'main.js', 'quality.js', 'style.css', 'cover.png', 'prologue.mp4', 'ambience.wav', 'dialogue.js', 'warden-movement.wav', 'warden-unlocking.wav', 'warden-sword.wav', 'warden-death.wav', 'castle-audio.js', 'castle-ui.js', 'voice-lines.js', 'boss-intro.wav', 'boss-phase2.wav', 'boss-defeat.wav', 'escape-clue.wav', 'escape-ending.wav', 'castle-ambience.wav']
ACCENT = '.card[data-game="end-game"]::before { height: 190px; opacity: .78; background: linear-gradient(0deg, #0b1426, transparent), url("games/end-game/v12/cover.png") center / cover; }'


def assemble(source, destination, commit):
    page = destination / SLOT
    page.mkdir(parents=True, exist_ok=True)
    for name in FILES:
        shutil.copyfile(source / 'web' / SLOT / name, page / name)
    (page / 'pkg').mkdir(exist_ok=True)
    for name in ['end_game.js', 'end_game_bg.wasm']:
        shutil.copyfile(source / 'web' / SLOT / 'pkg' / name, page / 'pkg' / name)
    manifest = {name: hashlib.sha256((page / name).read_bytes()).hexdigest() for name in FILES + ['pkg/end_game.js', 'pkg/end_game_bg.wasm']}
    (page / 'version.json').write_text(json.dumps({'game': 'end-game', 'version': '12.0.0', 'source': commit, 'files': manifest}, indent=2) + '\n', encoding='utf-8', newline='\n')
    catalog_path = destination / 'games.json'
    current = json.loads(catalog_path.read_text(encoding='utf-8'))
    source_catalog = json.loads((source / 'web/games.json').read_text(encoding='utf-8'))
    game = next(g for g in source_catalog['games'] if g['id'] == 'end-game')
    current['games'] = [g for g in current['games'] if g['id'] != 'end-game']
    current['games'].insert(1, game)
    catalog_path.write_text(json.dumps(current, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
    landing = destination / 'index.html'
    text = landing.read_text(encoding='utf-8')
    # Drop the accent this game already carries before adding the current
    # one: an append leaves a dead rule per release, naming the cover of a
    # version nobody serves, and the pile only ever grows.
    marker = '.card[data-game="end-game"]::before'
    text = chr(10).join(l for l in text.split(chr(10)) if marker not in l)
    if ACCENT not in text:
        if '</style>' not in text:
            raise RuntimeError('Landing page style structure changed; inspect before publishing')
        text = text.replace('</style>', ACCENT + '\n</style>', 1)
        landing.write_text(text, encoding='utf-8', newline='\n')


def main():
    # The staging flow this used to drive — clean-tree check, built-file check,
    # a detached worktree on the retired gh-pages base, then the scoped diff —
    # is in this file's history rather than here, because keeping it runnable
    # would mean keeping it wrong.
    raise SystemExit(
        'end-game publish is retired: six of the files it staged are compiler '
        'output since the v12 page became templates, and the bytes that ship '
        'also carry the loader stamp written by deploy/deploy-pages.sh, which '
        'assembles this page from the same inputs. Use that assembler.'
    )


if __name__ == '__main__':
    main()
