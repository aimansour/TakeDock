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
  discard: vi.fn(),
  cancel: vi.fn(),
  reconnect: vi.fn(),
  checkUpdate: vi.fn(),
  installUpdate: vi.fn(),
  newWindow: vi.fn(),
  renameWindow: vi.fn(),
  fileResults: vi.fn(),
  acknowledgeFileResult: vi.fn(),
}));
vi.mock('./ipc', () => ({ ipc: mocks }));
import { createSession } from './session.svelte';

const handoffSource = {
  name: 'ready.mp4',
  size: 10,
  modified_ms: 0,
  ready: true,
  identity:
    '{"device":9007199254740993,"inode":9007199254740995,"size":10,"modified_ns":0,"changed_ns":1}',
};
it('committed handoff removes a source immediately and rejects an older in-flight listing', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation((name, callback) => {
    callbacks.set(name, callback);
    return Promise.resolve(vi.fn());
  });
  mocks.videos.mockResolvedValueOnce([handoffSource] as never);
  const app = createSession();
  await app.initialize();
  await app.refresh();
  let finish!: (value: never) => void;
  mocks.videos.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const old = app.refresh();
  const update = {
    generation: 3,
    job_id: 'job',
    operation_id: 'one',
    sequence: 1,
    source_name: handoffSource.name,
    source_identity: handoffSource.identity,
  };
  callbacks.get('file-handed-off')?.(update);
  expect(app.data.videos).toEqual([]);
  finish([handoffSource] as never);
  await old;
  expect(app.data.videos).toEqual([]);
  const replacement = {
    ...handoffSource,
    identity: handoffSource.identity.replace(
      '9007199254740995',
      '9007199254740996',
    ),
  };
  mocks.videos.mockResolvedValueOnce([replacement] as never);
  await app.refresh();
  callbacks.get('file-handed-off')?.(update);
  expect(app.data.videos).toEqual([replacement]);
  app.dispose();
});
it('reserves initiating rows synchronously, refuses competing actions and releases them on failed settlement', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation((name, callback) => {
    callbacks.set(name, callback);
    return Promise.resolve(vi.fn());
  });
  mocks.videos.mockResolvedValue([handoffSource] as never);
  const accept: ((value: string) => void)[] = [];
  mocks.job.mockImplementation(
    () =>
      new Promise<string>((resolve) => {
        accept.push(resolve);
      }),
  );
  const app = createSession();
  await app.initialize();
  await app.refresh();
  app.job('mark', [handoffSource]);
  expect(app.data.pendingKeys).toEqual(['9007199254740993:9007199254740995']);
  app.job('delete', [handoffSource]);
  expect(mocks.job).toHaveBeenCalledTimes(1);
  accept[0]('job');
  await Promise.resolve();
  await Promise.resolve();
  callbacks.get('file-job')?.({
    generation: 3,
    id: 'job',
    kind: 'mark',
    status: 'failed',
    phase: 'failed',
    name: handoffSource.name,
    completed: 0,
    count: 1,
    message: 'destination_exists',
    results: [
      {
        name: handoffSource.name,
        status: 'retained',
        message: 'destination_exists',
      },
    ],
  });
  await Promise.resolve();
  await Promise.resolve();
  expect(app.data.pendingKeys).toEqual([]);
  expect(app.data.videos).toEqual([handoffSource]);
  app.dispose();
});
it('does not dispatch a retired desktop-copy command from a stale caller', async () => {
  const app = createSession();
  await app.initialize();
  app.job('copy' as never, [handoffSource], undefined, 3);
  expect(mocks.job).not.toHaveBeenCalled();
  expect(app.data.result).toBe('invalid_file_job');
  app.dispose();
});
it('committed receipt releases a row even before delayed command acceptance returns', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation((name, callback) => {
    callbacks.set(name, callback);
    return Promise.resolve(vi.fn());
  });
  mocks.videos.mockResolvedValueOnce([handoffSource] as never);
  let accept!: (value: string) => void;
  mocks.job.mockImplementationOnce(
    () =>
      new Promise<string>((resolve) => {
        accept = resolve;
      }),
  );
  const app = createSession();
  await app.initialize();
  await app.refresh();
  app.job('mark', [handoffSource]);
  callbacks.get('file-handed-off')?.({
    generation: 3,
    job_id: 'delayed',
    operation_id: 'done',
    sequence: 1,
    source_name: handoffSource.name,
    source_identity: handoffSource.identity,
  });
  expect(app.data.pendingKeys).toEqual([]);
  expect(app.data.videos).toEqual([]);
  accept('delayed');
  await Promise.resolve();
  await Promise.resolve();
  expect(app.data.jobs.some((job) => job.id === 'delayed')).toBe(false);
  app.dispose();
});

it('retains an active accepted job after twenty-one terminal results', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      callbacks.set(name, callback);
      return Promise.resolve(vi.fn());
    },
  );
  const app = createSession();
  await app.initialize();
  const queued = {
    id: 'held',
    generation: 3,
    kind: 'mark',
    status: 'queued',
    name: 'one.mp4',
    completed: 0,
    count: 1,
    message: '',
    results: [],
  };
  callbacks.get('file-job')?.(queued);
  for (let i = 0; i < 21; i++)
    callbacks.get('file-job')?.({
      ...queued,
      id: `done-${i}`,
      status: 'completed',
    });
  expect(app.data.jobs.some((job) => job.id === 'held')).toBe(true);
  app.cancel('held');
  expect(mocks.cancel).toHaveBeenCalledWith('held');
  app.dispose();
});

it('does not revive an evicted terminal job from delayed acceptance or progress', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation((name, callback) => {
    callbacks.set(name, callback);
    return Promise.resolve(vi.fn());
  });
  let accept!: (id: string) => void;
  mocks.job.mockImplementation(
    () =>
      new Promise((resolve) => {
        accept = resolve;
      }),
  );
  const app = createSession();
  await app.initialize();
  await app.refresh();
  app.job('mark', [video]);
  const terminal = {
    id: 'first',
    generation: 3,
    kind: 'mark',
    status: 'completed',
    name: 'one.mp4',
    completed: 1,
    count: 1,
    message: '',
    results: [],
  };
  callbacks.get('file-job')?.(terminal);
  for (let i = 0; i < 21; i++)
    callbacks.get('file-job')?.({ ...terminal, id: `later-${i}` });
  expect(app.data.jobs.some((job) => job.id === 'first')).toBe(false);
  accept('first');
  await Promise.resolve();
  await Promise.resolve();
  expect(app.data.jobs.some((job) => job.id === 'first')).toBe(false);
  callbacks.get('file-job')?.({ ...terminal, status: 'queued' });
  expect(app.data.jobs.some((job) => job.id === 'first')).toBe(false);
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  try {
    callbacks.get('file-job')?.({ ...terminal, status: 'running' });
    await vi.advanceTimersByTimeAsync(100);
    expect(app.data.jobs.some((job) => job.id === 'first')).toBe(false);
  } finally {
    vi.useRealTimers();
  }
  app.dispose();
});

it('does not revive evicted terminal work from a delayed bootstrap', async () => {
  const initial = await mocks.bootstrap();
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation((name, callback) => {
    callbacks.set(name, callback);
    return Promise.resolve(vi.fn());
  });
  let bootstrap!: (value: unknown) => void;
  mocks.bootstrap.mockImplementation(
    () =>
      new Promise((resolve) => {
        bootstrap = resolve;
      }),
  );
  const app = createSession();
  const initializing = app.initialize();
  await vi.waitFor(() => expect(bootstrap).toBeTypeOf('function'));
  callbacks.get('session-state')?.(initial.session);
  const terminal = {
    id: 'first',
    generation: 3,
    kind: 'mark',
    status: 'completed',
    name: 'one.mp4',
    completed: 1,
    count: 1,
    message: '',
    results: [],
  };
  callbacks.get('file-job')?.(terminal);
  for (let i = 0; i < 21; i++)
    callbacks.get('file-job')?.({ ...terminal, id: `later-${i}` });
  bootstrap({ ...initial, jobs: [{ ...terminal, status: 'running' }] });
  await initializing;
  expect(app.data.jobs.some((job) => job.id === 'first')).toBe(false);
  app.dispose();
});

it('reconciles a bootstrap terminal result over an earlier queued event', async () => {
  const initial = await mocks.bootstrap();
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation((name, callback) => {
    callbacks.set(name, callback);
    return Promise.resolve(vi.fn());
  });
  let bootstrap!: (value: unknown) => void;
  mocks.bootstrap.mockImplementation(
    () =>
      new Promise((resolve) => {
        bootstrap = resolve;
      }),
  );
  const app = createSession();
  const initializing = app.initialize();
  await vi.waitFor(() => expect(bootstrap).toBeTypeOf('function'));
  callbacks.get('session-state')?.(initial.session);
  const terminal = {
    id: 'first',
    generation: 3,
    kind: 'mark',
    status: 'completed',
    name: 'one.mp4',
    completed: 1,
    count: 1,
    message: '',
    results: [],
  };
  callbacks.get('file-job')?.({ ...terminal, status: 'queued' });
  bootstrap({ ...initial, jobs: [terminal] });
  await initializing;
  expect(app.data.jobs.find((job) => job.id === 'first')?.status).toBe(
    'completed',
  );
  app.dispose();
});

it('file success cannot overwrite an unrelated verification message', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      callbacks.set(name, callback);
      return Promise.resolve(vi.fn());
    },
  );
  const app = createSession();
  await app.initialize();
  callbacks.get('verification-result')?.({
    generation: 3,
    sequence: 1,
    status: 'unconfirmed',
    message: 'verification_unconfirmed',
  });
  callbacks.get('file-job')?.({
    id: 'done',
    generation: 3,
    kind: 'mark',
    status: 'completed',
    name: 'one.mp4',
    completed: 1,
    count: 1,
    message: '',
    results: [],
  });
  expect(app.data.result).toBe('verification_unconfirmed');
  app.dispose();
});

it('expires a bootstrap file success without listing, audio or capture work', async () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'performance'] });
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      callbacks.set(name, callback);
      return Promise.resolve(vi.fn());
    },
  );
  const notice = {
    key: { generation: 3, owner: 'job', operation_id: 'done' },
    revision: 1,
    kind: 'mark',
    outcome: 'completed',
    code: 'completed',
    warnings: [],
    name: 'one.mp4',
    completed: 1,
    count: 1,
    results: [],
    expires_at_ms: 10_000,
  };
  mocks.bootstrap.mockResolvedValue({
    session: { ...initialSession, generation: 3, connected: true },
    settings: defaultSettings,
    errors: [],
    window_name: '',
    jobs: [],
    check_at_startup: false,
    file_results: {
      revision: 1,
      now_ms: 0,
      notices: [notice],
      history: [notice],
    },
  });
  const app = createSession();
  try {
    await app.initialize();
    expect(app.data.fileResults.notices).toHaveLength(1);
    callbacks.get('verification-result')?.({
      generation: 3,
      sequence: 1,
      status: 'unconfirmed',
      message: 'verification_unconfirmed',
    });
    vi.advanceTimersByTime(9_999);
    expect(app.data.fileResults.notices).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(app.data.fileResults.notices).toHaveLength(0);
    expect(app.data.fileResults.history).toHaveLength(1);
    expect(app.data.result).toBe('verification_unconfirmed');
    expect(mocks.videos).not.toHaveBeenCalled();
    expect(mocks.sound).not.toHaveBeenCalled();
    expect(mocks.recording).not.toHaveBeenCalled();
  } finally {
    app.dispose();
    vi.useRealTimers();
  }
});
const video = {
  name: 'one.mp4',
  size: 10,
  modified_ms: 0,
  ready: true,
  identity: 'opaque',
};
it('old_listing_cannot_restore_renamed_identity', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      callbacks.set(name, callback);
      return Promise.resolve(vi.fn());
    },
  );
  const a = {
    ...video,
    identity:
      '{"device":9007199254740993,"inode":9007199254740995,"size":10,"modified_ns":0,"changed_ns":1}',
  };
  const b = {
    ...a,
    name: 'renamed.mp4',
    duration_ms: 123,
    identity: a.identity.replace('"changed_ns":1', '"changed_ns":2'),
  };
  mocks.videos.mockResolvedValueOnce([a] as never);
  const app = createSession();
  await app.initialize();
  await app.refresh();
  let finish!: (value: never) => void;
  mocks.videos.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const old = app.refresh();
  callbacks.get('file-renamed')?.({
    generation: 3,
    id: 'rename',
    sequence: 1,
    source_name: a.name,
    source_identity: a.identity,
    video: b,
  });
  expect(app.data.videos).toEqual([b]);
  expect(app.data.loading).toBe(false);
  finish([a] as never);
  await old;
  expect(app.data.videos).toEqual([b]);
  mocks.videos.mockResolvedValueOnce([a] as never);
  await app.refresh();
  expect(app.data.videos).toEqual([b]);
  expect(mocks.sound).not.toHaveBeenCalled();
  expect(mocks.recording).not.toHaveBeenCalled();
  app.dispose();
});
it('rename_invalidation_releases_loading_owner', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      callbacks.set(name, callback);
      return Promise.resolve(vi.fn());
    },
  );
  const a = {
    ...video,
    identity: '{"device":1,"inode":2,"size":10,"modified_ns":0,"changed_ns":1}',
  };
  const b = { ...a, name: 'new.mp4' };
  const app = createSession();
  await app.initialize();
  let rejectOld!: (reason: unknown) => void;
  mocks.videos.mockImplementationOnce(
    () =>
      new Promise((_, reject) => {
        rejectOld = reject;
      }),
  );
  const old = app.refresh();
  callbacks.get('file-renamed')?.({
    generation: 3,
    id: 'rename',
    sequence: 1,
    source_name: a.name,
    source_identity: a.identity,
    video: b,
  });
  expect(app.data.loading).toBe(false);
  let finishNew!: (value: never) => void;
  mocks.videos.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finishNew = resolve;
      }),
  );
  const fresh = app.refresh();
  rejectOld('obsolete listing error');
  await old;
  expect(app.data.loading).toBe(true);
  expect(app.data.result).toBe('');
  finishNew([a] as never);
  await fresh;
  expect(app.data.videos).toEqual([b]);
  expect(app.data.loading).toBe(false);
  app.dispose();
});
beforeEach(() => {
  vi.clearAllMocks();
  mocks.videos.mockReset().mockResolvedValue([]);
  mocks.job.mockReset().mockResolvedValue('accepted');
  mocks.bootstrap.mockResolvedValue({
    session: { ...initialSession, generation: 3, connected: true },
    settings: defaultSettings,
    errors: [],
    window_name: 'Window 1',
    file_results: { revision: 0, now_ms: 0, notices: [], history: [] },
    jobs: [],
    check_at_startup: true,
  });
  mocks.listen.mockImplementation(() => Promise.resolve(vi.fn()));
  mocks.checkUpdate.mockResolvedValue(null);
  mocks.cancel.mockResolvedValue(undefined);
  mocks.fileResults.mockResolvedValue({
    revision: 0,
    now_ms: 0,
    notices: [],
    history: [],
  });
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
  expect(stop).toHaveBeenCalledTimes(9);
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
  expect(app.data.settings).not.toHaveProperty('destination');
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
it('shares readiness settings and backend state across windows without frontend audio', async () => {
  const callbacks = new Map<string, ((value: unknown) => void)[]>();
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      callbacks.set(name, [...(callbacks.get(name) ?? []), callback]);
      return Promise.resolve(vi.fn());
    },
  );
  const first = createSession();
  const second = createSession();
  await Promise.all([first.initialize(), second.initialize()]);
  const saved = {
    ...defaultSettings,
    language: 'ar' as const,
    ready_sound: false,
    ready_delay_ms: 1250,
  };
  for (const callback of callbacks.get('settings-changed') ?? [])
    callback(saved);
  for (const app of [first, second]) {
    expect(app.data.settings).toMatchObject({
      language: 'ar',
      ready_sound: false,
      ready_delay_ms: 1250,
      success_sound: false,
    });
  }
  for (const [revision, state] of [
    [1, 'preparing'],
    [2, 'ready'],
  ] as const) {
    for (const callback of callbacks.get('session-state') ?? [])
      callback({
        ...initialSession,
        generation: 3,
        revision,
        connected: true,
        observer_ready: true,
        foreground: true,
        video_mode: true,
        predicted: 'recording',
        observed: 'recording',
        command_sequence: 1,
        speech_readiness: state,
      });
    expect(first.data.session.speech_readiness).toBe(state);
    expect(second.data.session.speech_readiness).toBe(state);
  }
  expect(mocks.sound).not.toHaveBeenCalled();
  first.dispose();
  second.dispose();
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
    file_results: { revision: 0, now_ms: 0, notices: [], history: [] },
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
    file_results: { revision: 0, now_ms: 0, notices: [], history: [] },
    check_at_startup: false,
    jobs: [
      {
        id: 'copy-one',
        generation: 3,
        kind: 'mark',
        status: 'running',
        phase: 'marking',
        name: 'one.mp4',
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
  expect(app.data.jobs[0].phase).toBe('marking');
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
  app.job('mark', [
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
    kind: 'mark',
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
  const first = { ...defaultSettings, ready_delay_ms: 1000 };
  const second = {
    ...defaultSettings,
    ready_delay_ms: 1250,
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

it('owns discard results once and ignores generic job completion for that operation', async () => {
  const callbacks = new Map<string, (value: unknown) => void>();
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      callbacks.set(name, callback);
      return Promise.resolve(vi.fn());
    },
  );
  const take = { generation: 3, start_sequence: 1, id: 'take-1' };
  mocks.bootstrap.mockResolvedValue({
    session: {
      ...initialSession,
      generation: 3,
      connected: true,
      discard: {
        take,
        eligible: true,
        operation_id: null,
        reason: '',
        phase: 'tracking',
      },
    },
    settings: defaultSettings,
    errors: [],
    jobs: [],
    window_name: '',
    file_results: { revision: 0, now_ms: 0, notices: [], history: [] },
    check_at_startup: false,
  });
  mocks.discard.mockResolvedValue({
    generation: 3,
    take,
    operation_id: 'op-1',
  });
  const app = createSession();
  await app.initialize();
  app.discard();
  expect(mocks.discard).toHaveBeenCalledWith(take, 3);
  const result = {
    generation: 3,
    operation_id: 'op-1',
    status: 'completed',
    code: 'discard_completed',
  };
  callbacks.get('discard-result')?.(result);
  const notice = {
    key: { generation: 3, owner: 'discard', operation_id: 'op-1' },
    revision: 1,
    kind: 'delete',
    outcome: 'completed',
    code: 'discard_completed',
    warnings: [],
    name: 'take.mp4',
    completed: 1,
    count: 1,
    results: [],
    expires_at_ms: 10_000,
  };
  mocks.fileResults.mockResolvedValue({
    revision: 2,
    now_ms: 0,
    notices: [notice],
    history: [notice],
  });
  callbacks.get('file-results-changed')?.({ revision: 2 });
  await Promise.resolve();
  expect(app.data.result).toBe('');
  expect(app.data.fileResults.notices[0].key.operation_id).toBe('op-1');
  expect(mocks.videos).toHaveBeenCalledTimes(1);
  callbacks.get('file-job')?.({
    id: 'delete-1',
    generation: 3,
    kind: 'delete',
    status: 'completed',
    discard_operation_id: 'op-1',
    results: [],
  });
  callbacks.get('discard-result')?.(result);
  expect(app.data.result).toBe('');
  expect(mocks.videos).toHaveBeenCalledTimes(1);
  expect(mocks.sound).not.toHaveBeenCalled();
  app.dispose();
});
it('ignores a discard rejection after a newer take replaces its request', async () => {
  let incoming!: (value: unknown) => void;
  mocks.listen.mockImplementation(
    (name: string, callback: (value: unknown) => void) => {
      if (name === 'session-state') incoming = callback;
      return Promise.resolve(vi.fn());
    },
  );
  const take = { generation: 3, start_sequence: 1, id: 'old' };
  const session = {
    ...initialSession,
    generation: 3,
    revision: 1,
    connected: true,
    discard: {
      take,
      eligible: true,
      operation_id: null,
      reason: '',
      phase: 'tracking',
    },
  };
  mocks.bootstrap.mockResolvedValue({
    session,
    settings: defaultSettings,
    errors: [],
    jobs: [],
    window_name: '',
    file_results: { revision: 0, now_ms: 0, notices: [], history: [] },
    check_at_startup: false,
  });
  let reject!: (error: Error) => void;
  mocks.discard.mockImplementation(
    () =>
      new Promise((_resolve, rejected) => {
        reject = rejected;
      }),
  );
  const app = createSession();
  await app.initialize();
  app.discard();
  incoming({
    ...session,
    revision: 2,
    command_sequence: 3,
    discard: {
      ...session.discard,
      take: { generation: 3, start_sequence: 3, id: 'new' },
    },
  });
  reject(new Error('old discard failed'));
  await Promise.resolve();
  await Promise.resolve();
  expect(app.data.result).toBe('');
  expect(mocks.sound).not.toHaveBeenCalled();
  app.dispose();
});
