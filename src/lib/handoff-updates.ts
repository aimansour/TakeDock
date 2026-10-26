import type { HandoffUpdate, Video } from './types';
function identity(value: string): { key: string; changed: bigint } | null {
  if (typeof value !== 'string') return null;
  const match = value.match(
    /^\{"device":(\d+),"inode":(\d+),"size":\d+,"modified_ns":\d+,"changed_ns":(\d+)\}$/,
  );
  return match
    ? { key: `${match[1]}:${match[2]}`, changed: BigInt(match[3]) }
    : null;
}
export class HandoffUpdates {
  private generation = 0;
  private readonly seen = new Set<string>();
  private readonly removals = new Map<
    string,
    { sequence: number; changed: bigint }
  >();
  reset(generation: number): void {
    this.generation = generation;
    this.seen.clear();
    this.removals.clear();
  }
  record(update: HandoffUpdate): boolean {
    if (
      !update ||
      update.generation !== this.generation ||
      typeof update.job_id !== 'string' ||
      !update.job_id ||
      typeof update.operation_id !== 'string' ||
      !update.operation_id ||
      this.seen.has(update.operation_id) ||
      !Number.isSafeInteger(update.sequence) ||
      update.sequence <= 0 ||
      typeof update.source_name !== 'string' ||
      !update.source_name ||
      ['.', '..'].includes(update.source_name) ||
      /[\/\u0000]/.test(update.source_name) ||
      update.source_name.startsWith('.takedock-')
    )
      return false;
    const source = identity(update.source_identity);
    if (!source) return false;
    this.seen.add(update.operation_id);
    const previous = this.removals.get(source.key);
    if (previous && previous.sequence >= update.sequence) return false;
    this.removals.set(source.key, {
      sequence: update.sequence,
      changed:
        previous && previous.changed > source.changed
          ? previous.changed
          : source.changed,
    });
    return true;
  }
  apply(videos: Video[], generation: number): Video[] {
    if (generation !== this.generation) return videos;
    const current = videos.filter((video) => {
      const source = identity(video.identity);
      const removal = source && this.removals.get(source.key);
      return !source || !removal || source.changed > removal.changed;
    });
    return current.length === videos.length ? videos : current;
  }
  // Caller has accepted a listing started after the latest receipt; older
  // in-flight reads are fenced by session.refreshSerial before this barrier.
  settle(): void {
    this.removals.clear();
  }
}
