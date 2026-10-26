import assert from 'node:assert/strict';
import { unlinkSync, writeFileSync } from 'node:fs';
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
  waitForBaseline,
  deleteCalls,
  fileResults,
} from './fixtures';

function xpathLiteral(value: string): string {
  return value.includes("'")
    ? `concat('${value.split("'").join(`',"'",'`)}')`
    : `'${value}'`;
}
function sourceRow(name: string) {
  return $(`//tbody/tr[td[2]/bdi[text()=${xpathLiteral(name)}]]`);
}
describe('TakeDock release desktop acceptance', () => {
  it('launches offline with named keyboard controls and silent success defaults', async () => {
    await $('h1').waitForExist();
    await waitFor(
      async () => (await $('h1').getText()) === 'TakeDock — Window 1',
    );
    assert.equal(await button('Start video').isExisting(), false);
    await shortcut('s');
    await $('#language').waitForExist();
    assert.equal(await $('#success-sound').isSelected(), false);
    assert.equal(await $('#ready-sound').isSelected(), true);
    assert.equal(await $('#ready-delay').getValue(), '800');
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
    assert.equal(
      await $('.file-results-panel details').getAttribute('open'),
      null,
    );
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
    await $('#ready-sound').click();
    await $('#ready-delay').setValue('1250');
    await button('Save settings').click();
    await browser.switchToWindow(first);
    await waitFor(async () => (await $('html').getAttribute('dir')) === 'rtl');
    assert.equal(await $('#language').getValue(), 'ar');
    assert.equal(await $('#ready-sound').isSelected(), false);
    assert.equal(await $('#ready-delay').getValue(), '1250');
    assert.equal(await $('#success-sound').isSelected(), false);
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
    await waitFor(
      async () =>
        (await $('#speech-readiness').getAttribute('data-readiness')) ===
        'preparing',
    );
    // Hold longer than the default guard: clicking Start cannot arm the timer.
    await browser.pause(1000);
    assert.equal(await $('#speech-readiness').getText(), 'Preparing to speak');
    assert.equal(await button('Start video').isExisting(), false);
    assert.equal(
      await browser.execute(() => document.activeElement?.textContent?.trim()),
      'Stop',
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
    await waitFor(async () => !(await $('#speech-readiness').isExisting()));
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
    assert.equal(await $('#speech-readiness').isExisting(), false);
    unlinkSync(join(fixtureRoot, 'hold-observation'));
    await button('Start video').waitForExist();
    assert.equal(await button('Stop').isExisting(), false);
    // Use a fresh fixture so unobserved intermediate toggles cannot be
    // mistaken for proof of this independently confirmed Start.
    await restartFixture(true);
    await button('Start video').waitForExist();
    await waitForBaseline();
    await button('Start video').click();
    await waitFor(
      async () =>
        (await $('#speech-readiness').getAttribute('data-readiness')) ===
        'ready',
    );
    assert.equal(await $('#speech-readiness').getText(), 'Ready to speak');
    assert.equal(
      await browser.execute(() => document.activeElement?.textContent?.trim()),
      'Stop',
    );
    const choice = await $('input[name="discard-take"]');
    await choice.waitForExist();
    assert.equal(await choice.isSelected(), false);
    const primary = await button('Stop');
    await choice.click();
    await browser.keys('Space');
    assert.equal(await choice.isSelected(), false);
    await browser.keys('Space');
    assert.equal(await choice.isSelected(), true);
    assert.equal(
      (await button('Stop and delete')).elementId,
      primary.elementId,
    );
    const first = await browser.getWindowHandle();
    await shortcut('n');
    await waitFor(async () => (await browser.getWindowHandles()).length === 2);
    const second = (await browser.getWindowHandles()).find(
      (handle) => handle !== first,
    )!;
    await browser.switchToWindow(second);
    await waitFor(
      async () =>
        (await $('#speech-readiness').getAttribute('data-readiness')) ===
        'ready',
    );
    assert.equal(commands().length, 1);
    assert.equal(await $('input[name="discard-take"]').isSelected(), false);
    flag('hold-finalization');
    await browser.switchToWindow(first);
    await button('Stop and delete').click();
    await $('[data-discard-phase="finalizing"]').waitForExist();
    await browser.switchToWindow(second);
    await $('[data-discard-phase="finalizing"]').waitForExist();
    assert.equal(await $('input[name="discard-take"]').isExisting(), false);
    unlinkSync(join(fixtureRoot, 'hold-finalization'));
    await $('[data-discard-phase="completed"]').waitForExist();
    assert.equal(
      existsSync(join(fixtureRoot, 'videos', 'take-0001.mp4')),
      false,
    );
    assert.equal(deleteCalls().length, 1);
    assert.equal(deleteCalls()[0].name, 'take-0001.mp4');
    assert.ok(deleteCalls()[0].identity);
    await browser.closeWindow();
    await browser.switchToWindow(first);
    await button('Start video').waitForExist();
    await waitFor(async () => !(await $('#speech-readiness').isExisting()));

    await restartFixture(true, ['hold-watch-baseline']);
    await button('Start video').waitForExist();
    await button('Start video').click();
    await button('Stop').waitForExist();
    assert.equal(await $('input[name="discard-take"]').isExisting(), false);
    await button('Stop').click();
    await button('Start video').waitForExist();
    assert.equal(deleteCalls().length, 0);
    assert.equal(
      existsSync(join(fixtureRoot, 'videos', 'take-0001.mp4')),
      true,
    );

    await restartFixture(true, ['hold-delete-prepared']);
    await button('Start video').waitForExist();
    await waitForBaseline();
    await button('Start video').click();
    await $('input[name="discard-take"]').waitForExist();
    await $('input[name="discard-take"]').click();
    await button('Stop and delete').click();
    await waitFor(() => existsSync(join(fixtureRoot, 'delete-prepared')));
    const retained = hash(join(fixtureRoot, 'videos', 'take-0001.mp4'));
    await button('Start video').click();
    await waitFor(() => commands().length === 3);
    unlinkSync(join(fixtureRoot, 'hold-delete-prepared'));
    await waitFor(async () =>
      (await $('main').getText()).includes('Pending deletion was cancelled.'),
    );
    assert.equal(hash(join(fixtureRoot, 'videos', 'take-0001.mp4')), retained);
    assert.equal(
      readFileSync(join(fixtureRoot, 'videos', 'take-0002.mp4'), 'utf8'),
      'fixture-recording',
    );
    await button('Stop').click();
    await button('Start video').waitForExist();

    await restartFixture(true, ['drop-delete-result']);
    await button('Start video').waitForExist();
    await waitForBaseline();
    await button('Start video').click();
    await $('input[name="discard-take"]').waitForExist();
    await $('input[name="discard-take"]').click();
    await button('Stop and delete').click();
    await $('[data-discard-phase="outcome_unknown"]').waitForExist();
    assert.match(await $('main').getText(), /result could not be confirmed/);
    assert.equal(deleteCalls().length, 1);
    assert.equal(
      existsSync(join(fixtureRoot, 'videos', 'take-0001.mp4')),
      false,
    );
  });
  it('hands off marked sources once, preserves shared results and never manages the handoff collection', async () => {
    await restartFixture(true);
    await button('Start video').waitForExist();
    await waitForBaseline();
    const originals = ["a'b.mp4", 'رحلة.mp4'];
    const expected = new Map(
      originals.map((name) => [name, hash(join(fixtureRoot, 'videos', name))]),
    );
    await shortcut('v');
    await waitFor(
      async () => (await $$('tbody input[type="checkbox"]')).length === 2,
    );
    assert.equal(await button('Copy to computer').isExisting(), false);
    assert.equal(await button('Move to computer').isExisting(), false);
    const arabic = await sourceRow('رحلة.mp4').$('input');
    await arabic.click();
    const originalId = arabic.elementId;
    await button('Rename video').click();
    await $('#new-name').setValue('renamed-رحلة');
    await button('Save name').click();
    const renamed = await sourceRow('renamed-رحلة.mp4').$('input');
    await renamed.waitForExist();
    await renamed.waitForEnabled();
    assert.equal(renamed.elementId, originalId);
    assert.equal(await renamed.getComputedLabel(), 'Select renamed-رحلة.mp4');
    assert.equal(
      hash(join(fixtureRoot, 'videos', 'renamed-رحلة.mp4')),
      expected.get('رحلة.mp4'),
    );
    await button('Select all completed videos').click();
    assert.equal(await button('Rename video').isExisting(), false);
    flag('hold-handoff-before-permit');
    try {
      await button('Mark selected').click();
      await waitFor(() => existsSync(join(fixtureRoot, 'handoff-started')));
      assert.equal(await sourceRow("a'b.mp4").$('input').isEnabled(), false);
      assert.equal(await $('#activity-progress').getAttribute('value'), null);
      await button('Recording').click();
      await button('Start video').waitForExist();
      await button('Start video').click();
      await waitFor(() => commands().length === 1);
      await waitFor(
        async () =>
          (await $('#speech-readiness').getAttribute('data-readiness')) ===
          'ready',
      );
      assert.equal(
        await browser.execute(() =>
          document.activeElement?.textContent?.trim(),
        ),
        'Stop',
      );
      assert.equal(
        existsSync(join(fixtureRoot, 'hold-handoff-before-permit')),
        true,
      );
      await button('Stop').click();
      await waitFor(() => commands().length === 2);
    } finally {
      unlinkSync(join(fixtureRoot, 'hold-handoff-before-permit'));
    }
    await shortcut('v');
    for (const [source, target] of [
      ["a'b.mp4", "a'b.mp4"],
      ['رحلة.mp4', 'renamed-رحلة.mp4'],
    ]) {
      await waitFor(() => existsSync(join(fixtureRoot, 'handoff', target)));
      assert.equal(
        hash(join(fixtureRoot, 'handoff', target)),
        expected.get(source),
      );
      assert.equal(existsSync(join(fixtureRoot, 'videos', target)), false);
      await waitFor(async () => !(await sourceRow(target).isExisting()));
    }
    await waitFor(
      async () =>
        (await $('#activity-progress').getAttribute('value')) === '100',
    );
    const shared = (await fileResults()).notices.find(
      (notice) => notice.kind === 'mark' && notice.count === 2,
    );
    assert.ok(shared?.expires_at_ms);
    assert.equal(
      await $('.file-results-panel details').getAttribute('open'),
      null,
    );
    const originalWindow = await browser.getWindowHandle();
    await shortcut('n');
    await waitFor(async () => (await browser.getWindowHandles()).length === 2);
    const otherWindow = (await browser.getWindowHandles()).find(
      (handle) => handle !== originalWindow,
    )!;
    await browser.switchToWindow(otherWindow);
    await $('.file-results-panel').waitForExist();
    const mirrored = (await fileResults()).notices.find(
      (notice) => notice.key.operation_id === shared.key.operation_id,
    );
    assert.equal(mirrored?.expires_at_ms, shared.expires_at_ms);
    await waitFor(async () => (await $$('.file-result-notice')).length === 0);
    await browser.closeWindow();
    await browser.switchToWindow(originalWindow);
    await waitFor(
      async () => (await $('#activity-progress').getAttribute('value')) === '0',
    );
    await $('.file-results-panel summary').click();
    assert.match(await $('.file-results-panel details').getText(), /2\/2/);
    await $('.file-results-panel summary').click();
    assert.equal(
      (await $$('[aria-live], [role="status"], [role="alert"]')).length,
      0,
    );
    // The preceding batch deliberately changes unrelated source history.
    // Start a fresh managed take so retirement is proved while eligible.
    await button('Recording').click();
    await button('Start video').waitForExist();
    await button('Start video').click();
    await $('input[name="discard-take"]').waitForExist();
    await waitFor(
      async () =>
        (await $('#speech-readiness').getAttribute('data-readiness')) ===
        'ready',
    );
    await button('Stop').click();
    await button('Start video').waitForExist();
    const eligibleBeforeHandoff = await browser.execute(async () => {
      const api = (
        window as unknown as {
          __TAURI_INTERNALS__: {
            invoke: (
              name: string,
            ) => Promise<{ session: { discard: { eligible: boolean } } }>;
          };
        }
      ).__TAURI_INTERNALS__;
      return (await api.invoke('bootstrap')).session.discard.eligible;
    });
    assert.equal(eligibleBeforeHandoff, true);
    await shortcut('v');
    await sourceRow('take-0002.mp4').$('button[data-mark]').waitForExist();
    await sourceRow('take-0002.mp4').$('button[data-mark]').click();
    await waitFor(() =>
      existsSync(join(fixtureRoot, 'handoff', 'take-0002.mp4')),
    );
    await button('Recording').click();
    await waitFor(async () => {
      const result = await browser.execute(async () => {
        const api = (
          window as unknown as {
            __TAURI_INTERNALS__: {
              invoke: (name: string) => Promise<{
                session: { discard: { eligible: boolean; reason: string } };
              }>;
            };
          }
        ).__TAURI_INTERNALS__;
        return (await api.invoke('bootstrap')).session.discard;
      });
      return !result.eligible && result.reason === 'discard_file_handed_off';
    });
    assert.equal(deleteCalls().length, 0);
    await shortcut('v');
    const bytes = readFileSync(join(fixtureRoot, 'handoff', "a'b.mp4"));
    writeFileSync(join(fixtureRoot, 'videos', 'delete.mp4'), bytes);
    await button('Refresh videos').click();
    await sourceRow('delete.mp4').$('input').waitForExist();
    await sourceRow('delete.mp4').$('input').click();
    await button('Delete from phone').click();
    await button('Confirm deletion').click();
    await waitFor(() => !existsSync(join(fixtureRoot, 'videos', 'delete.mp4')));
    writeFileSync(join(fixtureRoot, 'videos', 'collision.mp4'), bytes);
    writeFileSync(
      join(fixtureRoot, 'handoff', 'collision.mp4'),
      'previous destination',
    );
    await button('Refresh videos').click();
    await sourceRow('collision.mp4').$('button[data-mark]').waitForExist();
    await sourceRow('collision.mp4').$('button[data-mark]').click();
    await waitFor(async () =>
      (await fileResults()).notices.some((notice) =>
        notice.results.some(
          (result) =>
            result.name === 'collision.mp4' && result.status === 'retained',
        ),
      ),
    );
    assert.equal(
      hash(join(fixtureRoot, 'videos', 'collision.mp4')),
      expected.get("a'b.mp4"),
    );
    assert.equal(
      readFileSync(join(fixtureRoot, 'handoff', 'collision.mp4'), 'utf8'),
      'previous destination',
    );
    assert.equal(await sourceRow('collision.mp4').isExisting(), true);
    writeFileSync(join(fixtureRoot, 'videos', 'unknown.mp4'), bytes);
    flag('drop-handoff-result');
    await button('Refresh videos').click();
    await sourceRow('unknown.mp4').$('button[data-mark]').waitForExist();
    await sourceRow('unknown.mp4').$('button[data-mark]').click();
    await waitFor(async () =>
      (await fileResults()).notices.some(
        (notice) => notice.outcome === 'outcome_unknown',
      ),
    );
    unlinkSync(join(fixtureRoot, 'drop-handoff-result'));
    assert.equal(existsSync(join(fixtureRoot, 'videos', 'unknown.mp4')), false);
    assert.equal(
      hash(join(fixtureRoot, 'handoff', 'unknown.mp4')),
      expected.get("a'b.mp4"),
    );
    const calls = readFileSync(join(fixtureRoot, 'helper-calls.jsonl'), 'utf8')
      .trim()
      .split('\n')
      .map((line) => JSON.parse(line) as string[]);
    assert.equal(
      calls.filter(
        (args) => args[0] === 'handoff-guarded' && args[1] === 'unknown.mp4',
      ).length,
      1,
    );
    assert.equal(
      calls.some((args) => ['read', 'hash'].includes(args[0])),
      false,
    );
    writeFileSync(join(fixtureRoot, 'videos', 'warning.mp4'), bytes);
    flag('fail-handoff-index');
    await button('Refresh videos').click();
    await sourceRow('warning.mp4').$('button[data-mark]').waitForExist();
    await sourceRow('warning.mp4').$('button[data-mark]').click();
    await waitFor(async () =>
      (await fileResults()).notices.some(
        (notice) =>
          notice.name === 'warning.mp4' &&
          notice.warnings.includes('media_index_pending'),
      ),
    );
    unlinkSync(join(fixtureRoot, 'fail-handoff-index'));
    const persistent = (await fileResults()).notices.filter((notice) =>
      ['retained', 'outcome_unknown', 'warning'].includes(notice.outcome),
    );
    assert.ok(persistent.length >= 3);
    assert.ok(persistent.every((notice) => notice.expires_at_ms === null));
    assert.equal(
      readFileSync(join(fixtureRoot, 'handoff', 'unrelated.mp4'), 'utf8'),
      'hands off',
    );
    await browser.reloadSession();
    await $('h1').waitForExist();
    await button('Start video').waitForExist();
    await shortcut('v');
    await button('Refresh videos').click();
    await sourceRow('collision.mp4').waitForExist();
    assert.equal(await sourceRow('unrelated.mp4').isExisting(), false);
    assert.equal(
      readFileSync(join(fixtureRoot, 'handoff', 'unrelated.mp4'), 'utf8'),
      'hands off',
    );
    assert.equal(
      hash(join(fixtureRoot, 'handoff', 'renamed-رحلة.mp4')),
      expected.get('رحلة.mp4'),
    );
  });
  it('persists Arabic and unrelated preferences while migrating obsolete destination across application restart', async () => {
    await restartFixture(false);
    await shortcut('s');
    await $('#language').waitForExist();
    await $('#language').selectByAttribute('value', 'ar');
    await $('#success-sound').click();
    await $('#ready-sound').click();
    await $('#ready-delay').setValue('1250');
    await button('Save settings').click();
    await waitFor(
      () => JSON.parse(readFileSync(settingsPath, 'utf8')).language === 'ar',
    );
    const saved = JSON.parse(readFileSync(settingsPath, 'utf8'));
    assert.equal(saved.success_sound, true);
    assert.equal(saved.ready_sound, false);
    assert.equal(saved.ready_delay_ms, 1250);
    assert.equal(Object.hasOwn(saved, 'destination'), false);
    await browser.reloadSession();
    await $('h1').waitForExist();
    await shortcut('s');
    await $('#language').waitForExist();
    assert.equal(await $('#language').getValue(), 'ar');
    assert.equal(await $('#success-sound').isSelected(), true);
    assert.equal(await $('#ready-sound').isSelected(), false);
    assert.equal(await $('#ready-delay').getValue(), '1250');
    assert.equal(await $('#destination').isExisting(), false);
    assert.equal(await $('html').getAttribute('dir'), 'rtl');
    assert.equal(await button('تثبيت التحديث').isExisting(), false);
    assert.equal(
      await $('.file-results-panel summary').getText(),
      'سجل العمليات',
    );
  });
});
