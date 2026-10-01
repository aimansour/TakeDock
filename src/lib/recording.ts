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
  private backend: SessionState;
  private nextRequest = 0;
  private requests: { id: number; predicted: RecordingState; guess: number }[] =
    [];
  constructor(
    public state: SessionState,
    private dispatch: (
      action: RecordingAction,
      generation: number,
    ) => Promise<CommandReceipt>,
    private changed: (state: SessionState) => void,
    private failed: (message: string) => void,
  ) {
    this.backend = state;
  }
  private publish(): void {
    const newer = this.requests.filter(
      (request) => request.guess > this.backend.command_sequence,
    );
    const latest = newer.at(-1);
    this.state = {
      ...this.backend,
      predicted: latest?.predicted ?? this.backend.predicted,
      command_sequence: latest?.guess ?? this.backend.command_sequence,
      pending: this.backend.pending + newer.length,
    };
    this.changed(this.state);
  }
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
    const request = {
      id: ++this.nextRequest,
      predicted,
      guess: this.state.command_sequence + 1,
    };
    this.requests.push(request);
    this.publish();
    // Invoke from the activation itself: no effect, DOM tick, or previous promise.
    try {
      void this.dispatch(action, generation).then(
        (receipt) => this.accept(request.id, receipt),
        (error) => this.reject(request.id, generation, error),
      );
    } catch (error) {
      this.reject(request.id, generation, error);
    }
  }
  private accept(id: number, receipt: CommandReceipt): void {
    if (receipt.generation !== this.backend.generation) return;
    this.requests = this.requests.filter((request) => request.id !== id);
    if (receipt.sequence > this.backend.command_sequence) {
      this.backend = {
        ...this.backend,
        predicted: receipt.predicted,
        command_sequence: receipt.sequence,
        pending: this.backend.pending + 1,
      };
      let guess = receipt.sequence;
      for (const request of this.requests) {
        if (request.id > id) request.guess = Math.max(request.guess, ++guess);
      }
    }
    this.publish();
  }
  private reject(id: number, generation: number, error: unknown): void {
    if (generation !== this.state.generation) return;
    this.requests = this.requests.filter((request) => request.id !== id);
    let guess = this.backend.command_sequence;
    for (const request of this.requests) request.guess = ++guess;
    this.publish();
    this.failed(String(error));
  }
  reconcile(incoming: SessionState): void {
    if (incoming.generation < this.backend.generation) return;
    if (incoming.generation !== this.backend.generation) this.requests = [];
    else if (
      incoming.command_sequence < this.backend.command_sequence ||
      incoming.revision < this.backend.revision
    )
      return;
    this.backend = incoming;
    if (!incoming.connected || !incoming.observer_ready) this.requests = [];
    this.publish();
  }
}
