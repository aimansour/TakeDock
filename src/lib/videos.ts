import type { Video } from './types';
export function videoKey(video: Video): string {
  const device = video.identity.match(/"device":(\d+)/)?.[1];
  const inode = video.identity.match(/"inode":(\d+)/)?.[1];
  return device && inode ? `${device}:${inode}` : video.identity;
}
