import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareStableVersions, verifyAssets } from './prepare-publication.mjs';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
test('stable publication rejects stale or invalid versions', () => {
  assert.equal(compareStableVersions('0.2.0', '0.1.9'), 1);
  assert.equal(compareStableVersions('0.1.0', '0.2.0'), -1);
  assert.equal(compareStableVersions('1.0.0', '1.0.0'), 0);
  assert.throws(() => compareStableVersions('01.0.0', '1.0.0'));
});
test('release checksums reject corrupt, extra and escaping assets', () => {
  const dir = mkdtempSync(join(tmpdir(), 'takedock-checksums-'));
  const sum = createHash('sha256').update('verified').digest('hex');
  writeFileSync(join(dir, 'asset.txt'), 'verified');
  writeFileSync(join(dir, 'SHA256SUMS.txt'), `${sum}  asset.txt\n`);
  verifyAssets(dir);
  writeFileSync(join(dir, 'asset.txt'), 'changed');
  assert.throws(() => verifyAssets(dir));
  writeFileSync(join(dir, 'asset.txt'), 'verified');
  writeFileSync(join(dir, 'extra.txt'), 'extra');
  assert.throws(() => verifyAssets(dir));
  writeFileSync(join(dir, 'SHA256SUMS.txt'), `${sum}  ../asset.txt\n`);
  assert.throws(() => verifyAssets(dir));
});
