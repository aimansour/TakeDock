import { initialSession, defaultSettings } from './types';
import type {
  JobEvent,
  Video,
  Settings,
  RecordingAction,
  SessionState,
} from './types';
import { RecordingController } from './recording';
import { ipc } from './ipc';
import { UpdaterController } from './updater';
import type { UpdateState } from './updater';
export function createSession() {
  const data = $state({
    session: { ...initialSession },
    settings: { ...defaultSettings },
    videos: [] as Video[],
    jobs: [] as JobEvent[],
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
  let flush: ReturnType<typeof setTimeout> | undefined;
  let unlisten: (() => void)[] = [];
  const progress = new Map<string, JobEvent>();
  const sound = (success: boolean) => {
    if (!success || data.settings.success_sound)
      void ipc.sound(success).catch(() => {});
  };
  const fail = (message: string) => {
    data.result = message;
    sound(false);
  };
  const controller = new RecordingController(
    initialSession,
    ipc.recording,
    (session) => {
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
    const index = data.jobs.findIndex((job) => job.id === event.id);
    if (index >= 0) data.jobs[index] = event;
    else data.jobs = [event, ...data.jobs].slice(0, 20);
    if (['completed', 'failed', 'cancelled'].includes(event.status)) {
      progress.delete(event.id);
      data.result = event.status === 'completed' ? 'completed' : event.message;
      if (event.status !== 'cancelled') sound(event.status === 'completed');
      void refresh();
    }
  }
  async function initialize() {
    try {
      const outcomes = await Promise.allSettled([
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
          if (value.status === 'confirmed') {
            sound(true);
          } else {
            fail(value.message || 'verification_unconfirmed');
          }
        }),
        ipc.listen<{ generation: number; code: string }>(
          'operation-error',
          (value) => {
            if (!destroyed && value.generation === data.session.generation)
              fail(value.code);
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
        ipc.listen<{ generation: number; code: string }>(
          'file-index-warning',
          (value) => {
            if (!destroyed && value.generation === data.session.generation)
              data.result = value.code;
          },
        ),
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
      const initial = await ipc.bootstrap();
      if (destroyed) return;
      data.settings = initial.settings;
      controller.reconcile(initial.session);
      if (initial.errors.length) data.result = initial.errors.join('\n');
      void updater.checkForUpdate('startup');
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
      )
        data.videos = videos;
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
      data.settings = await ipc.save(settings);
      data.result = 'saved';
      sound(true);
    } catch (error) {
      fail(String(error));
    }
  }
  function activate(action: RecordingAction) {
    controller.activate(action);
  }
  function dispose() {
    destroyed = true;
    refreshSerial++;
    unlisten.forEach((stop) => stop());
    unlisten = [];
    if (flush) clearTimeout(flush);
    progress.clear();
  }
  return {
    data,
    initialize,
    refresh,
    save,
    activate,
    fail,
    dispose,
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
    ) => {
      const generation = data.session.generation;
      void ipc
        .job(kind, videos, generation, stem)
        .then((id) => {
          if (
            !destroyed &&
            generation === data.session.generation &&
            !data.jobs.some((job) => job.id === id)
          )
            data.jobs = [
              {
                id,
                generation,
                kind,
                status: 'queued',
                name: '',
                bytes: 0,
                total: videos.reduce((sum, video) => sum + video.size, 0),
                count: videos.length,
                completed: 0,
                message: '',
                results: [],
              },
              ...data.jobs,
            ].slice(0, 20);
        })
        .catch((error) => fail(String(error)));
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
