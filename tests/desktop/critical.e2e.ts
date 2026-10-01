import assert from 'node:assert/strict';
import { unlinkSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { browser, $, $$ } from '@wdio/globals';
import {
  restartFixture,
  button,
  shortcut,
  waitFor,
  fixtureRoot,
  commands,
  flag,
  inject,
  hash,
  join,
  existsSync,
  readFileSync,
  settingsPath,
} from './fixtures';

describe('TakeDock release desktop acceptance', () => {
  it('launches offline with named keyboard controls and silent success defaults', async () => {
    await $('h1').waitForExist();
    await waitFor(
      async () => (await $('h1').getText()) === 'TakeDock — Window 1',
    );
    assert.equal(await button('Start video').isExisting(), false);
    await shortcut('s');
    await $('#language').waitForExist();
    assert.equal(await $('input[type="checkbox"]').isSelected(), false);
    await button('Save settings').click();
    await browser.keys('Tab');
    assert.equal(
      await browser.execute(() => document.activeElement?.textContent?.trim()),
      'Reconnect',
    );
    assert.equal(
      (await $$('[aria-live], [role="status"], [role="alert"]')).length,
      0,
    );
    assert.equal(await button('Install update').isExisting(), false);
    const first = await browser.getWindowHandle();
    await $('#window-name').setValue('Interview');
    await button('Save window name').click();
    await waitFor(
      async () => (await browser.getTitle()) === 'TakeDock — Interview',
    );
    await shortcut('n');
    await waitFor(async () => (await browser.getWindowHandles()).length === 2);
    const second = (await browser.getWindowHandles()).find(
      (handle) => handle !== first,
    )!;
    await browser.switchToWindow(second);
    await waitFor(async () => (await $('h1').getText()).includes('Window 2'));
    await shortcut('s');
    await $('#language').waitForExist();
    await $('#language').selectByAttribute('value', 'ar');
    await button('Save settings').click();
    await browser.switchToWindow(first);
    await waitFor(async () => (await $('html').getAttribute('dir')) === 'rtl');
    assert.equal(await $('#language').getValue(), 'ar');
    assert.equal(await $('#window-name').getValue(), 'Interview');
    await $('#language').selectByAttribute('value', 'en');
    await button('حفظ الإعدادات').click();
    await waitFor(async () => (await $('html').getAttribute('dir')) === 'ltr');
    const launched = spawn(process.env.TAKEDOCK_APP_EXE!, [], {
      windowsHide: true,
      stdio: 'ignore',
      signal: AbortSignal.timeout(15000),
    });
    const exit = await new Promise<number | null>((resolve, reject) => {
      launched.once('error', reject);
      launched.once('exit', resolve);
    });
    assert.equal(exit, 0);
    assert.equal((await browser.getWindowHandles()).length, 2);
    await browser.switchToWindow(second);
    await browser.closeWindow();
    await browser.switchToWindow(first);
  });
  it('dispatches all four actions while verification is held and retains logical focus', async () => {
    await restartFixture(true);
    await button('Start video').waitForExist();
    flag('hold-observation');
    await button('Start video').click();
    await waitFor(() => commands().length === 1);
    assert.equal(await button('Start video').isExisting(), false);
    assert.equal(
      await browser.execute(() => document.activeElement?.textContent?.trim()),
      'Stop video',
    );
    await button('Pause video').click();
    await waitFor(() => commands().length === 2);
    assert.equal(
      await browser.execute(() => document.activeElement?.textContent?.trim()),
      'Resume video',
    );
    await button('Resume video').click();
    await waitFor(() => commands().length === 3);
    await shortcut('r');
    await waitFor(() => commands().length === 4);
    assert.deepEqual(
      commands().map((command) => command.key),
      ['up', 'down', 'down', 'up'],
    );
    assert.equal(existsSync(join(fixtureRoot, 'hold-observation')), true);
    assert.equal(
      await browser.execute(() => document.activeElement?.textContent?.trim()),
      'Start video',
    );
    inject({
      type: 'state',
      seq: 0,
      event_time: 0,
      state: 'paused',
      foreground: true,
      video_mode: true,
    });
    await waitFor(() => existsSync(join(fixtureRoot, 'injected-event')));
    assert.equal(await button('Resume video').isExisting(), false);
    unlinkSync(join(fixtureRoot, 'hold-observation'));
    await button('Start video').waitForExist();
    assert.equal(await button('Stop video').isExisting(), false);
  });
  it('selects two videos, hides batch rename and copies verified bytes through Rust IPC', async () => {
    await restartFixture(true);
    await button('Start video').waitForExist();
    await shortcut('v');
    await waitFor(
      async () => (await $$('tbody input[type="checkbox"]')).length === 2,
    );
    const boxes = await $$('tbody input[type="checkbox"]');
    assert.match(await $('tbody').getText(), /100 milliseconds/);
    await boxes[0].click();
    assert.equal(await button('Rename video').isExisting(), true);
    await boxes[1].click();
    assert.equal(await button('Rename video').isExisting(), false);
    await button('Copy to computer').click();
    await waitFor(() =>
      ["a'b.mp4", 'رحلة.mp4'].every((name) =>
        existsSync(join(fixtureRoot, 'destination', name)),
      ),
    );
    for (const name of ["a'b.mp4", 'رحلة.mp4'])
      assert.equal(
        hash(join(fixtureRoot, 'destination', name)),
        hash(join(fixtureRoot, 'videos', name)),
      );
    await waitFor(async () => (await $('main').getText()).includes('2/2'));
    assert.equal(await $('#activity-progress').getAttribute('value'), '100');
    assert.equal(
      (await $$('[aria-live], [role="status"], [role="alert"]')).length,
      0,
    );
  });
  it('persists Arabic, destination and optional success sound across application restart', async () => {
    await restartFixture(false);
    await shortcut('s');
    await $('#language').waitForExist();
    await $('#language').selectByAttribute('value', 'ar');
    await $('input[type="checkbox"]').click();
    await button('Save settings').click();
    await waitFor(
      () => JSON.parse(readFileSync(settingsPath, 'utf8')).language === 'ar',
    );
    const saved = JSON.parse(readFileSync(settingsPath, 'utf8'));
    assert.equal(saved.success_sound, true);
    assert.equal(saved.destination, join(fixtureRoot, 'destination'));
    await browser.reloadSession();
    await $('h1').waitForExist();
    await shortcut('s');
    await $('#language').waitForExist();
    assert.equal(await $('#language').getValue(), 'ar');
    assert.equal(await $('input[type="checkbox"]').isSelected(), true);
    assert.equal(await $('#destination').getValue(), saved.destination);
    assert.equal(await $('html').getAttribute('dir'), 'rtl');
    assert.equal(await button('تثبيت التحديث').isExisting(), false);
  });
});
