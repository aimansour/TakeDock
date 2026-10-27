import { render, screen, fireEvent } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import VideosPanel from './VideosPanel.svelte';
import type { Video } from '../lib/types';
import { readFileSync } from 'node:fs';
import { fileURLToPath, URL as NodeURL } from 'node:url';

it('keeps native benchmark refresh and loading locators aligned with the rendered panel', () => {
  const source = readFileSync(
    fileURLToPath(
      new NodeURL('../../scripts/transfer-benchmark/Ui.psm1', import.meta.url),
    ),
    'utf8',
  );
  const refresh = source.match(
    /\$refresh=Get-TransferElement[^\r\n]*-Value '([^']+)'/,
  );
  const loading = source.match(/value='([^']+Loading…[^']+)'/);
  expect(refresh).not.toBeNull();
  expect(loading).not.toBeNull();
  const { container } = render(VideosPanel, {
    videos: [],
    language: 'en',
    loading: true,
    onJob: vi.fn(),
    onRefresh: vi.fn(),
  });
  const button = screen.getByRole('button', { name: 'Refresh videos' });
  const message = screen.getByText('Loading…');
  const other = document.createElement('section');
  other.innerHTML =
    '<h2>Other</h2><button>Refresh videos</button><p>Loading…</p>';
  container.append(other);
  for (const [xpath, expected] of [
    [refresh![1], button],
    [loading![1], message],
  ] as const) {
    const result = document.evaluate(
      xpath,
      document,
      null,
      XPathResult.ORDERED_NODE_SNAPSHOT_TYPE,
      null,
    );
    expect(result.snapshotLength).toBe(1);
    expect(result.snapshotItem(0)).toBe(expected);
  }
});

const videos: Video[] = [
  {
    name: 'one.mp4',
    size: 10,
    modified_ms: 0,
    ready: true,
    identity: '{"device":1,"inode":1}',
  },
  {
    name: 'two.mp4',
    size: 10,
    modified_ms: 0,
    ready: true,
    identity: '{"device":1,"inode":2}',
  },
];
function nativeDisableBlur(control: HTMLElement) {
  // jsdom ignores blur() while disabled. Emulate the Chromium transition,
  // leaving the control disabled and checking that focus actually reached body.
  const disabled = control.hasAttribute('disabled');
  control.removeAttribute('disabled');
  control.blur();
  if (disabled) control.setAttribute('disabled', '');
  expect(document.body).toHaveFocus();
}
it('discards a delete confirmation and selection across a new connection generation', async () => {
  const user = userEvent.setup();
  const onJob = vi.fn();
  const { rerender } = render(VideosPanel, {
    videos,
    generation: 3,
    onJob,
    onRefresh: vi.fn(),
  });
  await user.click(screen.getByRole('checkbox', { name: 'Select one.mp4' }));
  await user.click(screen.getByRole('button', { name: 'Delete from phone' }));
  const staleConfirmation = screen.getByRole('button', {
    name: 'Confirm deletion',
  });
  await rerender({ generation: 4, videos: [...videos] });
  expect(screen.queryByRole('button', { name: 'Confirm deletion' })).toBeNull();
  expect(
    screen.getByRole('checkbox', { name: 'Select one.mp4' }),
  ).not.toBeChecked();
  staleConfirmation.click();
  expect(onJob).not.toHaveBeenCalled();
});
it('allows batch operations and renders rename only for a single selection', async () => {
  const user = userEvent.setup();
  const onJob = vi.fn();
  const { rerender, container } = render(VideosPanel, {
    videos,
    onJob,
    onRefresh: vi.fn(),
  });
  await user.click(screen.getByRole('checkbox', { name: 'Select one.mp4' }));
  expect(
    screen.getByRole('button', { name: 'Rename video' }),
  ).toBeInTheDocument();
  await user.click(screen.getByRole('checkbox', { name: 'Select two.mp4' }));
  expect(screen.queryByRole('button', { name: 'Rename video' })).toBeNull();
  expect(screen.queryByRole('button', { name: 'Copy to computer' })).toBeNull();
  expect(screen.queryByRole('button', { name: 'Move to computer' })).toBeNull();
  expect(onJob).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button', { name: 'Mark selected' }));
  expect(onJob).toHaveBeenCalledWith('mark', videos, undefined, 0);
  await user.click(screen.getByRole('button', { name: 'Delete from phone' }));
  expect(onJob.mock.calls.length).toBe(1);
  await user.click(screen.getByRole('button', { name: 'Confirm deletion' }));
  expect(onJob).toHaveBeenLastCalledWith('delete', videos, undefined, 0);
  await rerender({
    videos: [{ ...videos[0], name: 'renamed.mp4' }, videos[1]],
  });
  expect(
    screen.getByRole('checkbox', { name: 'Select renamed.mp4' }),
  ).toBeChecked();
  expect(
    container.querySelector('[aria-live], [role="alert"], [role="status"]'),
  ).toBeNull();
});
it('keeps unfinished videos discoverable without an actionable checkbox', () => {
  render(VideosPanel, {
    videos: [{ ...videos[0], ready: false }],
    onJob: vi.fn(),
    onRefresh: vi.fn(),
  });
  expect(screen.getByRole('cell', { name: 'one.mp4' })).toBeInTheDocument();
  expect(screen.queryByRole('checkbox')).toBeNull();
});

it('marks one row directly and protects pending and unfinished rows', async () => {
  const user = userEvent.setup();
  const onJob = vi.fn();
  const { rerender } = render(VideosPanel, {
    videos,
    generation: 3,
    onJob,
    onRefresh: vi.fn(),
  });
  await user.click(screen.getByRole('button', { name: 'Mark one.mp4' }));
  expect(onJob).toHaveBeenCalledWith('mark', [videos[0]], undefined, 3);
  await rerender({
    pendingKeys: ['1:1'],
    videos: [videos[0], { ...videos[1], ready: false }],
  });
  expect(screen.getByRole('button', { name: 'Mark one.mp4' })).toBeDisabled();
  expect(screen.getByRole('button', { name: 'Mark two.mp4' })).toBeDisabled();
  expect(screen.getByRole('cell', { name: 'one.mp4' })).toBeInTheDocument();
  expect(
    screen.getByRole('checkbox', { name: 'Select one.mp4' }),
  ).toBeDisabled();
});
it.each(['next', 'previous', 'empty'] as const)(
  'retains logical focus on %s after confirmed row removal',
  async (where) => {
    const { rerender } = render(VideosPanel, {
      videos,
      onJob: vi.fn(),
      onRefresh: vi.fn(),
    });
    const name = where === 'previous' ? 'two.mp4' : 'one.mp4';
    const mark = screen.getByRole('button', { name: `Mark ${name}` });
    mark.focus();
    await rerender({ pendingKeys: [where === 'previous' ? '1:2' : '1:1'] });
    expect(mark).toBeDisabled();
    // Chromium blurs a focused control when it becomes disabled; jsdom does not.
    nativeDisableBlur(mark);
    const remaining =
      where === 'empty' ? [] : where === 'next' ? [videos[1]] : [videos[0]];
    await rerender({ videos: remaining, pendingKeys: [] });
    expect(
      screen.getByRole('button', {
        name:
          where === 'empty' ? 'Refresh videos' : `Mark ${remaining[0].name}`,
      }),
    ).toHaveFocus();
  },
);
it('does not steal unrelated focus when a background handoff removes a row', async () => {
  const { rerender } = render(VideosPanel, {
    videos,
    onJob: vi.fn(),
    onRefresh: vi.fn(),
  });
  const refresh = screen.getByRole('button', { name: 'Refresh videos' });
  const mark = screen.getByRole('button', { name: 'Mark one.mp4' });
  mark.focus();
  await rerender({ pendingKeys: ['1:1'] });
  nativeDisableBlur(mark);
  refresh.focus();
  await rerender({ videos: [videos[1]], pendingKeys: [] });
  expect(refresh).toHaveFocus();
});
it('returns a disappearing batch action to the stable toolbar after the last source leaves', async () => {
  const user = userEvent.setup();
  const { rerender } = render(VideosPanel, {
    videos,
    onJob: vi.fn(),
    onRefresh: vi.fn(),
  });
  await user.click(screen.getByRole('checkbox', { name: 'Select one.mp4' }));
  await user.click(screen.getByRole('button', { name: 'Mark selected' }));
  const mark = screen.getByRole('button', { name: 'Mark selected' });
  await rerender({ pendingKeys: ['1:1'] });
  nativeDisableBlur(mark);
  await rerender({ videos: [], pendingKeys: [] });
  expect(screen.getByRole('button', { name: 'Refresh videos' })).toHaveFocus();
});
it('restores the initiating action after a retained pending result without moving the source', async () => {
  const { rerender } = render(VideosPanel, {
    videos,
    onJob: vi.fn(),
    onRefresh: vi.fn(),
  });
  const mark = screen.getByRole('button', { name: 'Mark one.mp4' });
  mark.focus();
  await rerender({ pendingKeys: ['1:1'] });
  nativeDisableBlur(mark);
  await rerender({ pendingKeys: [] });
  expect(mark).toHaveFocus();
});
it('abandons remembered pending focus when the user clicks non-focusable content', async () => {
  const { rerender } = render(VideosPanel, {
    videos,
    onJob: vi.fn(),
    onRefresh: vi.fn(),
  });
  const mark = screen.getByRole('button', { name: 'Mark one.mp4' });
  mark.focus();
  await rerender({ pendingKeys: ['1:1'] });
  nativeDisableBlur(mark);
  await fireEvent.pointerDown(screen.getByRole('cell', { name: 'two.mp4' }));
  await rerender({ videos: [videos[1]], pendingKeys: [] });
  expect(document.body).toHaveFocus();
});
it('retains the selected focused native checkbox and uses fresh rename metadata', async () => {
  const user = userEvent.setup();
  const onJob = vi.fn();
  const { rerender } = render(VideosPanel, {
    videos,
    generation: 3,
    onJob,
    onRefresh: vi.fn(),
  });
  const checkbox = screen.getByRole('checkbox', { name: 'Select one.mp4' });
  await user.click(checkbox);
  const renamed = {
    ...videos[0],
    name: 'renamed.mp4',
    duration_ms: 250,
    identity: '{"device":1,"inode":1,"size":10,"modified_ns":1,"changed_ns":2}',
  };
  await rerender({ videos: [renamed, videos[1]] });
  expect(screen.getByRole('checkbox', { name: 'Select renamed.mp4' })).toBe(
    checkbox,
  );
  expect(checkbox).toBeChecked();
  expect(checkbox).toHaveFocus();
  await user.click(screen.getByRole('button', { name: 'Mark selected' }));
  expect(onJob).toHaveBeenLastCalledWith('mark', [renamed], undefined, 3);
  await rerender({ language: 'ar' });
  expect(checkbox).toHaveAccessibleName(/renamed\.mp4/);
  expect(screen.getAllByRole('checkbox')[0]).toBe(checkbox);
  await rerender({ videos: [{ ...renamed, ready: false }] });
  expect(screen.queryByRole('checkbox')).toBeNull();
});
it('exposes precise container duration as ordinary table text', () => {
  const { container } = render(VideosPanel, {
    videos: [
      { ...videos[0], duration_ms: 123514 },
      { ...videos[1], duration_ms: null },
    ],
    onJob: vi.fn(),
    onRefresh: vi.fn(),
  });
  expect(
    screen.getByRole('columnheader', { name: 'Duration' }),
  ).toBeInTheDocument();
  expect(
    screen.getByText('2 minutes, 3 seconds, 514 milliseconds'),
  ).toBeInTheDocument();
  expect(screen.getByText('Duration unavailable')).toBeInTheDocument();
  expect(container.querySelector('[aria-live], [role="status"]')).toBeNull();
});
