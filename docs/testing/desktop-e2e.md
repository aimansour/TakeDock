# Windows Desktop E2E

Four critical flows drive the actual release EXE, WebView2 and Rust IPC through
WebdriverIO's standard client, an external `tauri-driver` and Edge WebDriver.
No WebDriver plugin, test server, command mock or test hook is shipped in the
application. One serial spec owns the shared Windows profile and restart flow.

1. Offline launch, named keyboard controls, silent success defaults, named
   Ctrl+Shift+N windows, shared language/readiness settings, legacy readiness
   defaults on/800, and single-instance relaunch.
2. Start/pause/resume/stop and focus continuity while the external peer holds
   observation. All four shell commands must arrive before the barrier is
   released. Preparing remains after more than the default guard while
   confirmation is held; later capture intents cancel it. A stale observation
   cannot restore an inapplicable control. An independently confirmed Start
   becomes ready, retains focus and is shared by a newly opened window.
   The native discard checkbox starts unchecked per window; Space changes the
   same primary button to Stop and delete. Windows share finalization/completion.
   Exact fixture deletion, missing baseline, cancellation by a newer Start and
   a lost deletion acknowledgement are checked through the real Rust engine.
3. Two-file selection, single-only rename, container duration, completed native
   progress and exact batch-copy SHA-256 outputs. Recording reaches ready while
   remote hash verification is held; the barrier is released in `finally`.
   A discard waits behind that queue and a newer ordinary intent cancels it;
   releasing the queue must preserve both fixture takes with zero delete calls.
4. Arabic/RTL, destination, optional success sound, and readiness off/1250 across
   an actual restart, using explicit checkbox IDs.

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
Current-take discard requires separate [owned-device acceptance and preserved
fixtures](current-take-discard.md); actual Android watch support remains pending
until those cases run. The release tests use existing public bootstrap only for
read-only baseline synchronization and add no production test control.

GitHub-hosted Windows runners use a disposable ordinary account through the external
`scripts/run-desktop-unprivileged.ps1`. The child asserts it is not an
administrator and propagates any failure. WebView2 150+ ignores environment
debugging arguments in elevated hosts, preventing normal session creation:
[Microsoft issue](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5645),
[WebdriverIO investigation](https://github.com/webdriverio/desktop-mobile/issues/542).
No product test hook, downgraded runtime, changed host policy or allow-failure
condition is used. All four cases remain mandatory.

The ordinary account is test-owned and removed on completion. Only on disposable
GitHub-hosted VMs, the launcher adds this account's access to `WinSta0/Default`
and restores the exact original desktop/window-station DACLs in `finally`.
Chromium's session-wide `Local\ChromeProcessSingletonStartup!` mutex can already
belong to `runneradmin`. The ordinary-account diagnostic reproduced an explicit
access denial for that mutex and native WebView2 ProcessSingleton failure, even
with a fresh writable profile. The launcher temporarily adds only this account
to that mutex's DACL and makes its mandatory label Medium for ordinary Chromium.
It restores the exact original DACL/label and never acquires it or
changes its ownership. This hosted-only setup is tested with a standalone
official WebView2 control before relying on the actual application suite.
The account also receives temporary access to its hosted session's
`BaseNamedObjects` directory. Native probes confirmed that a writable profile
and mutex DACL alone cannot grant Win32 namespace initialization: unique local
mutexes also failed with error 5. Granting the ordinary account the directory
rights required by Win32 made local/global mutex creation and native WebView2
startup succeed. The exact original directory DACL is restored in `finally`;
the directory's existing integrity label is preserved.
Private noninteractive stations failed native WebView2 ProcessSingleton startup.
This follows [Windows interactive-client access](https://learn.microsoft.com/en-us/windows/win32/secauthz/processes-in-the-client-security-context).
No local accounts or desktop ACL changes occur on the developer's computer.
The fixture is built in the parent CI job before the child starts. Local tests
use the current user's filtered token and preserve the existing Settings bytes.

## Development dependency decisions

WebdriverIO is pinned consistently to 10.1.0, with its Mocha 12 adapter. This
replaces the old Mocha/chokidar/braces and browser-manager/proxy/FTP dependency
chains that blocked the audit gate. See the
[v10 migration guide](https://webdriver.io/docs/v10-migration) and
[dependency acceptance record](../research/2026-10-20-desktop-test-dependencies.md).

The new browser manager no longer depends on the vulnerable `extract-zip`
package, so the former local throwing adapter, override and obsolete adapter
test have been removed. The launcher still requires existing, version-matched
external Tauri and Edge drivers and connects to their explicit local endpoint.
`serialize-javascript` remains pinned to patched 7.1.2. No audit advisories are
ignored; `npm audit` gates development dependencies too. All four real release
desktop cases remain mandatory, including the existing failure/skip gate.
