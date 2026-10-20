import { test } from 'node:test';
import assert from 'node:assert/strict';
import { compareStableVersions, verifyAssets } from './prepare-publication.mjs';
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

test('publication accepts the current repository and rejects other destinations', () => {
  const dir = mkdtempSync(join(tmpdir(), 'takedock-publication-'));
  mkdirSync(join(dir, 'src-tauri'));
  mkdirSync(join(dir, 'release'));
  writeFileSync(
    join(dir, 'src-tauri/tauri.conf.json'),
    JSON.stringify({ version: '0.1.3' }),
  );
  const artifacts = {
    'latest.json': { version: '0.1.3' },
    'release-verification.json': {
      version: '0.1.3',
      commit: 'tested-commit',
      signature_verified: true,
      packaged_resources_match: true,
    },
    'acceptance.json': {
      commit: 'tested-commit',
      conclusion: 'passed',
      tests: Array.from({ length: 4 }, () => ({ passed: true })),
    },
  };
  for (const [name, value] of Object.entries(artifacts))
    writeFileSync(join(dir, 'release', name), JSON.stringify(value));
  const sums = readdirSync(join(dir, 'release')).map(
    (name) =>
      `${createHash('sha256')
        .update(readFileSync(join(dir, 'release', name)))
        .digest('hex')}  ${name}`,
  );
  writeFileSync(join(dir, 'release/SHA256SUMS.txt'), sums.join('\n'));
  // Keep asset/version validation real; replace only the external GitHub API call.
  const preload = join(dir, 'github-fixture.mjs');
  writeFileSync(
    preload,
    `
    import childProcess from 'node:child_process';
    import { syncBuiltinESMExports } from 'node:module';
    import assert from 'node:assert/strict';
    childProcess.spawnSync = (command, args) => {
      if (command !== 'gh' || JSON.stringify(args) !== JSON.stringify(['api', 'repos/wip-bunker-42/TakeDock/releases/latest', '--include']))
        throw new Error('Unexpected GitHub API call');
      return { status: 0, stdout: 'HTTP/2.0 200 OK\\n\\n{"tag_name":"v0.1.2"}', stderr: '' };
    };
    syncBuiltinESMExports();
  `,
  );
  const script = fileURLToPath(
    new URL('./prepare-publication.mjs', import.meta.url),
  );
  for (const repository of [
    'wip-bunker-42/TakeDock',
    'aimansour/TakeDock',
    'unrelated/TakeDock',
  ]) {
    const result = spawnSync(
      process.execPath,
      ['--import', pathToFileURL(preload).href, script],
      {
        cwd: dir,
        encoding: 'utf8',
        env: {
          ...process.env,
          RELEASE_TAG: 'v0.1.3',
          GITHUB_SHA: 'tested-commit',
          GITHUB_REPOSITORY: repository,
        },
      },
    );
    if (repository === 'wip-bunker-42/TakeDock') {
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /PASS: tag, checksums, acceptance/);
    } else {
      assert.notEqual(
        result.status,
        0,
        `Unexpected publication to ${repository}`,
      );
      assert.match(result.stderr, /AssertionError/);
    }
  }
});
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
