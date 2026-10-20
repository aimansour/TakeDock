import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import { initialSession, defaultSettings } from './lib/types';
const mocks = vi.hoisted(() => ({
  bootstrap: vi.fn(),
  listen: vi.fn(() => Promise.resolve(vi.fn())),
  recording: vi.fn(),
  discard: vi.fn(),
  sound: vi.fn(),
  videos: vi.fn(),
}));
vi.mock('./lib/ipc', () => ({ ipc: mocks }));
import App from './App.svelte';
const take = { generation: 7, start_sequence: 1, id: 'take-1' };
beforeEach(() => {
  vi.clearAllMocks();
  mocks.bootstrap.mockResolvedValue({
    settings: defaultSettings,
    errors: [],
    window_name: '',
    jobs: [],
    check_at_startup: false,
    session: {
      ...initialSession,
      generation: 7,
      revision: 1,
      connected: true,
      observer_ready: true,
      foreground: true,
      video_mode: true,
      predicted: 'recording',
      observed: 'recording',
      command_sequence: 1,
      condition: 'ready',
      discard: {
        take,
        eligible: true,
        operation_id: null,
        reason: '',
        phase: 'tracking',
      },
    },
  });
  mocks.recording.mockResolvedValue({
    generation: 7,
    sequence: 2,
    predicted: 'idle',
  });
  mocks.discard.mockResolvedValue({
    generation: 7,
    take,
    operation_id: 'op-1',
  });
});
it('keeps the ordinary recording shortcut non-destructive even when checked', async () => {
  const user = userEvent.setup();
  render(App);
  const checkbox = await screen.findByRole('checkbox', {
    name: 'Delete this take',
  });
  await user.click(checkbox);
  screen.getByRole('button', { name: 'Stop and delete' }).focus();
  await user.keyboard('{Control>}{Shift>}R{/Shift}{/Control}');
  expect(mocks.recording).toHaveBeenCalledWith('stop', 7);
  expect(mocks.discard).not.toHaveBeenCalled();
});
it('sends the exact take token only from the confirmed primary button', async () => {
  const user = userEvent.setup();
  render(App);
  await user.click(
    await screen.findByRole('checkbox', { name: 'Delete this take' }),
  );
  await user.click(screen.getByRole('button', { name: 'Stop and delete' }));
  expect(mocks.discard).toHaveBeenCalledWith(take, 7);
  expect(mocks.recording).not.toHaveBeenCalled();
});
