import { expect, it } from 'vitest';
import { HandoffUpdates } from './handoff-updates';
import { RenameUpdates } from './rename-updates';
import type { HandoffUpdate, Video } from './types';

const source: Video = {
  name: "رحلة '1.mp4",
  size: 10,
  modified_ms: 0,
  ready: true,
  identity:
    '{"device":9007199254740993,"inode":9007199254740995,"size":10,"modified_ns":1700000000123456789,"changed_ns":1700000000123456789}',
};
const moved: HandoffUpdate = {
  generation: 3,
  job_id: 'batch',
  operation_id: 'one',
  sequence: 2,
  source_name: source.name,
  source_identity: source.identity,
};
it('removes only the handed-off object and preserves a reused name with another or newer identity', () => {
  const updates = new HandoffUpdates();
  updates.reset(3);
  expect(updates.record(moved)).toBe(true);
  const replacement = {
    ...source,
    identity: source.identity.replace('9007199254740995', '9007199254740996'),
  };
  const reused = {
    ...source,
    identity: source.identity.replace(
      '"changed_ns":1700000000123456789',
      '"changed_ns":1700000000123456790',
    ),
  };
  expect(updates.apply([source, replacement, reused], 3)).toEqual([
    replacement,
    reused,
  ]);
});
it('rejects duplicate, malformed and older-generation receipts and accepts every file in a batch', () => {
  const updates = new HandoffUpdates();
  updates.reset(3);
  expect(updates.record({ ...moved, generation: 2 })).toBe(false);
  expect(updates.record({ ...moved, sequence: -1 })).toBe(false);
  expect(updates.record({ ...moved, source_identity: '{"device":1}' })).toBe(
    false,
  );
  expect(updates.record(moved)).toBe(true);
  expect(updates.record(moved)).toBe(false);
  const second = {
    ...source,
    name: 'two.mp4',
    identity: source.identity.replace('9007199254740995', '9007199254740997'),
  };
  expect(
    updates.record({
      ...moved,
      operation_id: 'two',
      sequence: 3,
      source_name: second.name,
      source_identity: second.identity,
    }),
  ).toBe(true);
  expect(updates.apply([source, second], 3)).toEqual([]);
  updates.reset(4);
  expect(updates.apply([source, second], 4)).toEqual([source, second]);
});
it('cannot resurrect an older version when a rename arrives after the handoff', () => {
  const updates = new HandoffUpdates();
  updates.reset(3);
  const renamed = {
    ...source,
    name: 'renamed.mp4',
    identity: source.identity.replace(
      '"changed_ns":1700000000123456789',
      '"changed_ns":1700000000123456790',
    ),
  };
  updates.record({
    ...moved,
    source_name: renamed.name,
    source_identity: renamed.identity,
  });
  expect(updates.apply([source], 3)).toEqual([]);
  const renames = new RenameUpdates();
  renames.reset(3);
  expect(
    renames.record({
      generation: 3,
      id: 'rename',
      sequence: 1,
      source_name: source.name,
      source_identity: source.identity,
      video: renamed,
    }),
  ).toBe(true);
  expect(updates.apply(renames.apply([source], 3), 3)).toEqual([]);
});
it('keeps receipt deduplication after a fresh source-listing barrier', () => {
  const updates = new HandoffUpdates();
  updates.reset(3);
  updates.record(moved);
  updates.settle();
  expect(updates.record(moved)).toBe(false);
  const current = {
    ...source,
    identity: source.identity.replace('9007199254740995', '9007199254740998'),
  };
  expect(updates.apply([current], 3)).toEqual([current]);
});
