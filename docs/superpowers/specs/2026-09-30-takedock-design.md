# TakeDock design proposal

Status: updated with the user's Svelte choice, release and sound requirements,
and a focused Windows Desktop E2E verification layer. Inline implementation
was authorized and started on 2026-10-01.

## Intended outcome

TakeDock is an MIT-licensed Windows 10/11 desktop application built with
Tauri. It gives screen-reader users fast keyboard access to Open Camera video
recording and to videos stored on one externally connected Android phone.
The desktop application must remain responsive while recording verification
and file operations run independently in the background.

The initial interface languages are English and Arabic, including RTL layout.
Source code, identifiers, development documentation, and the default README
are English. Essential user documentation also has an Arabic edition.

## Requirements and boundaries

- ADB pairing, authorization, and Wi-Fi connection happen outside TakeDock.
- Exactly one authorized phone is supported in a session. Multiple connected
  phones produce an explicit connection condition rather than an arbitrary
  selection. Offline or unauthorized devices do not become active sessions.
- Recording controls require the user to open Open Camera in the foreground,
  select video mode, and configure its volume keys for capture.
- Volume up starts/stops recording; volume down pauses/resumes recording.
- Managing completed videos requires only the ADB connection, independently
  of whether Open Camera is open.
- The phone source directory is `/sdcard/DCIM/OpenCamera`.
- A user-selected destination directory on Windows is saved in Settings.
- Copy retains the phone original. Move copies and verifies the desktop file
  before removing the phone original. Delete removes the selected originals.
- Copy, move, and delete support a selection of multiple videos. Rename is
  available only for one selected video.
- Dark appearance is the default. Every action works with the keyboard.
- No text-to-speech, live regions, unsolicited announcements, or notification
  focus changes. Controls retain their native accessible names and semantics.
- Controls that do not apply to the current state are absent from the DOM and
  accessibility tree, rather than remaining visible as disabled buttons.
- Success is silent by default. Settings can enable the optional success
  sound. A distinct failure sound is enabled by default. Detailed results
  remain available as ordinary, user-navigable text without spoken alerts.
- Automatic update checks run at startup; manual checks are in Settings.
  Installing an update always requires a separate explicit user action.
- Release dependencies use current compatible stable versions with lockfiles.

## Chosen architecture

### Desktop interface

Use Tauri 2, Rust, Svelte 5, TypeScript, and Vite, with native HTML controls.
Use plain Svelte components; no SvelteKit router, route announcer, or SSR.
Separate Recording, Videos, and Settings into predictable keyboard navigation
areas.
Use translated visible labels rather than redundant ARIA text. Ordinary
status text is discoverable through screen-reader navigation without being
announced automatically.

Button activation sends the command directly from the user event and updates
the predicted recording state without awaiting verification. Dispatch must
not depend on reactive effects, DOM flushing, animations, or `tick()`.
Keep state/IPC controllers separate from the Svelte components. Own event
subscriptions once and clean them up when the application closes. Coalesce
progress updates independently of the recording-command path.

Reuse the primary recording button while changing its action and accessible
name. Remove inapplicable secondary controls. When removal affects focus,
preserve logical keyboard focus on the corresponding next action. Use keyed
video rows with stable identities and handle rename/delete explicitly.
Avoid whole-page re-rendering and lost selection.
Keyboard shortcuts are documented and avoid typing contexts and key repeats.

Check Svelte component types and accessibility warnings in CI. Resolve
current compatible stable dependency versions together; never force a newer
TypeScript version outside the component checker's supported peer range.

### Rust session engine

Maintain separate workers for device discovery, command dispatch, observer
events, verification, and file jobs. ADB device discovery uses a long-lived
device-tracking stream. Attach every operation to an explicit device serial
and session generation so an old job cannot act on a replacement phone.

Keep a persistent non-interactive ADB shell for capture commands. Send
`input keyevent --async KEYCODE_VOLUME_UP` or
`input keyevent --async KEYCODE_VOLUME_DOWN` in order. Commands carry local
sequence identifiers and shell completion markers. Neither observer reads,
file transfers, hashing, nor update checks share the dispatch worker.

The dispatcher returns after accepting a command for transmission, without
waiting for the observed recording state. Shell delivery errors produce an
immediate failure sound. Delivery acknowledgement is not proof that Open
Camera executed the requested action. Never automatically retry a volume-key
toggle: a retry can reverse a command that actually succeeded.

Foreground and video-mode eligibility come from the observer. If these
known prerequisites fail, recording controls are absent; the video manager
remains usable. Foreground evidence can race a user switching phone apps;
the independent verifier reports any resulting discrepancy.

### TakeDock Observer on Android

Ship a small, independently implemented Android APK containing a custom
Instrumentation runner targeting its own package. Launch it through ADB;
it does not launch an activity or instrument the Open Camera process.

Obtain UiAutomation exclusively with
`UiAutomation.FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES` and register
`setOnAccessibilityEventListener`. There is no zero-flag fallback. Failure
to establish this connection is an observer error, never permission to
suppress other accessibility services.

Perform one initial accessibility snapshot and subsequent event-triggered
reads. Coalesce bursts of relevant events on an independent worker, keeping
the main-thread callback short. Read stable Open Camera resource identifiers
and localized content descriptions to distinguish photo mode, video idle,
recording, and paused recording. Ignore unrelated application content except
the minimum information needed to detect loss of foreground eligibility.

Emit a versioned state stream using instrumentation status messages over a
dedicated long-lived ADB process. Include a session identifier, monotonically
increasing observation sequence, and Android event timestamps. Do not save
full accessibility trees or unrelated on-screen text.

No `uiautomator events`, XML dumps, periodic accessibility queries, screenshot
recognition, hidden-API reflection, or screen-reader suppression is allowed.
If the observation transport proves unreliable in testing, an ADB-forwarded
local socket is the alternative; it retains the same event-driven observer.
A user-enabled AccessibilityService is another possible architecture, but
requires extra phone setup and is not the selected approach.

Install/update the bundled Observer through the already authorized ADB
connection when required. Present its installed/session state in Settings.
Observe supported Android APIs and build against Android 17 where available,
while retaining Android 16 runtime compatibility. Disconnect and shutdown
must terminate the session and release UiAutomation cleanly.

### Independent verification

Keep predicted and observed recording states separate. Each command records
its session, sequence, prior observation boundary, and expected transition.
Newer commands may be sent while earlier ones await verification. Process
transition history in order, and distinguish coalesced events from evidence
that a command failed. Stale results must not rewind a newer predicted state.

A confirmed expected transition produces a success sound only when the user
has enabled it in Settings; the default is silence on success. Proven execution
failure or transport failure produces a different failure sound, with details
stored in an ordinary results panel. Verification delay never disables the
interface or delays dispatch of another command.

Use a background verification deadline independent of command dispatch,
with a documented configurable value and timing measurements to guide its
default. Four seconds is not a requirement. Expiry alone means unconfirmed,
not proven failure: signal the observation problem without inventing a
recording result or repeatedly sounding an alert. Loss of an established
connection is an explicit error condition. Reconcile from fresh observer
evidence while preserving causality and the latest user intent.

### Video management

List only supported video files inside the fixed Open Camera directory.
Use filename-safe protocols and shell argument quoting; do not parse
human-formatted `ls` output. Validate names, directory containment, Windows
filename rules, collisions, and symbolic links before mutations.

File operations use a separate bounded queue with per-file progress, results,
and cancellation. Batch results retain selection context and distinguish
successful, failed, skipped, and cancelled entries. Refresh the list on user
request and after completed mutations. Protect files known to be recording
or still being finalized; file management must not stop recording.

Copy to a unique temporary desktop file, verify transfer completion and
SHA-256 against the unchanged remote source, then publish to the final path
without replacing an existing file. Move removes the original only after
these checks succeed and another source-identity check passes. Disconnect,
checksum mismatch, insufficient disk space, or cancellation must retain
the phone original. Partial desktop files are identified and safely cleaned.

Rename affects exactly one selected video, preserves its extension, and
never overwrites another phone file. Delete presents one accessible batch
confirmation; camera actions do not present confirmation dialogs. File
mutations refresh Android media indexing as needed.

### Settings and updates

Persist language, destination directory, optional external ADB executable
path, sound preferences, and verification timing. Detect ADB on PATH and
common SDK locations. Do not bundle platform-tools until its distribution
terms and packaging are explicitly addressed. Explain missing ADB through
the bilingual setup guide.

Use the official Tauri updater with a pinned public signing key and the
GitHub Releases `latest.json` endpoint for `aimansour/TakeDock`. Generate
signed updater artifacts in release CI. Keep private updater and Observer
signing material outside the repository. A startup check cannot download,
install, restart, or steal focus. The user explicitly activates Install
update in Settings. Missing first-release metadata is handled predictably.

## Repository and delivery

Provide MIT LICENSE, English README and Arabic README, bilingual setup and
keyboard guidance, architecture and contributor documentation, changelog,
security policy, issue templates, pull-request template, and reproducible
build instructions. Do not include private device identifiers, phone unlock
credentials, local absolute paths, signing keys, or personal videos.

Build the Observer from source, bundle its verified APK as a Tauri resource,
and produce a Windows installer. CI checks Rust, Svelte/TypeScript, Android
builds, and important behavior tests. A small Windows Desktop E2E suite runs
the release Tauri executable and its bundled resources in actual WebView2,
with the real Rust IPC/backend and an external ADB fixture executable.
It covers critical integration paths without requiring a physical phone in
GitHub Actions. The fixture uses the existing external-ADB setting and is
never packaged into the application. Desktop tests do not certify NVDA or
Android Observer behavior; those still receive separate real-device checks.
GitHub automatically publishes versioned releases with signed updater
metadata after all required checks and artifact
validation succeed. A failed, cancelled, or incomplete required check prevents
publication. Publication is authorized by the user and needs no further
per-release confirmation. Keep version publication ordered so an older build
cannot replace a newer release's updater metadata.

## Verification and acceptance

1. Unit tests cover command transitions, stale events, rapid commands,
   observation timeout, disconnect, filename safety, and move integrity.
2. Svelte component tests cover keyboard use, state-specific accessible controls,
   focus continuity, selection, single rename, RTL, and absence of live
   announcements. Inspect real WebView2 accessibility with NVDA where tools
   permit; do not equate a browser test with a full screen-reader audit.
3. Real-phone tests record a dedicated test video, pause, resume, and stop;
   compare predicted/observed states and capture timing distributions.
4. Run real recording tests while a phone screen reader is enabled and
   verify that it remains bound and usable before, during, and after Observer
   sessions. Restore pre-test settings.
5. Test copies, moves, Unicode renames, batch deletion, collisions, failed
   transfers, disconnect, and cancellation using only test-created fixtures.
6. Verify file management while Open Camera is closed or backgrounded.
7. Artificially delay/fail verification and confirm that later commands
   still dispatch and that old results cannot rewind newer state.
8. Build and launch the actual Windows installer/application, validate update
   metadata/signatures, and verify that checking never installs an update.
9. Required Windows Desktop E2E checks cover application launch and keyboard
   navigation, recording commands while verification is deliberately held,
   stale results and focus continuity, batch-copy IPC and actual destination
   files, and Settings persistence after restart. Use deterministic fixture
   state and fresh test data; failures or skipped cases prevent publication.
   Keep test drivers and test IPC hooks out of the release application.
10. Record which Windows/Android versions were actually tested. Android 17 and
   Windows 11 are not certified solely by compilation or Windows 10 testing.

## Exploration evidence on 2026-09-30

- Repository begins with a README and one initial commit; no product exists.
- One authorized Wi-Fi ADB phone is connected: Android 16 / API 36.
- Installed Open Camera is version 1.56.2, target SDK 36.
- `/sdcard/DCIM/OpenCamera` exists on the connected phone.
- `input help` advertises asynchronous key events on this phone.
- A non-recording shell round-trip probe measured eight samples per method:
  new ADB shell processes averaged 99.08 ms (including a cold first sample),
  while a warmed persistent shell averaged 32.57 ms. These are transport
  observations, not measured recording-command or application latency.
- Open Camera source at tag `v1.56.2` confirms `take_photo`, `switch_video`,
  and `pause_video` resource identifiers and changes the shutter/pause
  content descriptions when recording state changes.
- Rust 1.97.1, Node 24.21.0, ADB 37.0.1, Java 17, Windows C++ build tools,
  and Android SDK platforms 35/36 are installed. Exact dependency versions
  will be resolved from current stable registries during implementation.
- NVDA is running on the desktop. No enabled phone accessibility service
  was reported during this read-only inspection; coexistence remains untested.
- No recording, file mutation, Observer installation, or product build has
  yet been performed.

## Primary references

- [Android UiAutomation](https://developer.android.com/reference/android/app/UiAutomation)
- [Android Instrumentation](https://developer.android.com/reference/android/app/Instrumentation)
- [Open Camera user guide](https://opencamera.sourceforge.io/help.html)
- [Open Camera source](https://sourceforge.net/p/opencamera/code/ci/master/tree/)
- [Tauri updater](https://v2.tauri.app/plugin/updater/)
- [Android 17 changes](https://developer.android.com/about/versions/17/behavior-changes-all)
- [Svelte frontend investigation](../../research/2026-09-30-frontend-comparison.md)
- [Tauri WebDriver testing](https://v2.tauri.app/develop/tests/webdriver/)
- [Tauri WebDriver CI](https://v2.tauri.app/develop/tests/webdriver/ci/)
# Accepted desktop refinements (2026-10-01)

The user manually installed and accepted the baseline screen-reader controls,
recording commands and file transfer. They now request multiple named windows
within one application process. Reopening the desktop shortcut activates the
most recently focused window; Ctrl+Shift+N and a Settings button create another.
Names belong to the individual window and last until it closes. All windows
share the one phone engine, file queue, destination and persisted settings.
Settings changes propagate to every window. Result sounds occur once per
backend result, irrespective of window count. Startup update checking happens
once per application process; manual checks remain available in Settings.

An empty destination migrates to the Windows Videos/TakeDock directory, created
automatically. A user's existing selected directory is preserved. Videos are
ordered newest first, using the source modification time, with stable name
ordering for ties. Their duration is read from the finalized MP4 movie timeline
or WebM Info timeline, respecting the time scale and rounding to milliseconds.
Missing or invalid metadata is shown as unavailable, never estimated from size.

Every window exposes ordinary activity text and a natively labelled progress
element covering listing and file phases, including verification and completion.
It never uses live regions or changes focus automatically. File processing and
its progress remain independent of recording dispatch and Observer verification.
The user's manual testing owns further physical desktop/phone interaction.
