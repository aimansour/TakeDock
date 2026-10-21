import type { RenameUpdate, Video } from './types';
function identityKey(identity: string): string | null {
  // Validate the opaque Rust wire format without rounding its integer fields.
  const match = identity.match(
    /^\{"device":(\d+),"inode":(\d+),"size":\d+,"modified_ns":\d+,"changed_ns":\d+\}$/,
  );
  return match ? `${match[1]}:${match[2]}` : null;
}
function sourceKey(name: string, identity: string): string {
  return JSON.stringify([name, identity]);
}
function validName(name: string): boolean {
  return (
    name.length > 0 &&
    name !== '.' &&
    name !== '..' &&
    !/[\/\\\u0000]/.test(name)
  );
}
export class RenameUpdates {
  private generation = 0;
  private readonly links = new Map<string, RenameUpdate>();
  private readonly seen = new Set<string>();
  reset(generation: number): void {
    this.generation = generation;
    this.links.clear();
    this.seen.clear();
  }
  record(update: RenameUpdate): boolean {
    const video = update.video;
    const source = identityKey(update.source_identity);
    if (
      update.generation !== this.generation ||
      this.seen.has(update.id) ||
      !update.id ||
      !Number.isSafeInteger(update.sequence) ||
      update.sequence <= 0 ||
      !validName(update.source_name) ||
      !validName(video.name) ||
      !source ||
      source !== identityKey(video.identity) ||
      typeof video.ready !== 'boolean' ||
      !Number.isFinite(video.size) ||
      video.size < 0 ||
      !Number.isFinite(video.modified_ms) ||
      video.modified_ms < 0 ||
      (video.duration_ms != null &&
        (!Number.isFinite(video.duration_ms) || video.duration_ms < 0))
    )
      return false;
    this.seen.add(update.id);
    const key = sourceKey(update.source_name, update.source_identity);
    const previous = this.links.get(key);
    if (!previous || previous.sequence < update.sequence)
      this.links.set(key, update);
    return true;
  }
  apply(videos: Video[], generation: number): Video[] {
    if (generation !== this.generation) return videos;
    let changed = false;
    const next = videos.map((original) => {
      let video = original;
      let sequence = 0;
      for (;;) {
        const link = this.links.get(sourceKey(video.name, video.identity));
        if (!link || link.sequence <= sequence) break;
        sequence = link.sequence;
        video = link.video;
      }
      changed ||= video !== original;
      return video;
    });
    return changed ? next : videos;
  }
}
