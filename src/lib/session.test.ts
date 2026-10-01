import { it, expect, vi, beforeEach } from 'vitest';
import { initialSession, defaultSettings } from './types';
const mocks = vi.hoisted(() => ({
  listen: vi.fn(),
  bootstrap: vi.fn(),
  sound: vi.fn(() => Promise.resolve()),
  videos: vi.fn(() => Promise.resolve([])),
  job: vi.fn(),
  save: vi.fn(),
  recording: vi.fn(),
  cancel: vi.fn(),
  reconnect: vi.fn(),
  checkUpdate: vi.fn(),
  installUpdate: vi.fn(),
}));
vi.mock('./ipc', () => ({ ipc: mocks }));
import { createSession } from './session.svelte';
beforeEach(() => {
  vi.clearAllMocks();
  mocks.bootstrap.mockResolvedValue({
    session: { ...initialSession, generation: 3, connected: true },
    settings: defaultSettings,
    errors: [],
  });
  mocks.listen.mockImplementation(() => Promise.resolve(vi.fn()));
  mocks.checkUpdate.mockResolvedValue(null);
});
it('unsubscribes successful listeners when one subscription fails', async () => {
  const stop = vi.fn();
  mocks.listen.mockImplementation((name: string) =>
    name === 'file-job'
      ? Promise.reject(new Error('subscription failed'))
      : Promise.resolve(stop),
  );
  const app = createSession();
  await app.initialize();
  app.dispose();
  expect(stop).toHaveBeenCalledTimes(5);
});
it('checks at startup and leaves installation to explicit activation', async () => {
  mocks.checkUpdate.mockResolvedValue({
    version: '0.2.0',
    notes: 'Fixes',
    date: '',
  });
  const app = createSession();
  await app.initialize();
  await Promise.resolve();
  expect(mocks.checkUpdate).toHaveBeenCalledTimes(1);
  expect(app.data.update.status).toBe('available');
  expect(mocks.installUpdate).not.toHaveBeenCalled();
  app.dispose();
});
it('makes accepted queued jobs available for cancellation without a worker result', async () => {
  mocks.job.mockResolvedValue('job-one');
  const app = createSession();
  await app.initialize();
  app.job('copy', [
    {
      name: 'one.mp4',
      size: 10,
      modified_ms: 0,
      ready: true,
      identity: 'opaque',
    },
  ]);
  await Promise.resolve();
  await Promise.resolve();
  expect(app.data.jobs[0]).toMatchObject({
    id: 'job-one',
    status: 'queued',
    generation: 3,
  });
  app.dispose();
});
