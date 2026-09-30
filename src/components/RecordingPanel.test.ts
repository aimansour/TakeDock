import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { expect, it, vi } from 'vitest';
import RecordingPanel from './RecordingPanel.svelte';
import { initialSession } from '../lib/types';
const ready = {
  ...initialSession,
  generation: 7,
  connected: true,
  observer_ready: true,
  foreground: true,
  video_mode: true,
  predicted: 'idle' as const,
  observed: 'idle' as const,
  condition: 'ready',
};
it('renders only applicable controls and keeps the primary native button focused', async () => {
  const { rerender, container } = render(RecordingPanel, {
    state: ready,
    onAction: vi.fn(),
  });
  const primary = screen.getByRole('button', { name: 'Start video' });
  primary.focus();
  primary.click();
  await rerender({ state: { ...ready, predicted: 'recording' } });
  expect(screen.queryByRole('button', { name: 'Start video' })).toBeNull();
  expect(screen.getByRole('button', { name: 'Stop video' })).toBe(primary);
  expect(primary).toHaveFocus();
  const pause = screen.getByRole('button', { name: 'Pause video' });
  pause.focus();
  await rerender({ state: { ...ready, predicted: 'paused' } });
  expect(screen.getByRole('button', { name: 'Resume video' })).toBe(pause);
  await rerender({ state: ready });
  await tick();
  expect(primary).toHaveFocus();
  expect(
    container.querySelector(
      '[aria-live], [role="alert"], [role="status"], button:disabled',
    ),
  ).toBeNull();
});
