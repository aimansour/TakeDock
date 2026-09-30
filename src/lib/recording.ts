import type {
  RecordingAction,
  RecordingState,
  SessionState,
  CommandReceipt,
} from './types';
export function predictTransition(
  state: RecordingState,
  action: RecordingAction,
): RecordingState {
  if (
    (state === 'idle' && action === 'start') ||
    (state === 'paused' && action === 'resume')
  )
    return 'recording';
  if (state === 'recording' && action === 'pause') return 'paused';
  if ((state === 'recording' || state === 'paused') && action === 'stop')
    return 'idle';
  throw new Error('invalid_recording_transition');
}
export class RecordingController {
  constructor(
    public state: SessionState,
    private dispatch: (
      action: RecordingAction,
      generation: number,
    ) => Promise<CommandReceipt>,
    private changed: (state: SessionState) => void,
    private failed: (message: string) => void,
  ) {}
  activate(action: RecordingAction): void {
    if (
      !this.state.connected ||
      !this.state.observer_ready ||
      !this.state.foreground ||
      !this.state.video_mode
    )
      return;
    let predicted: RecordingState;
    try {
      predicted = predictTransition(this.state.predicted, action);
    } catch {
      return;
    }
    const generation = this.state.generation;
    this.state = {
      ...this.state,
      predicted,
      command_sequence: this.state.command_sequence + 1,
      pending: this.state.pending + 1,
    };
    this.changed(this.state);
    // Invoke from the activation itself: no effect, DOM tick, or previous promise.
    try {
      void this.dispatch(action, generation).catch((error) =>
        this.reject(generation, error),
      );
    } catch (error) {
      this.reject(generation, error);
    }
  }
  private reject(generation: number, error: unknown): void {
    if (generation !== this.state.generation) return;
    this.state = {
      ...this.state,
      predicted: 'unknown',
      condition: 'command_delivery_failed',
    };
    this.changed(this.state);
    this.failed(String(error));
  }
  reconcile(incoming: SessionState): void {
    if (incoming.generation < this.state.generation) return;
    if (
      incoming.generation === this.state.generation &&
      incoming.command_sequence < this.state.command_sequence
    ) {
      this.state = {
        ...incoming,
        predicted: this.state.predicted,
        command_sequence: this.state.command_sequence,
        pending: Math.max(incoming.pending, this.state.pending),
      };
    } else this.state = incoming;
    this.changed(this.state);
  }
}
