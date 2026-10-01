export type RecordingState = 'unknown' | 'idle' | 'recording' | 'paused';
export type RecordingAction = 'start' | 'stop' | 'pause' | 'resume';
export interface SessionState {
  generation: number;
  connected: boolean;
  observer_ready: boolean;
  foreground: boolean;
  video_mode: boolean;
  predicted: RecordingState;
  observed: RecordingState;
  command_sequence: number;
  pending: number;
  condition: string;
}
export interface CommandReceipt {
  generation: number;
  sequence: number;
  predicted: RecordingState;
}
export interface Settings {
  language: 'en' | 'ar';
  destination: string;
  adb_path: string;
  success_sound: boolean;
  verification_ms: number;
}
export interface Video {
  name: string;
  size: number;
  modified_ms: number;
  ready: boolean;
  identity: string;
  duration_ms?: number | null;
}
export type JobKind = 'copy' | 'move' | 'delete' | 'rename';
export interface JobEvent {
  id: string;
  generation: number;
  kind: JobKind;
  status: string;
  phase?: string;
  name: string;
  bytes: number;
  total: number;
  completed: number;
  count: number;
  message: string;
  results: { name: string; status: string; message: string }[];
}
export const initialSession: SessionState = {
  generation: 0,
  connected: false,
  observer_ready: false,
  foreground: false,
  video_mode: false,
  predicted: 'unknown',
  observed: 'unknown',
  command_sequence: 0,
  pending: 0,
  condition: 'disconnected',
};
export const defaultSettings: Settings = {
  language: 'en',
  destination: '',
  adb_path: '',
  success_sound: false,
  verification_ms: 10000,
};
