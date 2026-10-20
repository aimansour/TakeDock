import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { expect, it, vi } from 'vitest';
import RecordingPanel from './RecordingPanel.svelte';
import { initialSession } from '../lib/types';

const take = { generation: 7, start_sequence: 1, id: 'take-1' };
const recording = {
  ...initialSession,
  generation: 7,
  connected: true,
  observer_ready: true,
  foreground: true,
  video_mode: true,
  predicted: 'recording' as const,
  observed: 'recording' as const,
  command_sequence: 1,
  condition: 'ready',
  discard: {
    take,
    eligible: true,
    reason: '',
    operation_id: null,
    phase: 'tracking' as const,
  },
};
it.each([
  ['en', 'Delete this take', 'Stop', 'Stop and delete'],
  ['ar', 'حذف هذه المحاولة', 'إيقاف', 'إيقاف وحذف'],
] as const)(
  'confirms only the current take with native %s controls',
  async (language, label, stop, discard) => {
    const user = userEvent.setup();
    const onAction = vi.fn();
    const onDiscard = vi.fn();
    const { container } = render(RecordingPanel, {
      state: recording,
      language,
      onAction,
      onDiscard,
    });
    const checkbox = screen.getByRole('checkbox', { name: label });
    const primary = screen.getByRole('button', { name: stop });
    expect(checkbox).not.toBeChecked();
    checkbox.focus();
    await user.keyboard(' ');
    expect(checkbox).toBeChecked();
    expect(onDiscard).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: discard })).toBe(primary);
    primary.focus();
    await user.keyboard('{Enter}');
    expect(onDiscard).toHaveBeenCalledTimes(1);
    expect(onAction).not.toHaveBeenCalled();
    expect(primary).toHaveFocus();
    expect(container.querySelector('section')).toHaveAttribute(
      'dir',
      language === 'ar' ? 'rtl' : 'ltr',
    );
    expect(
      container.querySelector(
        '[aria-live], [role="alert"], [role="status"], button:disabled',
      ),
    ).toBeNull();
  },
);
it('resets local confirmation for a new take and generation', async () => {
  const user = userEvent.setup();
  const { rerender } = render(RecordingPanel, {
    state: recording,
    onAction: vi.fn(),
    onDiscard: vi.fn(),
  });
  await user.click(screen.getByRole('checkbox', { name: 'Delete this take' }));
  await rerender({
    state: {
      ...recording,
      discard: {
        ...recording.discard,
        take: { ...take, id: 'take-2', start_sequence: 3 },
      },
    },
  });
  expect(
    screen.getByRole('checkbox', { name: 'Delete this take' }),
  ).not.toBeChecked();
  await user.click(screen.getByRole('checkbox', { name: 'Delete this take' }));
  await rerender({ state: { ...recording, generation: 8 } });
  expect(screen.queryByRole('checkbox')).toBeNull();
  expect(screen.getByRole('button', { name: 'Stop' })).toBeInTheDocument();
});
it('keeps confirmation local to each window and resets after remount', async () => {
  const user = userEvent.setup();
  const props = { state: recording, onAction: vi.fn(), onDiscard: vi.fn() };
  const first = render(RecordingPanel, props);
  render(RecordingPanel, props);
  const checkboxes = screen.getAllByRole('checkbox', {
    name: 'Delete this take',
  });
  await user.click(checkboxes[0]);
  expect(checkboxes[0]).toBeChecked();
  expect(checkboxes[1]).not.toBeChecked();
  first.unmount();
  render(RecordingPanel, props);
  expect(
    screen
      .getAllByRole('checkbox', { name: 'Delete this take' })
      .every((checkbox) => !(checkbox as HTMLInputElement).checked),
  ).toBe(true);
});
it('moves focus only when the focused checkbox disappears', async () => {
  const { rerender } = render(RecordingPanel, {
    state: recording,
    onAction: vi.fn(),
    onDiscard: vi.fn(),
  });
  const primary = screen.getByRole('button', { name: 'Stop' });
  screen.getByRole('checkbox').focus();
  await rerender({
    state: {
      ...recording,
      discard: {
        ...recording.discard,
        eligible: false,
        operation_id: 'op-1',
        phase: 'waiting_to_stop',
      },
    },
  });
  await tick();
  expect(screen.queryByRole('checkbox')).toBeNull();
  expect(primary).toHaveFocus();
  await rerender({
    state: {
      ...recording,
      discard: {
        ...recording.discard,
        eligible: false,
        operation_id: 'op-1',
        phase: 'confirming_stop',
      },
    },
  });
  expect(primary).toHaveFocus();
  expect(screen.getByRole('button', { name: 'Stop' })).toBe(primary);
});
