import { expect, it } from 'vitest';
import { RenameUpdates } from './rename-updates';
import type { RenameUpdate, Video } from './types';

const identity = (changed: number, inode = '9007199254740995') =>
  `{"device":9007199254740993,"inode":${inode},"size":10,"modified_ns":1000000,"changed_ns":${changed}}`;
const a: Video = {
  name: 'رحلة.mp4',
  identity: identity(1),
  size: 10,
  modified_ms: 1,
  ready: true,
  duration_ms: 100,
};
const b: Video = {
  ...a,
  name: 'renamed.mp4',
  identity: identity(2),
  duration_ms: 200,
};
const c: Video = {
  ...b,
  name: 'final.mp4',
  identity: identity(3),
  modified_ms: 2,
  duration_ms: 300,
  ready: false,
};
const update = (
  source: Video,
  video: Video,
  sequence: number,
): RenameUpdate => ({
  generation: 7,
  id: `rename-${sequence}`,
  sequence,
  source_name: source.name,
  source_identity: source.identity,
  video,
});

it('backwards_rename_updates_chain_without_rollback', () => {
  for (const reversed of [false, true]) {
    const reconciler = new RenameUpdates();
    reconciler.reset(7);
    const updates = [update(a, b, 1), update(b, c, 2)];
    if (reversed) updates.reverse();
    updates.forEach((value) => expect(reconciler.record(value)).toBe(true));
    expect(reconciler.apply([a], 7)).toEqual([c]);
    expect(reconciler.record(update(a, b, 1))).toBe(false);
    expect(reconciler.apply([c], 7)).toEqual([c]);
  }
});

it('replacement_or_generation_mismatch_is_untouched', () => {
  const reconciler = new RenameUpdates();
  reconciler.reset(7);
  reconciler.record(update(a, b, 1));
  const replacement = { ...a, identity: identity(8) };
  const rows = [replacement];
  expect(reconciler.apply(rows, 7)).toBe(rows);
  expect(reconciler.apply([a], 8)).toEqual([a]);
  expect(reconciler.record({ ...update(a, c, 2), generation: 8 })).toBe(false);
  reconciler.reset(8);
  expect(reconciler.apply([a], 8)).toEqual([a]);
});

it('orders same-identity cycles and terminates no-op links', () => {
  const sameB = { ...a, name: 'b.mp4' };
  const sameC = { ...a, name: 'c.mp4' };
  const reconciler = new RenameUpdates();
  reconciler.reset(7);
  [update(a, sameC, 3), update(sameB, a, 2), update(a, sameB, 1)].forEach(
    (value) => reconciler.record(value),
  );
  expect(reconciler.apply([a, sameB], 7)).toEqual([sameC, sameC]);
  const cycle = new RenameUpdates();
  cycle.reset(7);
  cycle.record(update(sameB, a, 2));
  cycle.record(update(a, sameB, 1));
  expect(cycle.apply([a, sameB], 7)).toEqual([a, a]);
  cycle.record(update(a, a, 3));
  expect(cycle.apply([a], 7)).toEqual([a]);
});

it('rejects malformed metadata, mismatched exact inode digits and unsafe ordering', () => {
  const reconciler = new RenameUpdates();
  reconciler.reset(7);
  for (const value of [
    update(a, { ...b, identity: identity(2, '9007199254740994') }, 1),
    update(
      a,
      { ...b, identity: '{"device":9007199254740993,"inode":9007199254740995' },
      2,
    ),
    update(a, { ...b, size: NaN }, 3),
    update(a, { ...b, name: '../bad.mp4' }, 4),
    update(a, b, 0),
    update(a, b, Number.MAX_SAFE_INTEGER + 1),
  ])
    expect(reconciler.record(value)).toBe(false);
  expect(reconciler.apply([a], 7)).toEqual([a]);
});
