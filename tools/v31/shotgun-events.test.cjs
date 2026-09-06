'use strict';
require('node:os').setPriority(0, require('node:os').constants.priority.PRIORITY_LOW);
const test = require('node:test');
const assert = require('node:assert/strict');
const { shotgunEndpoints } = require('./shotgun-events.cjs');
const fixture = (tick = 9123, owner = 7) => Array.from({ length: 8 }, (_, pellet) => ({
  t: 'shot', owner, weapon: 8, projectile_id: tick * 4096 + owner * 16 + pellet,
  x0: 1, y0: 1.45, z0: 2, x1: 25, y1: 20 + pellet * 0.1, z1: 3 + pellet * 0.2,
  hit: 0, cover: 255, victim: 255, normal: [0, 0, 0],
}));
test('one shell decodes eight exact distinct IDs without 32-bit JS bitwise truncation', () => {
  const input = fixture(2 ** 40 - 1, 255), result = shotgunEndpoints(input.slice().reverse(), 255);
  assert.equal(result.tickModulo40, 2 ** 40 - 1);
  assert.deepEqual(result.projectileIds, input.map(event => event.projectile_id));
  assert(result.projectileIds.every(Number.isSafeInteger));
});
test('missing, duplicated, nonfinite, wrong-owner, mixed-shell and out-of-range events fail closed', () => {
  for (const mutate of [
    events => events.pop(),
    events => events.push(events[0]),
    events => { events[7].projectile_id = events[0].projectile_id; },
    events => { events[7].projectile_id += 4096; },
    events => { events[7].projectile_id = 2 ** 52; },
    events => { events[7].projectile_id += 0.5; },
    events => { events[7].owner = 5; },
    events => { events[7].weapon = 1; },
    events => { events[7].y1 = Infinity; },
    events => { events[7].projectile_id += 1; },
  ]) {
    const input = fixture(); mutate(input); assert.throws(() => shotgunEndpoints(input, 7));
  }
});
