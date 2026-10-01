import {
  appendFileSync,
  writeFileSync,
  readFileSync,
  existsSync,
} from 'node:fs';
import { join } from 'node:path';
import { browser } from '@wdio/globals';
import { prepareFixture } from './fixtures';
const results = join(process.env.TAKEDOCK_TEST_ROOT!, 'results.jsonl');
export const config = {
  hostname: '127.0.0.1',
  port: Number(process.env.TAKEDOCK_DRIVER_PORT),
  path: '/',
  specs: ['./critical.e2e.ts'],
  maxInstances: 1,
  logLevel: 'warn',
  capabilities: [
    { 'tauri:options': { application: process.env.TAKEDOCK_APP_EXE } },
  ],
  reporters: ['spec'],
  framework: 'mocha',
  mochaOpts: { ui: 'bdd', timeout: 90000 },
  connectionRetryCount: 0,
  connectionRetryTimeout: 30000,
  beforeSession: () => {
    writeFileSync(results, '');
    prepareFixture(false);
  },
  afterTest: async (
    test: { title: string },
    _context: unknown,
    result: { passed: boolean; error?: Error },
  ) => {
    appendFileSync(
      results,
      JSON.stringify({
        title: test.title,
        passed: result.passed,
        error: result.error?.message ?? '',
      }) + '\n',
    );
    if (!result.passed) {
      writeFileSync(
        join(process.env.TAKEDOCK_TEST_ROOT!, 'failure-dom.html'),
        await browser.getPageSource().catch(() => '<unavailable>'),
      );
      await browser
        .saveScreenshot(join(process.env.TAKEDOCK_TEST_ROOT!, 'failure.png'))
        .catch(() => {});
    }
  },
  onComplete: () => {
    const tests = existsSync(results)
      ? readFileSync(results, 'utf8')
          .trim()
          .split('\n')
          .filter(Boolean)
          .map((line) => JSON.parse(line))
      : [];
    if (tests.length !== 4 || tests.some((test) => !test.passed))
      throw new Error(
        `All four desktop cases are required: ${tests.filter((test) => test.passed).length}/4 passed.`,
      );
  },
};
