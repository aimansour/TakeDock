# TakeDock

[العربية](README.ar.md) · [Download](https://github.com/aimansour/TakeDock/releases/latest) · [Setup](docs/setup.md) · [Keyboard guide](docs/keyboard.md)

TakeDock is a dark, keyboard-accessible Windows app for controlling Open Camera
and managing Android videos over an existing wireless ADB connection. Built
with Tauri 2, Svelte 5 and Rust, with a small Android Observer. MIT licensed.

## Features

- Start, stop, pause and resume video using Open Camera's configured volume keys.
- Dispatch immediately through a persistent ADB shell; verify independently
  through accessibility events without blocking subsequent controls.
- Mark completed videos to move them from `/sdcard/DCIM/OpenCamera` into
  `/sdcard/DCIM/TakeDock` on the phone; delete selected source videos or rename one.
- Marking removes the video from TakeDock after confirmation. Copy it to Windows
  yourself through Phone Link. Existing destination files are never overwritten.
- Manage files while Open Camera is closed; recording requires it active in video mode.
- English and Arabic/RTL, labelled native controls, logical focus, absent
  inapplicable capture buttons, and no unsolicited result announcements.
- Silent success by default; optional rising success sound in Settings.
  Failures always use a distinct falling sound.
- Named windows share one phone, settings and file queue.
  Reopening the shortcut activates the last active window; Ctrl+Shift+N opens another.
- Source videos appear newest first, with container duration accurate to the
  nearest millisecond. Handed-off files are no longer managed by TakeDock.
- Native activity progress and batch counts remain available without live regions.
- Automatic startup/manual update checks. Only **Install update** starts
  installation, with cryptographic artifact/version verification.

## Getting started

1. Install the x64 installer from [Releases](https://github.com/aimansour/TakeDock/releases).
2. Install [Android Platform Tools](https://developer.android.com/tools/releases/platform-tools).
   Pair, authorize and connect one phone **outside TakeDock**. Check `adb devices` shows `device`.
3. Choose `adb.exe` in Settings if it is not on `PATH`.
4. Configure Open Camera: volume up starts/stops video, volume down pauses/resumes.
   Keep it active in video mode for recording controls. Refresh Videos to manage files.

The app deploys its bundled signed Observer and MIT file helper through authorized
ADB. It does not pair phones or require enabling an AccessibilityService.
The Observer preserves existing services, including TalkBack.

## Compatibility and behavior

Target: Windows 10/11 x64, Android 16 and later, one phone at a time. Hardware
acceptance uses Windows 10 22H2, Android 16 and Open Camera 1.56.2. Windows 11 and
Android 17 hardware acceptance are not yet claimed. See [evidence](docs/testing/packaged.md).

The independent verification deadline defaults to 10 seconds and is configurable.
Unconfirmed commands produce failure feedback; toggles are never automatically
retried. Tested Open Camera ignores stops within about 500 ms of starting/resuming.
Inspect the phone if a command cannot be confirmed.

Unfinished, damaged or unrecognized containers remain visible but protected.
Delete is permanent and requires batch confirmation. Marking is a one-way phone
folder move: TakeDock offers no destination view or return action. If confirmation
is lost, refresh the source list before further action. Read [setup](docs/setup.md).

## Development

[CONTRIBUTING](CONTRIBUTING.md) covers pinned builds and checks;
[architecture](docs/architecture.md), [release/signing](docs/releasing.md),
[SECURITY](SECURITY.md), and [third-party notices](THIRD_PARTY_NOTICES.md) cover internals.
CI requires Rust, frontend, Android and four actual release Desktop E2E flows.
Failed, cancelled, skipped or missing mandatory checks block publication.

Open Camera is separate and is not bundled. No Open Camera source is incorporated.
