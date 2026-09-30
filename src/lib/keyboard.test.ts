import { it, expect } from 'vitest';
import { shortcut } from './keyboard';
it('ignores repeated keys, composing, and typing contexts', () => {
  const event = new KeyboardEvent('keydown', {
    key: 'r',
    ctrlKey: true,
    shiftKey: true,
  });
  expect(shortcut(event)).toBe('record');
  expect(
    shortcut(
      new KeyboardEvent('keydown', {
        key: 'r',
        ctrlKey: true,
        shiftKey: true,
        repeat: true,
      }),
    ),
  ).toBeNull();
  const input = document.createElement('input');
  document.body.append(input);
  let result: unknown = 'unset';
  input.addEventListener('keydown', (event) => (result = shortcut(event)));
  input.dispatchEvent(event);
  expect(result).toBeNull();
  input.remove();
});
