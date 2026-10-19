import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import RecordingPanel from './RecordingPanel.svelte';
import { initialSession } from '../lib/types';
import { ipc } from '../lib/ipc';
afterEach(() => vi.restoreAllMocks());
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
it.each([
  ['en', 'Preparing to speak', 'Ready to speak', 'Stop video'],
  ['ar', 'جارٍ التجهيز للكلام', 'جاهز للكلام', 'إنهاء الفيديو'],
] as const)(
  'renders ordinary %s readiness without moving focus or playing audio',
  async (language, preparing, speaking, stopName) => {
    const sound = vi.spyOn(ipc, 'sound').mockResolvedValue(undefined);
    const recording = {
      ...ready,
      predicted: 'recording' as const,
      observed: 'recording' as const,
    };
    const { rerender, container } = render(RecordingPanel, {
      state: recording,
      language,
      onAction: vi.fn(),
    });
    expect(container.querySelector('#speech-readiness')).toBeNull();
    const stop = screen.getByRole('button', { name: stopName });
    stop.focus();
    await rerender({ state: { ...recording, speech_readiness: 'preparing' } });
    expect(screen.getByText(preparing)).toHaveAttribute(
      'id',
      'speech-readiness',
    );
    expect(screen.getByText(preparing)).toHaveAttribute(
      'data-readiness',
      'preparing',
    );
    expect(stop).toHaveFocus();
    await rerender({ state: { ...recording, speech_readiness: 'ready' } });
    expect(screen.getByText(speaking)).toHaveAttribute(
      'data-readiness',
      'ready',
    );
    expect(screen.getByRole('button', { name: stopName })).toBe(stop);
    expect(stop).toHaveFocus();
    await rerender({ state: recording });
    expect(container.querySelector('#speech-readiness')).toBeNull();
    expect(stop).toHaveFocus();
    expect(container.querySelector('section')).toHaveAttribute(
      'dir',
      language === 'ar' ? 'rtl' : 'ltr',
    );
    expect(
      container.querySelector('[aria-live], [role="alert"], [role="status"]'),
    ).toBeNull();
    expect(sound).not.toHaveBeenCalled();
  },
);
it.each([
  { foreground: false },
  { connected: false },
  { predicted: 'unknown' as const, condition: 'verification_unconfirmed' },
])(
  'preserves capture focus on a stable target when controls disappear: %o',
  async (patch) => {
    const { rerender } = render(RecordingPanel, {
      state: { ...ready, predicted: 'recording' },
      onAction: vi.fn(),
    });
    const stop = screen.getByRole('button', { name: 'Stop video' });
    stop.focus();
    await rerender({ state: { ...ready, ...patch } });
    await tick();
    expect(document.activeElement).not.toBe(document.body);
    expect(document.activeElement?.textContent).toBe('Recording');
  },
);
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
