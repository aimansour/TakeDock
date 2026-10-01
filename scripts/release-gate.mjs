import { fileURLToPath } from 'node:url';
export function releaseGate(results) {
  return ['rust', 'ui', 'observer', 'desktop'].every(
    (name) => results[name] === 'success',
  );
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const names = ['rust', 'ui', 'observer', 'desktop'];
  const results = Object.fromEntries(
    names.map((name, index) => [name, process.argv[index + 2]]),
  );
  if (!releaseGate(results)) {
    console.error('Required validation gates did not all succeed.');
    process.exitCode = 1;
  }
}
