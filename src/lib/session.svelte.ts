import {
  initialSession,
  defaultSettings,
  withoutDesktopDestination,
} from './types';
import type {
  JobEvent,
  Video,
  Settings,
  RecordingAction,
  SessionState,
  FileNotice,
  RenameUpdate,
  HandoffUpdate,
} from './types';
import { RenameUpdates } from './rename-updates';
import { HandoffUpdates } from './handoff-updates';
import { videoKey } from './videos';
import { FileResultsController, type FileResultView } from './file-results';
import { RecordingController } from './recording';
import { ipc } from './ipc';
import { UpdaterController } from './updater';
import type { UpdateState } from './updater';
function retainJobs(jobs: JobEvent[]): JobEvent[] {
  let terminal = 0;
  return jobs.filter(
    (job) => ['queued', 'running'].includes(job.status) || ++terminal <= 20,
  );
}
export function createSession() {
  const data = $state({
    session: { ...initialSession },
    settings: { ...defaultSettings },
    windowName: '',
    videos: [] as Video[],
    pendingKeys: [] as string[],
    listingGeneration: 0,
    jobs: [] as JobEvent[],
    fileResults: { notices: [], history: [] } as FileResultView,
    result: '',
    loading: false,
    view: 'recording' as 'recording' | 'videos' | 'settings',
    update: {
      status: 'idle',
      info: null,
      message: '',
      bytes: 0,
      total: 0,
    } as UpdateState,
  });
  let destroyed = false;
  let refreshSerial = 0;
  let settingsRevision = 0;
  let flush: ReturnType<typeof setTimeout> | undefined;
  let unlisten: (() => void)[] = [];
  const progress = new Map<string, JobEvent>();
  const terminalJobs = new Set<string>();
  const discardResults = new Set<string>();
  const renameUpdates = new RenameUpdates();
  const handoffUpdates = new HandoffUpdates();
  const pending = new Map<
    number,
    {
      generation: number;
      kind: string;
      sources: Video[];
      id?: string;
    }
  >();
  let nextRequest = 0;
  function syncPending() {
    data.pendingKeys = [
      ...new Set(
        [...pending.values()].flatMap((request) =>
          request.sources.map(videoKey),
        ),
      ),
    ];
  }
  function releaseJob(id: string) {
    for (const [key, request] of pending)
      if (request.id === id) pending.delete(key);
    syncPending();
  }
  function applyUpdates(videos: Video[], generation: number) {
    return handoffUpdates.apply(
      renameUpdates.apply(videos, generation),
      generation,
    );
  }
  const sound = (success: boolean) => {
    if (!success || data.settings.success_sound)
      void ipc.sound(success).catch(() => {});
  };
  const fail = (message: string) => {
    data.result = message;
    sound(false);
  };
  const fileResults = new FileResultsController({
    read: ipc.fileResults,
    acknowledge: ipc.acknowledgeFileResult,
    now: () => performance.now(),
    schedule: (callback, delay) => setTimeout(callback, delay),
    cancel: (timer) => clearTimeout(timer),
    changed: (view) => {
      if (!destroyed) data.fileResults = view;
    },
    failed: (error) => {
      if (!destroyed) fail(String(error));
    },
  });
  const controller = new RecordingController(
    initialSession,
    ipc.recording,
    (session) => {
      if (
        session.generation !== data.session.generation ||
        !session.connected
      ) {
        refreshSerial++;
        renameUpdates.reset(session.generation);
        handoffUpdates.reset(session.generation);
        pending.clear();
        syncPending();
        data.videos = [];
        data.listingGeneration = 0;
        data.jobs = [];
        data.loading = false;
        progress.clear();
        terminalJobs.clear();
        discardResults.clear();
        if (flush) clearTimeout(flush);
        flush = undefined;
      }
      data.session = session;
    },
    fail,
  );
  const updater = new UpdaterController(
    { check: ipc.checkUpdate, install: ipc.installUpdate },
    (update) => {
      if (!destroyed) data.update = update;
    },
    (message) => {
      if (!destroyed) fail(message);
    },
  );
  function applyJob(event: JobEvent) {
    if (event.generation !== data.session.generation) return;
    const terminal = ['completed', 'failed', 'cancelled'].includes(
      event.status,
    );
    if (!terminal && terminalJobs.has(event.id)) return;
    if (terminal) {
      terminalJobs.add(event.id);
      releaseJob(event.id);
    }
    const index = data.jobs.findIndex((job) => job.id === event.id);
    if (index >= 0) {
      const previous = data.jobs[index].status;
      if (
        (event.status === 'queued' && previous !== 'queued') ||
        (['completed', 'failed', 'cancelled'].includes(previous) &&
          ['queued', 'running'].includes(event.status))
      )
        return;
    }
    const jobs = [...data.jobs];
    if (index >= 0) jobs[index] = event;
    else jobs.unshift(event);
    data.jobs = retainJobs(jobs);
    if (['completed', 'failed', 'cancelled'].includes(event.status)) {
      progress.delete(event.id);
      if (event.discard_operation_id) return;
      void refresh();
    }
  }
  async function initialize() {
    try {
      const outcomes = await Promise.allSettled([
        ipc.listen<HandoffUpdate>('file-handed-off', (update) => {
          if (
            destroyed ||
            !data.session.connected ||
            !handoffUpdates.record(update)
          )
            return;
          refreshSerial++;
          data.loading = false;
          data.videos = applyUpdates(data.videos, data.session.generation);
          for (const [key, request] of pending) {
            if (
              request.generation !== update.generation ||
              request.kind !== 'mark'
            )
              continue;
            request.sources = request.sources.filter(
              (source) =>
                source.name !== update.source_name ||
                source.identity !== update.source_identity,
            );
            if (!request.sources.length) pending.delete(key);
          }
          syncPending();
        }),
        ipc.listen<RenameUpdate>('file-renamed', (update) => {
          if (
            destroyed ||
            !data.session.connected ||
            !renameUpdates.record(update)
          )
            return;
          refreshSerial++;
          data.loading = false;
          data.videos = applyUpdates(data.videos, data.session.generation);
        }),
        ipc.listen<{ revision: number }>('file-results-changed', (value) => {
          if (!destroyed) void fileResults.refresh(value.revision);
        }),
        ipc.listen<Settings>('settings-changed', (settings) => {
          if (!destroyed) {
            settingsRevision++;
            data.settings = withoutDesktopDestination(settings);
          }
        }),
        ipc.listen<{ bytes: number; total: number }>(
          'update-progress',
          (value) => {
            if (!destroyed) updater.progress(value.bytes, value.total);
          },
        ),
        ipc.listen<SessionState>('session-state', (value) => {
          if (!destroyed) controller.reconcile(value);
        }),
        ipc.listen<{
          generation: number;
          operation_id: string;
          status: string;
          code: string;
        }>('discard-result', (value) => {
          if (
            destroyed ||
            value.generation !== data.session.generation ||
            discardResults.has(value.operation_id) ||
            !['completed', 'retained', 'outcome_unknown'].includes(value.status)
          )
            return;
          discardResults.add(value.operation_id);
          if (discardResults.size > 128)
            discardResults.delete(discardResults.values().next().value!);
          if (value.status === 'completed') void refresh();
        }),
        ipc.listen<{
          generation: number;
          sequence: number;
          status: string;
          message: string;
        }>('verification-result', (value) => {
          if (
            destroyed ||
            value.generation !== data.session.generation ||
            value.status === 'superseded'
          )
            return;
          if (value.status !== 'confirmed')
            data.result = value.message || 'verification_unconfirmed';
        }),
        ipc.listen<{ generation: number; code: string }>(
          'operation-error',
          (value) => {
            if (!destroyed && value.generation === data.session.generation)
              data.result = value.code;
          },
        ),
        ipc.listen<JobEvent>('file-job', (event) => {
          if (destroyed) return;
          if (event.status !== 'running') {
            applyJob(event);
            return;
          }
          progress.set(event.id, event);
          if (!flush)
            flush = setTimeout(() => {
              flush = undefined;
              for (const event of progress.values()) applyJob(event);
              progress.clear();
            }, 100);
        }),
      ]);
      const subscriptions = outcomes.flatMap((outcome) =>
        outcome.status === 'fulfilled' ? [outcome.value] : [],
      );
      if (destroyed) {
        subscriptions.forEach((stop) => stop());
        return;
      }
      unlisten = subscriptions;
      const rejected = outcomes.find(
        (outcome) => outcome.status === 'rejected',
      );
      if (rejected?.status === 'rejected') {
        unlisten.forEach((stop) => stop());
        unlisten = [];
        throw rejected.reason;
      }
      const revision = settingsRevision;
      const requestedAt = performance.now();
      const initial = await ipc.bootstrap();
      if (destroyed) return;
      if (settingsRevision === revision)
        data.settings = withoutDesktopDestination(initial.settings);
      data.windowName = initial.window_name;
      controller.reconcile(initial.session);
      fileResults.accept(initial.file_results, requestedAt);
      for (const job of initial.jobs) {
        if (
          job.generation === data.session.generation &&
          ['completed', 'failed', 'cancelled'].includes(job.status)
        )
          terminalJobs.add(job.id);
      }
      const currentJobs = data.jobs.filter(
        (job) =>
          !['queued', 'running'].includes(job.status) ||
          !terminalJobs.has(job.id),
      );
      const existing = new Set(currentJobs.map((job) => job.id));
      data.jobs = retainJobs([
        ...currentJobs,
        ...initial.jobs
          .filter(
            (job) =>
              !existing.has(job.id) &&
              job.generation === data.session.generation,
          )
          .filter(
            (job) =>
              !['queued', 'running'].includes(job.status) ||
              !terminalJobs.has(job.id),
          ),
      ]);
      if (initial.errors.length) data.result = initial.errors.join('\n');
      if (initial.check_at_startup) void updater.checkForUpdate('startup');
    } catch (error) {
      if (!destroyed) fail(String(error));
    }
  }
  async function refresh() {
    if (!data.session.connected) return;
    const request = ++refreshSerial;
    const generation = data.session.generation;
    data.loading = true;
    try {
      const videos = await ipc.videos(generation);
      if (
        !destroyed &&
        request === refreshSerial &&
        generation === data.session.generation
      ) {
        data.videos = applyUpdates(videos, generation);
        data.listingGeneration = generation;
        handoffUpdates.settle();
      }
    } catch (error) {
      if (
        !destroyed &&
        request === refreshSerial &&
        generation === data.session.generation
      )
        fail(String(error));
    } finally {
      if (request === refreshSerial) data.loading = false;
    }
  }
  async function save(settings: Settings) {
    try {
      const revision = settingsRevision;
      const saved = await ipc.save(withoutDesktopDestination(settings));
      if (destroyed) return;
      // Rust serializes persistence and broadcasts under the shared lock.
      // A response cannot replace a newer authoritative broadcast.
      if (revision === settingsRevision)
        data.settings = withoutDesktopDestination(saved);
      data.result = 'saved';
      sound(true);
    } catch (error) {
      fail(String(error));
    }
  }
  function activate(action: RecordingAction) {
    controller.activate(action);
  }
  function discard() {
    const generation = data.session.generation;
    const current = data.session.discard.take;
    if (
      !data.session.connected ||
      !current ||
      current.generation !== generation
    )
      return;
    const take = { ...current };
    const rejected = (error: unknown) => {
      if (
        !destroyed &&
        data.session.generation === generation &&
        data.session.discard.take?.id === take.id &&
        data.session.discard.take.start_sequence === take.start_sequence
      )
        fail(String(error));
    };
    try {
      void ipc.discard(take, generation).catch(rejected);
    } catch (error) {
      rejected(error);
    }
  }
  function dispose() {
    destroyed = true;
    fileResults.dispose();
    refreshSerial++;
    unlisten.forEach((stop) => stop());
    unlisten = [];
    if (flush) clearTimeout(flush);
    progress.clear();
    pending.clear();
    syncPending();
  }
  return {
    data,
    initialize,
    refresh,
    save,
    activate,
    discard,
    fail,
    dispose,
    hideFileResult: (notice: FileNotice) => fileResults.hide(notice),
    newWindow: () => {
      void ipc.newWindow().catch((error) => fail(String(error)));
    },
    renameWindow: (name: string) => {
      void ipc
        .renameWindow(name)
        .then((value) => {
          data.windowName = value;
          data.result = 'window_renamed';
        })
        .catch((error) => fail(String(error)));
    },
    checkUpdates: () => {
      void updater.checkForUpdate('manual');
    },
    installUpdate: () => {
      void updater.installUpdate();
    },
    job: (
      kind: Parameters<typeof ipc.job>[0],
      videos: Video[],
      stem?: string,
      listingGeneration = data.listingGeneration,
    ) => {
      const generation = data.session.generation;
      if (!['mark', 'delete', 'rename'].includes(kind)) {
        fail('invalid_file_job');
        return;
      }
      if (!data.session.connected || listingGeneration !== generation) {
        fail('session_replaced');
        return;
      }
      if (!videos.length || videos.some((video) => !video.ready)) {
        fail('file_not_ready');
        return;
      }
      if (videos.some((video) => data.pendingKeys.includes(videoKey(video)))) {
        fail('file_busy');
        return;
      }
      const token = ++nextRequest;
      pending.set(token, { generation, kind, sources: [...videos] });
      syncPending();
      void ipc
        .job(kind, videos, generation, stem)
        .then((id) => {
          const request = pending.get(token);
          if (!request) return;
          request.id = id;
          if (terminalJobs.has(id)) releaseJob(id);
          if (
            !destroyed &&
            generation === data.session.generation &&
            !terminalJobs.has(id) &&
            !data.jobs.some((job) => job.id === id)
          )
            data.jobs = retainJobs([
              {
                id,
                generation,
                kind,
                status: 'queued',
                name: '',
                count: videos.length,
                completed: 0,
                message: '',
                results: [],
              },
              ...data.jobs,
            ]);
        })
        .catch((error) => {
          pending.delete(token);
          syncPending();
          if (!destroyed && generation === data.session.generation)
            fail(String(error));
        });
    },
    cancel: (id: string) => {
      void ipc.cancel(id).catch((error) => fail(String(error)));
    },
    reconnect: () => {
      data.result = '';
      void ipc.reconnect().catch((error) => fail(String(error)));
    },
  };
}
