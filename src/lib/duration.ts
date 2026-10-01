import { t } from './i18n';
export function duration(
  milliseconds: number | null | undefined,
  language: 'en' | 'ar',
): string {
  if (
    milliseconds == null ||
    !Number.isSafeInteger(milliseconds) ||
    milliseconds < 0
  )
    return t(language, 'durationUnavailable');
  let remaining = milliseconds;
  const parts: string[] = [];
  for (const [unit, size] of [
    ['hours', 3600000],
    ['minutes', 60000],
    ['seconds', 1000],
    ['milliseconds', 1],
  ] as const) {
    const count = Math.floor(remaining / size);
    remaining %= size;
    if (count > 0 || (unit === 'seconds' && milliseconds === 0))
      parts.push(`${count.toLocaleString(language)} ${t(language, unit)}`);
  }
  return parts.join(language === 'ar' ? '، ' : ', ');
}
