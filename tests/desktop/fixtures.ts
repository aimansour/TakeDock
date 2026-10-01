import {
  copyFileSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  renameSync,
  existsSync,
} from 'node:fs';
import { join } from 'node:path';
import { randomUUID, createHash } from 'node:crypto';
import { browser, $ } from '@wdio/globals';

export let fixtureRoot = '';
export const settingsPath = process.env.TAKEDOCK_SETTINGS_FILE!;
const base = process.env.TAKEDOCK_TEST_ROOT!;
function atom(kind: string, payload: Buffer) {
  const header = Buffer.alloc(8);
  header.writeUInt32BE(payload.length + 8);
  header.write(kind, 4);
  return Buffer.concat([header, payload]);
}
function video() {
  const duration = Buffer.alloc(20);
  duration.writeUInt32BE(1000, 12);
  duration.writeUInt32BE(100, 16);
  return Buffer.concat([
    atom('ftyp', Buffer.from('isom0000')),
    atom('mdat', Buffer.alloc(262144, 123)),
    atom('moov', atom('mvhd', duration)),
  ]);
}
export function prepareFixture(connected = false) {
  fixtureRoot = join(base, randomUUID());
  mkdirSync(fixtureRoot, { recursive: true });
  mkdirSync(join(fixtureRoot, 'videos'));
  mkdirSync(join(fixtureRoot, 'destination'));
  copyFileSync(process.env.TAKEDOCK_FIXTURE_EXE!, join(fixtureRoot, 'adb.exe'));
  writeFileSync(
    join(fixtureRoot, 'devices.txt'),
    connected ? 'fixture-one\tdevice\n' : '',
  );
  writeFileSync(
    join(fixtureRoot, 'state.json'),
    JSON.stringify({ state: 'idle', foreground: true, video_mode: true }),
  );
  for (const name of ["a'b.mp4", 'رحلة.mp4'])
    writeFileSync(join(fixtureRoot, 'videos', name), video());
  writeFileSync(
    settingsPath,
    JSON.stringify({
      language: 'en',
      destination: join(fixtureRoot, 'destination'),
      adb_path: join(fixtureRoot, 'adb.exe'),
      success_sound: false,
      verification_ms: 120000,
    }),
  );
}
export async function restartFixture(connected: boolean) {
  prepareFixture(connected);
  await browser.reloadSession();
  await $('h1').waitForExist();
}
export const button = (name: string) =>
  $(`//button[normalize-space(.)=${JSON.stringify(name)}]`);
export async function shortcut(key: string) {
  await browser.keys(['Control', 'Shift', key, 'NULL']);
}
export async function waitFor(predicate: () => boolean | Promise<boolean>) {
  await browser.waitUntil(predicate, {
    timeout: 15000,
    interval: 50,
    timeoutMsg: 'Fixture condition was not reached',
  });
}
export function commands() {
  try {
    return readFileSync(join(fixtureRoot, 'commands.jsonl'), 'utf8')
      .trim()
      .split('\n')
      .filter(Boolean)
      .map((line) => JSON.parse(line));
  } catch {
    return [];
  }
}
export function flag(name: string) {
  writeFileSync(join(fixtureRoot, name), '');
}
export function inject(event: object) {
  writeFileSync(join(fixtureRoot, 'inject-event.tmp'), JSON.stringify(event));
  renameSync(
    join(fixtureRoot, 'inject-event.tmp'),
    join(fixtureRoot, 'inject-event.json'),
  );
}
export function hash(path: string) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}
export { join, existsSync, readFileSync };
