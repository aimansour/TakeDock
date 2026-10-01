import { describe, it, expect, vi } from 'vitest';
import cases from '../../tests/protocol/recording-transitions.json';
import { RecordingController, predictTransition } from './recording';
import { initialSession, defaultSettings } from './types';
import type { RecordingState, RecordingAction, CommandReceipt } from './types';
const ready = {
  ...initialSession,
  generation: 7,
  connected: true,
  observer_ready: true,
  foreground: true,
  video_mode: true,
  predicted: 'idle' as const,
  observed: 'idle' as const,
  condition: 'ready',
};
describe('recording control', () => {
  it('recovers from rejected IPC without an invented accepted sequence', async () => {
    const sent = vi.fn().mockRejectedValue(new Error('camera_not_eligible'));
    const controller = new RecordingController(ready, sent, vi.fn(), vi.fn());
    controller.activate('start');
    await Promise.resolve();
    controller.reconcile({ ...ready });
    expect(controller.state.predicted).toBe('idle');
    expect(controller.state.command_sequence).toBe(0);
    expect(controller.state.pending).toBe(0);
    controller.activate('start');
    expect(sent).toHaveBeenCalledTimes(2);
  });
  it('does not erase a newer accepted receipt when an older request rejects', async () => {
    let rejectFirst!: (error: Error) => void;
    const sent = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise<CommandReceipt>((_, reject) => {
            rejectFirst = reject;
          }),
      )
      .mockResolvedValueOnce({
        generation: 7,
        sequence: 1,
        predicted: 'paused',
      });
    const controller = new RecordingController(ready, sent, vi.fn(), vi.fn());
    controller.activate('start');
    controller.activate('pause');
    await Promise.resolve();
    rejectFirst(new Error('queue_unavailable'));
    await Promise.resolve();
    controller.reconcile({
      ...ready,
      command_sequence: 1,
      predicted: 'paused',
      pending: 1,
    });
    expect(controller.state.predicted).toBe('paused');
    expect(controller.state.command_sequence).toBe(1);
  });
  it('matches the literal Rust protocol cases', () => {
    for (const item of cases) {
      expect(
        predictTransition(
          item.state as RecordingState,
          item.action as RecordingAction,
        ),
      ).toBe(item.expected);
    }
  });
  it('dispatches each action immediately while prior IPC and verification are held', () => {
    const sent = vi.fn(() => new Promise<CommandReceipt>(() => {}));
    const changed = vi.fn();
    const controller = new RecordingController(ready, sent, changed, vi.fn());
    controller.activate('start');
    expect(controller.state.predicted).toBe('recording');
    controller.activate('pause');
    controller.activate('resume');
    controller.activate('stop');
    expect(sent.mock.calls.length).toBe(4);
    expect(controller.state.predicted).toBe('idle');
    controller.reconcile({
      ...ready,
      command_sequence: 1,
      predicted: 'recording',
      observed: 'recording',
    });
    expect(controller.state.predicted).toBe('idle');
    controller.reconcile({ ...ready, generation: 6, predicted: 'paused' });
    expect(controller.state.generation).toBe(7);
  });
  it('requires eligibility and defaults to silent success', () => {
    const sent = vi.fn();
    new RecordingController(initialSession, sent, vi.fn(), vi.fn()).activate(
      'start',
    );
    expect(sent).not.toHaveBeenCalled();
    expect(defaultSettings.success_sound).toBe(false);
    expect(() => predictTransition('idle', 'pause')).toThrow();
  });
});
