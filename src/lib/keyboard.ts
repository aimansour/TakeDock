export function shortcut(
  event: KeyboardEvent,
): 'record' | 'pause' | 'videos' | 'settings' | 'newWindow' | null {
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
  const action =
    (
      {
        r: 'record',
        p: 'pause',
        v: 'videos',
        s: 'settings',
        n: 'newWindow',
      } as const
    )[
      (event.code.startsWith('Key')
        ? event.code.slice(3)
        : event.key
      ).toLowerCase() as 'r' | 'p' | 'v' | 's' | 'n'
    ] ?? null;
  if (
    action !== 'newWindow' &&
    target instanceof HTMLElement &&
    (target.matches('input,textarea,select') || target.isContentEditable)
  )
    return null;
  return action;
}
