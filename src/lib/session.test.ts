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
  newWindow: vi.fn(),
  renameWindow: vi.fn(),
}));
vi.mock('./ipc', () => ({ ipc: mocks }));
import { createSession } from './session.svelte';
const video = {
  name: 'one.mp4',
  size: 10,
  modified_ms: 0,
  ready: true,
  identity: 'opaque',
};
beforeEach(() => {
  vi.clearAllMocks();
  mocks.bootstrap.mockResolvedValue({
    session: { ...initialSession, generation: 3, connected: true },
    settings: defaultSettings,
    errors: [],
    window_name: 'Window 1',
    jobs: [],
    check_at_startup: true,
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
  expect(stop).toHaveBeenCalledTimes(6);
});
it('shares settings changes and consumes asynchronous feedback without extra sounds', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      callbacks.set(name, callback);
      return Promise.resolve(vi.fn());
    },
  );
  const app = createSession();
  await app.initialize();
  callbacks.get('settings-changed')?.({
    ...defaultSettings,
    destination: 'C:\\Videos\\TakeDock',
    language: 'ar',
    success_sound: true,
  });
  expect(app.data.settings.language).toBe('ar');
  expect(app.data.settings.destination).toBe('C:\\Videos\\TakeDock');
  callbacks.get('verification-result')?.({
    generation: 3,
    sequence: 1,
    status: 'unconfirmed',
    message: 'verification_unconfirmed',
  });
  expect(app.data.result).toBe('verification_unconfirmed');
  expect(mocks.sound).not.toHaveBeenCalled();
  app.dispose();
});
it('does not replace a settings broadcast with an older bootstrap snapshot', async () => {
  let resolve!: (value: unknown) => void;
  let settingsChanged!: (value: unknown) => void;
  mocks.bootstrap.mockImplementation(
    () =>
      new Promise((value) => {
        resolve = value;
      }),
  );
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      if (name === 'settings-changed') settingsChanged = callback;
      return Promise.resolve(vi.fn());
    },
  );
  const app = createSession();
  const initialization = app.initialize();
  await vi.waitFor(() => expect(mocks.bootstrap).toHaveBeenCalled());
  settingsChanged({ ...defaultSettings, language: 'ar' });
  resolve({
    session: { ...initialSession, generation: 3 },
    settings: defaultSettings,
    errors: [],
    window_name: 'Window 1',
    jobs: [],
    check_at_startup: false,
  });
  await initialization;
  expect(app.data.settings.language).toBe('ar');
  app.dispose();
});
it('restores shared progress in a new window without another startup update check', async () => {
  mocks.bootstrap.mockResolvedValue({
    session: { ...initialSession, generation: 3, connected: true },
    settings: defaultSettings,
    errors: [],
    window_name: 'Interview',
    check_at_startup: false,
    jobs: [
      {
        id: 'copy-one',
        generation: 3,
        kind: 'copy',
        status: 'running',
        phase: 'verifying',
        name: 'one.mp4',
        bytes: 10,
        total: 10,
        completed: 0,
        count: 1,
        message: '',
        results: [],
      },
    ],
  });
  const app = createSession();
  await app.initialize();
  expect(app.data.windowName).toBe('Interview');
  expect(app.data.jobs[0].phase).toBe('verifying');
  expect(mocks.checkUpdate).not.toHaveBeenCalled();
  app.dispose();
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
  await app.refresh();
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
it('invalidates an expired listing and running activity on connection replacement', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation(
    (name: string, cb: (value: unknown) => void) => {
      callbacks.set(name, cb);
      return Promise.resolve(vi.fn());
    },
  );
  mocks.videos.mockResolvedValue([video] as never);
  const app = createSession();
  await app.initialize();
  await app.refresh();
  const oldVideos = [...app.data.videos];
  callbacks.get('file-job')?.({
    id: 'old-job',
    generation: 3,
    kind: 'copy',
    status: 'queued',
    results: [],
  });
  callbacks.get('session-state')?.({
    ...initialSession,
    generation: 4,
    connected: true,
  });
  expect(app.data.videos).toEqual([]);
  expect(app.data.jobs).toEqual([]);
  expect(app.data.loading).toBe(false);
  app.job('delete', oldVideos, undefined, 3);
  expect(mocks.job).not.toHaveBeenCalled();
  app.dispose();
});
it('keeps the authoritative shared settings when save replies arrive in reverse order', async () => {
  let broadcast!: (value: unknown) => void;
  mocks.listen.mockImplementation(
    (name: string, cb: (value: unknown) => void) => {
      if (name === 'settings-changed') broadcast = cb;
      return Promise.resolve(vi.fn());
    },
  );
  const app = createSession();
  await app.initialize();
  let finish!: (value: unknown) => void;
  mocks.save.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const first = { ...defaultSettings, destination: 'C:\\First' };
  const second = {
    ...defaultSettings,
    destination: 'C:\\Second',
    language: 'ar' as const,
  };
  const saving = app.save(first);
  broadcast(first);
  broadcast(second);
  finish(first);
  await saving;
  expect(app.data.settings).toEqual(second);
  app.dispose();
});
