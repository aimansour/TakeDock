import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
function stable(version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version))
    throw new Error('Expected a stable semantic version');
  return version.split('.').map(BigInt);
}
export function compareStableVersions(left, right) {
  const a = stable(left),
    b = stable(right);
  for (let i = 0; i < 3; i++) {
    if (a[i] > b[i]) return 1;
    if (a[i] < b[i]) return -1;
  }
  return 0;
}
export function verifyAssets(dir) {
  const expected = new Set(['SHA256SUMS.txt']);
  for (const line of readFileSync(join(dir, 'SHA256SUMS.txt'), 'utf8')
    .trim()
    .split(/\r?\n/)) {
    const match = /^([0-9a-f]{64})  ([A-Za-z0-9_.+-]+)$/.exec(line);
    if (!match || match[2].startsWith('.') || expected.has(match[2]))
      throw new Error('Invalid checksum inventory');
    expected.add(match[2]);
    assert.equal(
      createHash('sha256')
        .update(readFileSync(join(dir, match[2])))
        .digest('hex'),
      match[1],
    );
  }
  assert.deepEqual(readdirSync(dir).sort(), [...expected].sort());
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  verifyAssets('release');
  const version = JSON.parse(
    readFileSync('src-tauri/tauri.conf.json', 'utf8'),
  ).version;
  const manifest = JSON.parse(readFileSync('release/latest.json', 'utf8'));
  const evidence = JSON.parse(
    readFileSync('release/release-verification.json', 'utf8'),
  );
  const acceptance = JSON.parse(
    readFileSync('release/acceptance.json', 'utf8'),
  );
  assert.equal(process.env.RELEASE_TAG, `v${version}`);
  assert.equal(manifest.version, version);
  assert.equal(evidence.version, version);
  assert.equal(evidence.commit, process.env.GITHUB_SHA);
  assert.equal(evidence.signature_verified, true);
  assert.equal(evidence.packaged_resources_match, true);
  assert.equal(acceptance.commit, process.env.GITHUB_SHA);
  assert.equal(acceptance.conclusion, 'passed');
  assert.equal(acceptance.tests.length, 4);
  assert.ok(acceptance.tests.every((test) => test.passed));
  const repository = process.env.GITHUB_REPOSITORY;
  assert.equal(repository, 'aimansour/TakeDock');
  const latest = spawnSync(
    'gh',
    ['api', `repos/${repository}/releases/latest`, '--include'],
    { encoding: 'utf8' },
  );
  if (latest.status === 0) {
    const separator = latest.stdout.search(/\r?\n\r?\n/);
    const release = JSON.parse(latest.stdout.slice(separator).trim());
    assert.ok(
      compareStableVersions(version, release.tag_name.replace(/^v/, '')) > 0,
      'Refusing a stale or duplicate release',
    );
  } else if (!/^HTTP\/\S+ 404 /m.test(latest.stdout)) {
    throw new Error(`Cannot verify latest release: ${latest.stderr}`);
  }
  console.log(
    'PASS: tag, checksums, acceptance, package evidence and release ordering.',
  );
}
