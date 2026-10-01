# Windows Desktop E2E

Four critical flows drive the actual release EXE, WebView2 and Rust IPC through
WebdriverIO's standard client, an external `tauri-driver` and Edge WebDriver.
No WebDriver plugin, test server, command mock or test hook is shipped in the
application. One serial spec owns the shared Windows profile and restart flow.

1. Offline launch, named keyboard controls, silent success defaults, named
   Ctrl+Shift+N windows, shared settings and single-instance relaunch.
2. Start/pause/resume/stop and focus continuity while the external peer holds
   observation. All four shell commands must arrive before the barrier is
   released. A stale observation cannot restore an inapplicable control.
3. Two-file selection, single-only rename, container duration, completed native
   progress and exact batch-copy SHA-256 outputs.
4. Arabic/RTL, destination and optional success sound across an actual restart.

Install the pinned build dependencies, build Observer/file-helper resources,
and run `npm run desktop:build -- --no-bundle`. In PowerShell call
`./scripts/setup-desktop-drivers.ps1`, then `npm run test:desktop`. Driver setup
pins `tauri-driver` 2.1.0 and downloads the exact EdgeDriver version corresponding
to the installed WebView2 runtime from Microsoft's official endpoint. The test
preflight verifies Cargo's installed driver record and runtime version equality.
Explicit `TAKEDOCK_TAURI_DRIVER`, `TAKEDOCK_EDGE_DRIVER` and optional
`TAKEDOCK_APP_EXE` paths are also supported. Missing infrastructure fails.

Close TakeDock first. The launcher preserves existing Settings as bytes and
restores them in `finally`; each test starts against a fresh fixture-only ADB
executable and destination. It never uses a real phone. The external fixture
reuses the native file helper library and supports injected events and held
observation. Normal WebDriver teardown closes each application session; final
cleanup targets only processes launched from the tested executable/fixture root.

`test-results/desktop/acceptance.json` records the commit, executable/resource
SHA-256 hashes, driver/runtime versions and four outcomes. Skips, cancellation,
missing cases, failed cases and infrastructure failure all fail the suite.
Fixture diagnostics remain in a unique temporary directory. CI uses a disposable
runner profile. `TAKEDOCK_TEST_TMP` can choose a larger local temporary volume.

The suite does not certify Android execution, NVDA speech, native folder
selection, installer behavior or real-phone latency; those have separate
packaged/device acceptance checks. The initial Windows 10 22H2 run passed all
four flows with WebView2/EdgeDriver 154.0.4258.48 and Rust 1.98.1.

GitHub-hosted Windows runners use a disposable ordinary account through the external
`scripts/run-desktop-unprivileged.ps1`. The child asserts it is not an
administrator and propagates any failure. WebView2 150+ ignores environment
debugging arguments in elevated hosts, preventing normal session creation:
[Microsoft issue](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5645),
[WebdriverIO investigation](https://github.com/webdriverio/desktop-mobile/issues/542).
No product test hook, downgraded runtime, changed host policy or allow-failure
condition is used. All four cases remain mandatory.

The ordinary account and its noninteractive desktop are test-owned and removed
on completion. No local accounts are created outside GitHub-hosted runners.
The fixture is built in the parent CI job before the child starts. Local tests
use the current user's filtered token and preserve the existing Settings bytes.

## Development dependency decisions

The published `extract-zip` package has no patched release for
[symlink archive writes](https://github.com/advisories/GHSA-7pqw-9j4j-h8q3).
WebdriverIO depends on it through an unused automatic browser downloader.
TakeDock requires external drivers, so a clearly named local dependency guard
replaces that unused extractor and always throws; it contains no extraction
code and is not bundled in the app. Its test must fail if a downloader attempts
archive extraction. Remove the override when upstream drops the affected
dependency or publishes a verified fix. `serialize-javascript` is overridden
to the current patched 7.1.2; the complete desktop suite verifies compatibility.
No audit advisories are ignored. `npm audit` gates development dependencies too.
