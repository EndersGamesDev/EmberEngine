"""Offline release assembly checks. No network or git mutations."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('publish', Path(__file__).with_name('publish.py'))
publish = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publish)


class ReleaseAssembly(unittest.TestCase):
    def test_preserves_other_games_and_publishes_complete_local_bundle(self):
        with tempfile.TemporaryDirectory(prefix='end-game-assembly-test-') as temp:
            root = Path(temp); source = root / 'source'; dest = root / 'pages'
            assets = source / 'web' / publish.SLOT
            (assets / 'pkg').mkdir(parents=True); dest.mkdir()
            for name in publish.FILES + ['pkg/end_game.js', 'pkg/end_game_bg.wasm']:
                (assets / name).write_bytes(name.encode())
            old = {'games': [{'id': 'league', 'versions': [{'v': 'future', 'live': True}], 'unknown': 'preserve'}]}
            game = {'id': 'end-game', 'versions': [{'version': '2.0.0', 'path': 'games/end-game/v2/'}]}
            (source / 'web/games.json').write_text(json.dumps({'games': [game]}))
            (dest / 'games.json').write_text(json.dumps(old))
            (dest / 'index.html').write_text('<style>existing</style><main>keep live content</main>')
            (dest / 'server.json').write_bytes(b'keep host book')
            frozen = dest / 'games/end-game/v1/pkg/end_game_bg.wasm'
            frozen.parent.mkdir(parents=True, exist_ok=True)
            frozen.write_bytes(b'original published v1')
            publish.assemble(source, dest, 'source-sha')
            self.assertEqual(frozen.read_bytes(), b'original published v1')
            catalog = json.loads((dest / 'games.json').read_text())
            self.assertEqual(catalog['games'][0], old['games'][0])
            self.assertEqual(catalog['games'][1], game)
            self.assertEqual((dest / 'server.json').read_bytes(), b'keep host book')
            self.assertIn('keep live content', (dest / 'index.html').read_text())
            stamp = json.loads((dest / publish.SLOT / 'version.json').read_text())
            self.assertEqual(stamp['source'], 'source-sha')
            self.assertEqual(len(stamp['files']), 9)
            publish.assemble(source, dest, 'source-sha')
            self.assertEqual((dest / 'index.html').read_text().count(publish.ACCENT), 1)


if __name__ == '__main__':
    unittest.main()
