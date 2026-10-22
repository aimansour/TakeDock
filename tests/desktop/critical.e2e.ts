import assert from 'node:assert/strict';
import { mkdirSync, unlinkSync, writeFileSync } from 'node:fs';
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
  listedVideos,
  fileResults,
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
  it('selects two videos, hides batch rename and copies verified bytes through Rust IPC', async () => {
    await restartFixture(true);
    await button('Start video').waitForExist();
    await waitForBaseline();
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
    flag('hold-hash');
    try {
      await button('Copy to computer').click();
      await waitFor(() => existsSync(join(fixtureRoot, 'hash-started')));
      await button('Recording').click();
      await button('Start video').waitForExist();
      await button('Start video').click();
      await waitFor(() => commands().length === 1);
      await waitFor(
        async () =>
          (await $('#speech-readiness').getAttribute('data-readiness')) ===
          'ready',
      );
      assert.equal(existsSync(join(fixtureRoot, 'hold-hash')), true);
      await button('Stop').click();
      await waitFor(() => commands().length === 2);
      await button('Start video').waitForExist();
    } finally {
      if (existsSync(join(fixtureRoot, 'hold-hash')))
        unlinkSync(join(fixtureRoot, 'hold-hash'));
    }
    await shortcut('v');
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
    // The per-file 2/2 event can precede the job's terminal event. During
    // verification the progress element is intentionally indeterminate.
    await waitFor(
      async () =>
        (await $('#activity-progress').getAttribute('value')) === '100',
    );
    assert.equal(await $('#activity-progress').getAttribute('value'), '100');
    const history = await $('.file-results-panel details');
    await history.waitForExist();
    assert.equal(await history.getAttribute('open'), null);
    const originalWindow = await browser.getWindowHandle();
    const shared = (await fileResults()).notices.find(
      (notice) => notice.kind === 'copy',
    );
    assert.ok(shared?.expires_at_ms);
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
    assert.match(await $('.activity-panel').getText(), /Ready/);
    assert.equal((await $$('.file-result-notice')).length, 0);
    await $('.file-results-panel summary').click();
    await shortcut('n');
    await waitFor(async () => (await browser.getWindowHandles()).length === 2);
    await browser.switchToWindow(
      (await browser.getWindowHandles()).find(
        (handle) => handle !== originalWindow,
      )!,
    );
    await $('.file-results-panel').waitForExist();
    assert.equal((await $$('.file-result-notice')).length, 0);
    assert.equal(
      await $('.file-results-panel details').getAttribute('open'),
      null,
    );
    await browser.closeWindow();
    await browser.switchToWindow(originalWindow);
    assert.match(await history.getText(), /2\/2/);
    await $('.file-results-panel summary').click();
    assert.equal(deleteCalls().length, 0);
    assert.equal(
      existsSync(join(fixtureRoot, 'videos', 'take-0001.mp4')),
      true,
    );
    assert.equal(
      (await $$('[aria-live], [role="status"], [role="alert"]')).length,
      0,
    );
    // Preserve the completed copy's 100% assertion before a newer cancelled
    // job legitimately becomes the Activity panel's current operation.
    await $(
      '//label[span[normalize-space(.)="Select take-0001.mp4"]]/input',
    ).click();
    unlinkSync(join(fixtureRoot, 'hash-started'));
    flag('hold-hash');
    try {
      await button('Copy to computer').click();
      await waitFor(() => existsSync(join(fixtureRoot, 'hash-started')));
      await button('Recording').click();
      await button('Start video').waitForExist();
      await button('Start video').click();
      await $('input[name="discard-take"]').waitForExist();
      await $('input[name="discard-take"]').click();
      await button('Stop and delete').click();
      await $('[data-discard-phase="deleting"]').waitForExist();
      assert.equal(deleteCalls().length, 0);
      assert.equal(existsSync(join(fixtureRoot, 'hold-hash')), true);
      await button('Start video').click();
      await waitFor(() => commands().length === 5);
      await button('Stop').click();
      await waitFor(() => commands().length === 6);
    } finally {
      if (existsSync(join(fixtureRoot, 'hold-hash')))
        unlinkSync(join(fixtureRoot, 'hold-hash'));
    }
    await waitFor(() =>
      existsSync(join(fixtureRoot, 'destination', 'take-0001.mp4')),
    );
    await waitFor(async () =>
      (await $('.activity-panel').getText()).includes('Cancelled'),
    );
    assert.equal(deleteCalls().length, 0);
    assert.equal(
      existsSync(join(fixtureRoot, 'videos', 'take-0002.mp4')),
      true,
    );
    assert.equal(
      existsSync(join(fixtureRoot, 'videos', 'take-0003.mp4')),
      true,
    );
    assert.equal(
      hash(join(fixtureRoot, 'destination', 'take-0001.mp4')),
      hash(join(fixtureRoot, 'videos', 'take-0001.mp4')),
    );
    await shortcut('v');
    const initialVideo = (await listedVideos()).find(
      (video) => video.name === 'رحلة.mp4',
    );
    assert.ok(initialVideo);
    const sourceBox = await $(
      '//label[span[normalize-space(.)="Select رحلة.mp4"]]/input',
    );
    await sourceBox.waitForExist();
    if (await button('Clear selection').isExisting())
      await button('Clear selection').click();
    await sourceBox.click();
    const originalElementId = sourceBox.elementId;
    if (existsSync(join(fixtureRoot, 'index-started')))
      unlinkSync(join(fixtureRoot, 'index-started'));
    if (existsSync(join(fixtureRoot, 'hash-started')))
      unlinkSync(join(fixtureRoot, 'hash-started'));
    flag('hold-hash');
    flag('hold-index');
    try {
      await button('Rename video').click();
      await $('#new-name').setValue('renamed-رحلة');
      await button('Save name').click();
      await waitFor(() => existsSync(join(fixtureRoot, 'index-started')));
      await browser.waitUntil(
        async () =>
          (await sourceBox.getComputedLabel()) === 'Select renamed-رحلة.mp4',
        {
          timeout: 1000,
          interval: 50,
          timeoutMsg:
            'Confirmed rename did not reach the native label while indexing held the worker',
        },
      );
      const renamedBox = await $(
        '//label[span[normalize-space(.)="Select renamed-رحلة.mp4"]]/input',
      );
      assert.equal(renamedBox.elementId, originalElementId);
      assert.equal(await renamedBox.isSelected(), true);
      await button('Copy to computer').click();
      await browser.execute(
        (element: HTMLElement) => element.focus(),
        renamedBox,
      );
      assert.equal(existsSync(join(fixtureRoot, 'hold-index')), true);
      assert.equal(
        (await $$('[aria-live], [role="status"], [role="alert"]')).length,
        0,
      );
    } finally {
      if (existsSync(join(fixtureRoot, 'hold-index')))
        unlinkSync(join(fixtureRoot, 'hold-index'));
    }
    await waitFor(() => existsSync(join(fixtureRoot, 'hash-started')));
    try {
      await button('Recording').click();
      await button('Start video').click();
      await waitFor(() => commands().length === 7);
      await button('Stop').click();
      await waitFor(() => commands().length === 8);
      await shortcut('v');
      const checkbox = await $(
        '//label[span[normalize-space(.)="Select renamed-رحلة.mp4"]]/input',
      );
      await checkbox.waitForExist();
      if (!(await checkbox.isSelected())) await checkbox.click();
      await browser.execute(
        (element: HTMLElement) => element.focus(),
        checkbox,
      );
    } finally {
      if (existsSync(join(fixtureRoot, 'hold-hash')))
        unlinkSync(join(fixtureRoot, 'hold-hash'));
    }
    await waitFor(() =>
      existsSync(join(fixtureRoot, 'destination', 'renamed-رحلة.mp4')),
    );
    await waitFor(async () =>
      (await fileResults()).history.some(
        (notice) =>
          notice.kind === 'copy' &&
          notice.name === 'renamed-رحلة.mp4' &&
          notice.outcome === 'completed',
      ),
    );
    assert.equal(
      await browser.execute(() => document.activeElement?.getAttribute('type')),
      'checkbox',
    );
    assert.equal(
      hash(join(fixtureRoot, 'destination', 'renamed-رحلة.mp4')),
      hash(join(fixtureRoot, 'videos', 'renamed-رحلة.mp4')),
    );
    const renamed = (await listedVideos()).find(
      (video) => video.name === 'renamed-رحلة.mp4',
    );
    assert.ok(renamed);
    for (const part of ['device', 'inode']) {
      const exact = new RegExp(`"${part}":(\\d+)`);
      assert.equal(
        renamed.identity.match(exact)?.[1],
        initialVideo.identity.match(exact)?.[1],
      );
    }
    await button('Copy to computer').click();
    await waitFor(async () =>
      (await fileResults()).notices.some(
        (notice) =>
          notice.outcome === 'failed' && notice.name === 'renamed-رحلة.mp4',
      ),
    );
    const failure = (await fileResults()).notices.find(
      (notice) =>
        notice.outcome === 'failed' && notice.name === 'renamed-رحلة.mp4',
    )!;
    const failureWindow = await browser.getWindowHandle();
    await shortcut('n');
    await waitFor(async () => (await browser.getWindowHandles()).length === 2);
    const observerWindow = (await browser.getWindowHandles()).find(
      (handle) => handle !== failureWindow,
    )!;
    await browser.switchToWindow(observerWindow);
    const failureCard = () =>
      $(`article[data-operation-id="${failure.key.operation_id}"]`);
    await failureCard().waitForExist();
    const protectedHide = await failureCard().$('button');
    await browser.execute(
      (element: HTMLElement) => element.focus(),
      protectedHide,
    );
    assert.equal(
      await browser.execute(
        (element: HTMLElement) => document.activeElement === element,
        protectedHide,
      ),
      true,
      'The observer window must establish focus before later results arrive',
    );
    await browser.switchToWindow(failureWindow);
    await button('Rename video').click();
    await $('#new-name').setValue('final-رحلة');
    await button('Save name').click();
    await waitFor(async () =>
      (await fileResults()).notices.some(
        (notice) =>
          notice.kind === 'rename' &&
          notice.outcome === 'completed' &&
          notice.results.some(
            (item) => item.destination_name === 'final-رحلة.mp4',
          ),
      ),
    );
    await waitFor(
      async () =>
        !(await fileResults()).notices.some(
          (notice) => notice.expires_at_ms != null,
        ),
    );
    assert.equal(await failureCard().isExisting(), true);
    await browser.switchToWindow(observerWindow);
    assert.equal(
      await browser.execute(
        (element: HTMLElement) => document.activeElement === element,
        protectedHide,
      ),
      true,
      'New results and passive expiry must preserve the observer focus',
    );
    await browser.switchToWindow(failureWindow);
    await failureCard().$('button').click();
    await waitFor(
      async () =>
        !(await fileResults()).notices.some(
          (notice) => notice.key.operation_id === failure.key.operation_id,
        ),
    );
    await browser.switchToWindow(observerWindow);
    assert.equal(
      (await failureCard().$('button')).elementId,
      protectedHide.elementId,
    );
    await browser.execute(() =>
      document
        .querySelector<HTMLElement>('.file-results-panel summary')
        ?.focus(),
    );
    await waitFor(async () => !(await failureCard().isExisting()));
    await browser.closeWindow();
    await browser.switchToWindow(failureWindow);

    const collisionDirectory = join(fixtureRoot, 'collision-destination');
    mkdirSync(collisionDirectory);
    await shortcut('s');
    await $('#destination').waitForExist();
    await $('#destination').setValue(collisionDirectory);
    await button('Save settings').click();
    await shortcut('v');
    await $('tbody').waitForExist();
    for (const checked of await $$('tbody input[type="checkbox"]:checked'))
      await checked.click();
    const collisionName = 'final-رحلة.mp4';
    const collisionSelection = await $(
      `//label[span[normalize-space(.)="Select ${collisionName}"]]/input`,
    );
    await collisionSelection.waitForExist();
    await collisionSelection.click();
    assert.equal((await $$('tbody input[type="checkbox"]:checked')).length, 1);
    const collisionSource = (await listedVideos()).find(
      (video) => video.name === collisionName,
    )!;
    const sourceHash = hash(join(fixtureRoot, 'videos', collisionName));
    const collisionTarget = join(collisionDirectory, collisionName);
    if (existsSync(join(fixtureRoot, 'hash-started')))
      unlinkSync(join(fixtureRoot, 'hash-started'));
    flag('hold-hash');
    try {
      await button('Copy to computer').click();
      await waitFor(() => existsSync(join(fixtureRoot, 'hash-started')));
      writeFileSync(collisionTarget, 'keep the destination sentinel');
    } finally {
      if (existsSync(join(fixtureRoot, 'hold-hash')))
        unlinkSync(join(fixtureRoot, 'hold-hash'));
    }
    await waitFor(async () =>
      (await fileResults()).notices.some(
        (notice) =>
          notice.kind === 'copy' &&
          notice.name === collisionName &&
          notice.outcome === 'failed',
      ),
    );
    const collision = (await fileResults()).notices.find(
      (notice) =>
        notice.kind === 'copy' &&
        notice.name === collisionName &&
        notice.outcome === 'failed',
    )!;
    assert.equal(collision.expires_at_ms, null);
    assert.equal(
      readFileSync(collisionTarget, 'utf8'),
      'keep the destination sentinel',
    );
    assert.equal(hash(join(fixtureRoot, 'videos', collisionName)), sourceHash);
    assert.equal(
      (await listedVideos()).find((video) => video.name === collisionName)
        ?.identity,
      collisionSource.identity,
    );
    const collisionCard = await $(
      `article[data-operation-id="${collision.key.operation_id}"]`,
    );
    assert.equal((await collisionCard.getText()).includes(collisionName), true);
    await collisionCard.$('button').click();
    await waitFor(
      async () =>
        !(await fileResults()).notices.some(
          (notice) => notice.key.operation_id === collision.key.operation_id,
        ),
    );
    await $('.file-results-panel summary').click();
    await waitFor(async () => !(await collisionCard.isExisting()));
  });
  it('persists Arabic, destination and optional success sound across application restart', async () => {
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
    assert.equal(saved.destination, join(fixtureRoot, 'destination'));
    await browser.reloadSession();
    await $('h1').waitForExist();
    await shortcut('s');
    await $('#language').waitForExist();
    assert.equal(await $('#language').getValue(), 'ar');
    assert.equal(await $('#success-sound').isSelected(), true);
    assert.equal(await $('#ready-sound').isSelected(), false);
    assert.equal(await $('#ready-delay').getValue(), '1250');
    assert.equal(await $('#destination').getValue(), saved.destination);
    assert.equal(await $('html').getAttribute('dir'), 'rtl');
    assert.equal(await button('تثبيت التحديث').isExisting(), false);
    assert.equal(
      await $('.file-results-panel summary').getText(),
      'سجل العمليات',
    );
  });
});
