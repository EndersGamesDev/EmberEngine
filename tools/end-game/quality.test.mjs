import test from 'node:test';
import assert from 'node:assert/strict';
import { Quality } from '../../web/games/end-game/v11/quality.js';

test('5K and hardware dimension budgets are respected on dense ultrawide displays', () => {
  for (const maxDimension of [2048, 4096, 8192]) {
    const q = new Quality({width: 7680, height: 2160, dpr: 2, memory: 16, maxDimension});
    q.set('native');
    assert.ok(q.scale * 7680 * 2 <= Math.min(maxDimension, 5120) + 0.01);
  }
});
test('severe overload reduces resolution and invalid samples do not poison it', () => {
  const q = new Quality({width: 1920, height: 1080, memory: 16});
  const before = q.scale;
  for (let i = 0; i < 30; i++) q.sample(150);
  assert.ok(q.scale < before);
  q.sample(NaN); q.sample(Infinity); q.sample(-1);
  assert.ok(Number.isFinite(q.scale));
});
test('native mode stays fixed; auto improves only after sustained headroom', () => {
  const q = new Quality({width: 2560, height: 1440, memory: 4});
  const before = q.scale;
  for (let i = 0; i < 120; i++) q.sample(16);
  assert.equal(q.scale, before);
  for (let i = 0; i < 360; i++) q.sample(16);
  assert.ok(q.scale > before);
  q.set('native'); const native = q.scale;
  for (let i = 0; i < 240; i++) q.sample(40);
  assert.equal(q.scale, native);
});
