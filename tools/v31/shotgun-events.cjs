// Pure assertions over real protocol24 endpoint events. No sockets or services.
'use strict';
const assert = require('node:assert/strict');

function shotgunEndpoints(events, owner) {
  assert(Number.isInteger(owner) && owner >= 0 && owner < 256, 'Invalid original player ID');
  assert.equal(events.length, 8, 'One shell must produce exactly eight endpoint events');
  const ordered = [...events].sort((a, b) => a.projectile_id - b.projectile_id);
  const ids = ordered.map(event => event.projectile_id);
  assert.equal(new Set(ids).size, 8, 'Each pellet needs a distinct projectile ID');
  for (const event of ordered) {
    assert.equal(event.t, 'shot'); assert.equal(event.weapon, 8); assert.equal(event.owner, owner);
    assert(Number.isSafeInteger(event.projectile_id) && event.projectile_id >= 0 && event.projectile_id < 2 ** 52,
      'Projectile ID must fit the exact 52-bit wire contract');
    assert(['x0', 'y0', 'z0', 'x1', 'y1', 'z1'].every(key => Number.isFinite(event[key])), 'Endpoint contains invalid coordinates');
    assert.equal(Math.floor(event.projectile_id / 16) % 256, owner, 'Projectile ID lost its original owner');
  }
  const shellKey = Math.floor(ids[0] / 16);
  assert(ids.every(id => Math.floor(id / 16) === shellKey), 'Pellets belong to different shells');
  assert.deepEqual(ids.map(id => id % 16), [0, 1, 2, 3, 4, 5, 6, 7], 'Pellet IDs must be exactly zero through seven');
  return { shellKey, tickModulo40: Math.floor(ids[0] / 4096), projectileIds: ids, endpoints: ordered };
}
module.exports = { shotgunEndpoints };
