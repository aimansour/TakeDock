import type { FileNotice, FileResultSnapshot, ResultKey } from './types';

export interface FileResultView {
  notices: FileNotice[];
  history: FileNotice[];
}
interface Dependencies {
  read: () => Promise<FileResultSnapshot>;
  acknowledge: (
    key: ResultKey,
    revision: number,
  ) => Promise<FileResultSnapshot>;
  now: () => number;
  schedule: (
    callback: () => void,
    delay: number,
  ) => ReturnType<typeof setTimeout>;
  cancel: (timer: ReturnType<typeof setTimeout>) => void;
  changed: (view: FileResultView) => void;
  failed: (error: unknown) => void;
}
export function resultKey(key: ResultKey): string {
  return JSON.stringify([key.generation, key.owner, key.operation_id]);
}
export class FileResultsController {
  private revision = -1;
  private targetRevision = -1;
  private requested = false;
  private disposed = false;
  private pending: Promise<void> | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private cutoffs = new Map<string, number>();
  private notices: FileNotice[] = [];
  private history: FileNotice[] = [];
  constructor(private dependencies: Dependencies) {}

  accept(snapshot: FileResultSnapshot, requestedAt: number): void {
    if (this.disposed || snapshot.revision < this.revision) return;
    const receivedAt = this.dependencies.now();
    const latency = Math.max(0, receivedAt - requestedAt);
    this.revision = snapshot.revision;
    for (const notice of snapshot.notices) {
      if (notice.expires_at_ms === null) continue;
      const key = resultKey(notice.key);
      const cutoff =
        receivedAt +
        Math.max(0, notice.expires_at_ms - snapshot.now_ms - latency);
      this.cutoffs.set(
        key,
        Math.min(cutoff, this.cutoffs.get(key) ?? Infinity),
      );
    }
    this.notices = snapshot.notices;
    this.history = snapshot.history;
    this.publish();
  }

  refresh(minRevision?: number): Promise<void> {
    if (this.disposed) return Promise.resolve();
    if (minRevision === undefined) this.requested = true;
    else this.targetRevision = Math.max(this.targetRevision, minRevision);
    if (
      !this.requested &&
      this.targetRevision <= this.revision &&
      !this.pending
    ) {
      return Promise.resolve();
    }
    if (!this.pending) {
      this.pending = this.drain().finally(() => {
        this.pending = undefined;
        if (
          !this.disposed &&
          (this.requested || this.targetRevision > this.revision)
        ) {
          return this.refresh(this.targetRevision);
        }
      });
    }
    return this.pending;
  }

  async hide(notice: FileNotice): Promise<void> {
    if (this.disposed) return;
    const started = this.dependencies.now();
    try {
      const snapshot = await this.dependencies.acknowledge(
        notice.key,
        notice.revision,
      );
      this.accept(snapshot, started);
    } catch (error) {
      if (!this.disposed) this.dependencies.failed(error);
      throw error;
    }
  }

  dispose(): void {
    this.disposed = true;
    if (this.timer !== undefined) this.dependencies.cancel(this.timer);
    this.timer = undefined;
    this.cutoffs.clear();
  }

  private async drain(): Promise<void> {
    do {
      this.requested = false;
      const started = this.dependencies.now();
      try {
        this.accept(await this.dependencies.read(), started);
      } catch (error) {
        if (!this.disposed) this.dependencies.failed(error);
        this.requested = false;
        this.targetRevision = this.revision;
        return;
      }
    } while (
      !this.disposed &&
      (this.requested || this.targetRevision > this.revision)
    );
  }

  private publish(): void {
    if (this.disposed) return;
    if (this.timer !== undefined) this.dependencies.cancel(this.timer);
    this.timer = undefined;
    const now = this.dependencies.now();
    const notices = this.notices.filter(
      (notice) =>
        notice.expires_at_ms === null ||
        (this.cutoffs.get(resultKey(notice.key)) ?? -Infinity) > now,
    );
    this.dependencies.changed({ notices, history: this.history });
    const next = Math.min(
      ...notices
        .filter((notice) => notice.expires_at_ms !== null)
        .map((notice) => this.cutoffs.get(resultKey(notice.key))!),
    );
    if (Number.isFinite(next)) {
      this.timer = this.dependencies.schedule(
        () => this.publish(),
        Math.max(0, next - now),
      );
    }
  }
}
