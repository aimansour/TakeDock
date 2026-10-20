# Desktop test dependency migration — 20 October 2026

## Problem and scope

Recording-readiness PR #16 could not pass the existing npm audit gate. The
unchanged dependency baseline reported 18 affected package entries from two
advisories: [basic-ftp](https://github.com/advisories/GHSA-c475-qrg2-pj4r) and
[braces](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm). The former entered
through the old browser manager and proxy chain; the latter through Mocha 10
and chokidar 3. These are desktop-test dependencies, not bundled phone code.

The approved bounded change pins the five direct WebdriverIO packages together
at 10.1.0 and regenerates the npm lockfile. Its compatible transitive refresh
also changes the generated frontend bundle, so the rebuilt application needs
release desktop acceptance in addition to the measured historical executable.
Application source, Rust dependencies, direct frontend versions and the
application's 0.1.2 version are unchanged by this migration.

## Compatibility decisions

- Node 24.21.0 satisfies WebdriverIO 10's Node floor. The
  [migration guide](https://webdriver.io/docs/v10-migration) was checked against
  the actual suite: it uses one Classic WebDriver session, exact selectors,
  awaited element lists, supported commands and Mocha BDD with a finite timeout.
  Strict selectors remain enabled; the suite is not opted back into v9 behavior.
- The new graph uses Mocha 12 and browser-manager 3. It no longer contains
  `braces`, `basic-ftp` or `extract-zip`. Remove the obsolete local throwing
  extractor, its override and its characterization test. Keep the launcher's
  external-driver existence/version checks and explicit local connection.
- Retain the existing patched `serialize-javascript` 7.1.2 override. Do not
  suppress advisories or relax the moderate audit threshold or four-case gate.
- npm's in-place major upgrade retained conflicting old peer relationships.
  A clean isolated resolution succeeded without peer overrides. Archive the
  old installed tree/lock locally, regenerate normally, and verify with
  `npm ci`; do not use `--force` or `--legacy-peer-deps`.

## Reproduced desktop test race

The first WebdriverIO 10 run passed three cases and failed the batch-copy case:
the expected progress attribute was `100`, but the immediate read returned
`null`. Its failure DOM then showed `value="100"`. The Classic `getAttribute`
implementation is unchanged across the two WebdriverIO majors.

The file engine emits its per-file `completed = 2` event while the job is still
running, then emits the terminal completed event. `ActivityPanel` deliberately
shows indeterminate progress during verification. Seeing `2/2` therefore does
not prove that the terminal progress update has arrived.

The test now waits for the actual `value="100"` condition before retaining its
original exact assertion. The existing bounded condition wait fails if that
state never arrives. There is no fixed sleep, case retry, skipped case or
application behavior change.

## Relationship to acoustic acceptance

The acoustic results remain bound to product commit `4d50c5d` and executable
SHA-256 `A73031C49E20E83C801345B8F03C9B3E5CDD9300CE4AD6297768862A4B40D3DC`.
An exact copy of that executable and its required resources is retained in the
ignored measured-release evidence directory before rebuilding. The dependency
migration does not claim a new physical acoustic measurement of a different
binary. See [the original acceptance record](2026-10-19-recording-readiness-acceptance.md)
for the device, settings, procedure and limits.

## Local validation before rebuilding

On Node 24.21.0 / npm 11.19.0 and Rust 1.98.1:

| Check | Result |
| --- | --- |
| Clean `npm ci` and `npm audit --audit-level=moderate` | Passed; zero advisories |
| Svelte/TypeScript, formatting and frontend build | Passed |
| UI suite | 43 passed |
| Publication tests | Three passed; the retired extractor-only test is no longer applicable |
| Strict desktop Clippy, Rust formatting and Rust suite | Passed; 92 tests |
| Signed-updater validation | Passed; no installation |
| WebdriverIO 10 against the measured release executable | All four mandatory cases passed |

The updater-validation example retains the previously recorded non-blocking
window-icon diagnostic. npm also reports a deprecated `whatwg-encoding`
transitive and warns that driver-package and esbuild install scripts are not
yet covered by `allowScripts`; the frontend build succeeds.
These messages are not hidden and do not suppress the security audit.

The dependency/condition-wait implementation commit is
`d2b263de7bc773601c06d9b8c3bb68a5000f4124`.

## Rebuilt application acceptance

The optimized release build at the implementation commit above passed all four
mandatory cases with WebdriverIO 10.1.0, WebView2/EdgeDriver 155.0.4283.62 and
Tauri driver 2.1.0. The existing Settings file was restored byte for byte.

Rebuilt executable SHA-256:
`E87400397E94807A51B745F8C5889E8515D25301AA57A8B6CA3FB0E93F4CFC2A`.

The distinct hash includes the regenerated frontend bundle. Sanitized resource
hashes, versions and individual case results are in
[the rebuilt acceptance JSON](2026-10-20-desktop-dependencies-acceptance.json).
Raw logs, archived dependencies and the original measured executable/resources
remain ignored local evidence; they are not repository publication artifacts.
