import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';
import type {
  SessionState,
  Settings,
  Video,
  JobKind,
  CommandReceipt,
} from './types';
export const ipc = {
  bootstrap: () =>
    invoke<{ session: SessionState; settings: Settings; errors: string[] }>(
      'bootstrap',
    ),
  recording: (action: string, generation: number) =>
    invoke<CommandReceipt>('recording_action', { action, generation }),
  videos: (generation: number) =>
    invoke<Video[]>('list_videos', { generation }),
  job: (kind: JobKind, videos: Video[], generation: number, newStem?: string) =>
    invoke<string>('start_file_job', {
      kind,
      videos,
      generation,
      newStem: newStem ?? null,
    }),
  cancel: (id: string) => invoke<void>('cancel_file_job', { id }),
  save: (settings: Settings) => invoke<Settings>('save_settings', { settings }),
  reconnect: () => invoke<void>('reconnect'),
  sound: (success: boolean) => invoke<void>('play_feedback', { success }),
  folder: () =>
    open({ directory: true, multiple: false }).then((path) =>
      typeof path === 'string' ? path : null,
    ),
  adb: () =>
    open({
      multiple: false,
      filters: [{ name: 'ADB executable', extensions: ['exe'] }],
    }).then((path) => (typeof path === 'string' ? path : null)),
  listen: <T>(event: string, callback: (value: T) => void) =>
    listen<T>(event, (event) => callback(event.payload)),
};
