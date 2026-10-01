# Packaged acceptance

## Evidence on 2026-10-01

- Host: Windows 10 Pro 22H2, build 19045; Rust 1.98.1; Node 24.21.0.
- WebView2 and external EdgeDriver: 154.0.4258.48; tauri-driver 2.1.0.
- Phone: Samsung SM-A155F, Android 16/API 36; Open Camera 1.56.2; TalkBack enabled.
- Signed NSIS package extraction matched the previously tested EXE, Observer and
  all three ABI helpers. The installed per-user EXE matched the release EXE.
- All four critical Desktop E2E cases passed against the installed application
  under a restricted user token, using an external fake ADB and actual Rust IPC.
- Replacing only the development Observer with the persistently signed release
  APK succeeded. The installed desktop app reached Ready/Start with the real
  phone and packaged Observer. Enabled accessibility service settings remained
  unchanged. Prior Observer acceptance also verified bound-service coexistence.
- The native folder dialog exposes the Windows Folder edit and Select Folder
  controls; the assistant's native input tool could not complete selection.
- Native accessibility inspection finds named English/Arabic navigation,
  Settings controls and ordinary result text. Automated DOM tests prove absent
  inapplicable actions, stable recording focus and no live announcement regions.

The user took over physical Windows/phone testing and reported successful
installation, screen-reader compatibility, recording start/stop and video
transfer. This is user-reported baseline acceptance. It does not separately
certify every audio option or production latency measurement. They then requested
named shared windows, a default destination, accurate duration and native activity
progress. These refinements undergo native/component and real EXE fixture
acceptance before publication; further physical screen-reader use belongs to the
user. No assistant native UI/phone driving resumes during their testing.

## Timing and remaining coverage

The 0.1.1 release EXE passed all four extended fixture E2E cases on Windows 10:
named Ctrl+Shift+N windows, independent names, shared English/Arabic settings,
single-process relaunch, precise duration and final native progress, alongside
the original four flows. Native tests passed 51/51; component/controller tests
passed 23/23. The persistently signed 0.1.1 Observer passed unit tests, Release
lint and certificate verification; all three ABI helpers rebuilt successfully.
This does not claim a physical screen-reader audit of the new progress feature.

Earlier real-device Rust debug acceptance measured command enqueue at
28–88 microseconds and event confirmation at 126–877 ms in one four-action run.
These are backend debug measurements, not production desktop activation latency.
Fixture E2E proves later actions dispatch while verification is held, but its
timings are not phone performance. Production frontend event-to-dispatch,
control-update and real-phone confirmation measurements remain unclaimed.
The user's requested manual acceptance replaces further assistant desktop driving.

Windows 11 and Android 17 physical-device acceptance remain open. The APK targets
API 37 but that build setting alone does not establish Android 17 compatibility.
GitHub's Windows Server runner validates the external E2E/package gate; it is
not a substitute for Windows 11 screen-reader hardware acceptance.
