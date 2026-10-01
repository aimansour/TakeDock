import { test } from 'node:test';
import assert from 'node:assert/strict';
import { releaseGate } from './release-gate.mjs';
test('publication requires every named gate to succeed', () => {
  const good = {
    rust: 'success',
    ui: 'success',
    observer: 'success',
    desktop: 'success',
  };
  assert.equal(releaseGate(good), true);
  for (const name of Object.keys(good))
    for (const result of ['failure', 'cancelled', 'skipped', undefined])
      assert.equal(releaseGate({ ...good, [name]: result }), false);
  assert.equal(releaseGate({}), false);
});
