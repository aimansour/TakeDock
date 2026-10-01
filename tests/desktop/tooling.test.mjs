import { test } from 'node:test';
import assert from 'node:assert/strict';
import extractZip from 'extract-zip';
import { createRequire } from 'node:module';
test('unused browser manager cannot extract or auto-install a driver', async () => {
  await assert.rejects(
    () => extractZip('missing.zip', { dir: process.cwd() }),
    /Automatic browser\/driver downloads are disabled/,
  );
  const manager = createRequire(import.meta.resolve('@puppeteer/browsers'));
  await assert.rejects(
    () => manager('extract-zip')('missing.zip', { dir: process.cwd() }),
    /Automatic browser\/driver downloads are disabled/,
  );
});
