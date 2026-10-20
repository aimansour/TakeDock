export type RecordingState = 'unknown' | 'idle' | 'recording' | 'paused';
export type RecordingAction = 'start' | 'stop' | 'pause' | 'resume';
export type SpeechReadiness = 'inactive' | 'preparing' | 'ready';
export interface TakeToken {
  generation: number;
  start_sequence: number;
  id: string;
}
export interface DiscardSnapshot {
  take: TakeToken | null;
  eligible: boolean;
  reason: string;
  operation_id: string | null;
  phase:
    | 'unavailable'
    | 'tracking'
    | 'waiting_to_stop'
    | 'confirming_stop'
    | 'finalizing'
    | 'deleting'
    | 'completed'
    | 'retained'
    | 'outcome_unknown';
}
export interface DiscardReceipt {
  generation: number;
  take: TakeToken;
  operation_id: string;
}
export interface SessionState {
  discard: DiscardSnapshot;
  speech_readiness: SpeechReadiness;
  generation: number;
  revision: number;
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
  ready_sound: boolean;
  ready_delay_ms: number;
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
  discard_operation_id?: string | null;
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
  discard: {
    take: null,
    eligible: false,
    reason: 'discard_no_managed_take',
    operation_id: null,
    phase: 'unavailable',
  },
  speech_readiness: 'inactive',
  generation: 0,
  revision: 0,
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
  ready_sound: true,
  ready_delay_ms: 800,
  verification_ms: 10000,
};
