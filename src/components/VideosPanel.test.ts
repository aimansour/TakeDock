import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import VideosPanel from './VideosPanel.svelte';
import type { Video } from '../lib/types';
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
  await user.click(screen.getByRole('button', { name: 'Copy to computer' }));
  expect(onJob).toHaveBeenCalledWith('copy', videos);
  await user.click(screen.getByRole('button', { name: 'Delete from phone' }));
  expect(onJob.mock.calls.length).toBe(1);
  await user.click(screen.getByRole('button', { name: 'Confirm deletion' }));
  expect(onJob).toHaveBeenLastCalledWith('delete', videos);
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
  expect(screen.getByText('one.mp4')).toBeInTheDocument();
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
