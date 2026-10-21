import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import type { FileNotice, FileResultSnapshot } from './types';
import { FileResultsController, type FileResultView } from './file-results';

const success: FileNotice = {
  key: { generation: 7, owner: 'job', operation_id: 'copy-1' },
  revision: 1,
  kind: 'copy',
  outcome: 'completed',
  code: 'completed',
  warnings: [],
  name: 'رحلة.mp4',
  completed: 1,
  count: 1,
  results: [{ name: 'رحلة.mp4', status: 'completed', message: '' }],
  expires_at_ms: 11_000,
};
const snapshot = (
  revision = 1,
  now_ms = 1_000,
  notices = [success],
): FileResultSnapshot => ({ revision, now_ms, notices, history: [success] });
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}
function harness() {
  let now = 0;
  let view: FileResultView = { notices: [], history: [] };
  const read = vi
    .fn<() => Promise<FileResultSnapshot>>()
    .mockResolvedValue(snapshot());
  const acknowledge = vi
    .fn<
      (
        _key: FileNotice['key'],
        _revision: number,
      ) => Promise<FileResultSnapshot>
    >()
    .mockResolvedValue(snapshot(2, 1_000, []));
  const failed = vi.fn();
  const controller = new FileResultsController({
    read,
    acknowledge,
    now: () => now,
    schedule: (callback, delay) => setTimeout(callback, delay),
    cancel: clearTimeout,
    changed: (value) => {
      view = value;
    },
    failed,
  });
  return {
    controller,
    read,
    acknowledge,
    failed,
    view: () => view,
    setNow: (value: number) => {
      now = value;
    },
    advance: (value: number) => {
      const elapsed = value - now;
      now = value;
      vi.advanceTimersByTime(elapsed);
    },
  };
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

it('expiry_uses_original_deadline_despite_snapshot_latency', () => {
  const h = harness();
  h.setNow(400);
  h.controller.accept(snapshot(), 100);
  expect(h.view().notices).toHaveLength(1);
  h.advance(5_000);
  h.controller.accept(snapshot(), 5_000); // delayed duplicate cannot extend cutoff
  h.advance(10_099);
  expect(h.view().notices).toHaveLength(1);
  h.advance(10_100);
  expect(h.view().notices).toHaveLength(0);
  expect(h.view().history).toHaveLength(1);
  h.controller.accept(snapshot(), 10_100);
  expect(h.view().notices).toHaveLength(0);
  h.controller.dispose();
});

it('older_snapshot_cannot_restore_acknowledged_notice', async () => {
  const h = harness();
  h.controller.accept(snapshot(), 0);
  await h.controller.hide(success);
  h.controller.accept(snapshot(), 0);
  expect(h.view().notices).toHaveLength(0);
  expect(h.view().history).toHaveLength(1);
  h.controller.dispose();
});

it('warning_revision_survives_old_hide_response', async () => {
  const h = harness();
  const reply = deferred<FileResultSnapshot>();
  h.acknowledge.mockReturnValue(reply.promise);
  h.controller.accept(snapshot(), 0);
  const hiding = h.controller.hide(success);
  const warned: FileNotice = {
    ...success,
    revision: 2,
    outcome: 'warning',
    warnings: ['media_index_pending'],
    expires_at_ms: null,
  };
  h.controller.accept(snapshot(3, 2_000, [warned]), 0);
  reply.resolve(snapshot(2, 1_000, []));
  await hiding;
  h.advance(50_000);
  expect(h.view().notices).toEqual([warned]);
  h.controller.dispose();
});

it('notification_during_read_triggers_fresh_projection', async () => {
  const h = harness();
  const old = deferred<FileResultSnapshot>();
  h.read
    .mockReturnValueOnce(old.promise)
    .mockResolvedValueOnce(snapshot(2, 2_000, []));
  const first = h.controller.refresh(1);
  const second = h.controller.refresh(2);
  old.resolve(snapshot());
  await Promise.all([first, second]);
  expect(h.view().notices).toHaveLength(0);
  expect(h.read).toHaveBeenCalledTimes(2); // both revisions must be projected
  h.controller.dispose();
});

it('late_new_window_does_not_replay_success', () => {
  const first = harness();
  first.controller.accept(snapshot(), 0);
  first.advance(10_000);
  const second = harness();
  second.controller.accept(snapshot(1, 11_000, []), 0);
  expect(first.view().notices).toHaveLength(0);
  expect(second.view().notices).toHaveLength(0);
  expect(second.view().history).toHaveLength(1);
  first.controller.dispose();
  second.controller.dispose();
});

it('dispose_cancels_timers_and_pending_reads', async () => {
  const h = harness();
  h.controller.accept(snapshot(), 0);
  expect(h.view().notices).toHaveLength(1);
  expect(vi.getTimerCount()).toBe(1);
  const pending = deferred<FileResultSnapshot>();
  h.read.mockReturnValue(pending.promise);
  const reading = h.controller.refresh(2);
  h.controller.dispose();
  expect(vi.getTimerCount()).toBe(0);
  const last = h.view();
  pending.resolve(snapshot(2, 2_000, []));
  await reading;
  h.advance(30_000);
  expect(h.view()).toBe(last);
});

it('failed_hide_keeps_the_notice_and_reports_once', async () => {
  const h = harness();
  h.controller.accept(snapshot(), 0);
  h.acknowledge.mockRejectedValue(new Error('transport failed'));
  await expect(h.controller.hide(success)).rejects.toThrow('transport failed');
  expect(h.view().notices).toEqual([success]);
  expect(h.failed).toHaveBeenCalledTimes(1);
  h.controller.dispose();
});
