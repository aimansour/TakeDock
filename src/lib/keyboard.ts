export function shortcut(
  event: KeyboardEvent,
): 'record' | 'pause' | 'videos' | 'settings' | null {
  const target = event.target;
  if (
    event.repeat ||
    event.isComposing ||
    !event.ctrlKey ||
    !event.shiftKey ||
    event.altKey ||
    event.metaKey
  )
    return null;
  if (
    target instanceof HTMLElement &&
    (target.matches('input,textarea,select') || target.isContentEditable)
  )
    return null;
  return (
    ({ r: 'record', p: 'pause', v: 'videos', s: 'settings' } as const)[
      event.key.toLowerCase() as 'r' | 'p' | 'v' | 's'
    ] ?? null
  );
}
