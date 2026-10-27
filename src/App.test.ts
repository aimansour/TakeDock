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
  fileResults: vi.fn(),
  acknowledgeFileResult: vi.fn(),
}));
vi.mock('./lib/ipc', () => ({ ipc: mocks }));
import App from './App.svelte';
import { tick } from 'svelte';
const take = { generation: 7, start_sequence: 1, id: 'take-1' };
beforeEach(() => {
  vi.clearAllMocks();
  mocks.bootstrap.mockResolvedValue({
    settings: defaultSettings,
    errors: [],
    window_name: '',
    file_results: { revision: 0, now_ms: 0, notices: [], history: [] },
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
it('success_expires_from_notice_activity_and_expanded_job_surfaces', async () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
  const terminal = {
    id: 'copy-done',
    generation: 7,
    kind: 'mark',
    status: 'completed',
    name: 'clip.mp4',
    completed: 1,
    count: 1,
    message: '',
    results: [{ name: 'clip.mp4', status: 'completed', message: '' }],
  };
  const notice = {
    ...terminal,
    key: { generation: 7, owner: 'job', operation_id: terminal.id },
    revision: 1,
    outcome: 'completed',
    code: 'completed',
    warnings: [],
    expires_at_ms: 10000,
  };
  const initial = await mocks.bootstrap();
  mocks.bootstrap.mockResolvedValue({
    ...initial,
    jobs: [terminal],
    file_results: {
      revision: 1,
      now_ms: 0,
      notices: [notice],
      history: [notice],
    },
  });
  const { container } = render(App);
  try {
    for (let i = 0; i < 8; i++) await tick();
    expect(screen.getByRole('progressbar')).toHaveAttribute('value', '100');
    expect(container.querySelectorAll('.file-result-notice')).toHaveLength(1);
    expect(container.querySelector('details')?.open).toBe(false);
    vi.advanceTimersByTime(10000);
    await tick();
    expect(container.querySelectorAll('.file-result-notice')).toHaveLength(0);
    expect(screen.getByRole('progressbar')).toHaveAttribute('value', '0');
    expect(container.querySelector('.activity-panel')).toHaveTextContent(
      'Ready',
    );
    expect(container.querySelectorAll('details article')).toHaveLength(1);
    expect(container.querySelectorAll('article')).toHaveLength(1);
  } finally {
    vi.useRealTimers();
  }
});
