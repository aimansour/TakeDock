import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import { verifyAssets } from './prepare-publication.mjs';
const tag = process.env.RELEASE_TAG;
const draft = JSON.parse(
  execFileSync('gh', ['release', 'view', tag, '--json', 'isDraft,assets'], {
    encoding: 'utf8',
  }),
);
assert.equal(draft.isDraft, true);
assert.deepEqual(
  draft.assets.map((asset) => asset.name).sort(),
  readdirSync('release').sort(),
);
const downloaded = mkdtempSync(join(tmpdir(), 'takedock-draft-'));
execFileSync('gh', ['release', 'download', tag, '--dir', downloaded], {
  stdio: 'inherit',
});
verifyAssets(downloaded);
for (const name of readdirSync('release'))
  assert.equal(
    createHash('sha256')
      .update(readFileSync(join(downloaded, name)))
      .digest('hex'),
    createHash('sha256')
      .update(readFileSync(join('release', name)))
      .digest('hex'),
  );
console.log('PASS: every uploaded draft asset matches the verified candidate.');
