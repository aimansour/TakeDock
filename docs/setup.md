# Setup and troubleshooting

[العربية](setup.ar.md) · [Keyboard guide](keyboard.md)

## Installation and connection

Run the x64 installer from GitHub Releases. The per-user installer supports
English/Arabic and downloads the Microsoft WebView2 bootstrapper when required.
The installer has an updater integrity signature; it currently has no Windows
Authenticode certificate. Manual downloads can be checked against `SHA256SUMS.txt`
using `Get-FileHash -Algorithm SHA256`.

Install current Android Platform Tools. Enable Developer options/Wireless debugging
on the phone. Use its displayed addresses/ports in your terminal outside TakeDock:

```text
adb pair PHONE_ADDRESS:PAIRING_PORT
adb connect PHONE_ADDRESS:DEBUGGING_PORT
adb devices
```

Enter the pairing code when requested. Pairing and connection ports differ and
may change. Both devices need a network allowing their connection. See
[official wireless ADB guidance](https://developer.android.com/tools/adb#connect-to-a-device-over-wi-fi).
Keep exactly one authorized `device`; disconnect other phones/emulators yourself.
TakeDock does not pair, authorize or connect to network addresses.

The destination defaults to your Windows Videos folder / TakeDock and is created
automatically. Existing selected folders are preserved. Optionally change the
destination in Settings and Save. Set `adb.exe` if absent from
`PATH`. ADB/deadline changes apply on Reconnect; language, folder and optional
success sound persist. Failure sounds are always enabled; success defaults off.

## Phone configuration

Install Open Camera separately. Use its default `/sdcard/DCIM/OpenCamera` folder;
alternate folders and Storage Access Framework destinations are unsupported.
Configure volume up for video start/stop and volume down for pause/resume.
Keep Open Camera active in video mode for capture controls.

TakeDock installs its bundled signed `app.takedock.observer` APK through authorized
ADB and runs instrumentation with `FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES`.
It listens to events, preserves TalkBack, and needs no AccessibilityService toggle.
The bundled MIT native helper manages the fixed video directory as the ADB shell
user. Closing TakeDock stops the Observer; Reconnect establishes a new session.
Changes to Open Camera version/language/layout/settings may need revalidation.

## Managing files

Refresh Videos, select completed files, then Copy, Move or Delete. Rename appears
for one selected file and preserves the extension. Delete has batch confirmation
and is permanent. Existing destinations are retained. Move checks the original
again after verified copying and before deletion. Independent file failures do
not prevent the remaining batch from being processed. Read per-file results by
ordinary navigation; there are no live regions or automatic result announcements.

Videos appear newest first with finalized-container duration rounded to the
nearest millisecond. Unknown duration is labelled unavailable. The Activity bar
shows native per-file transfer percentage, batch counts, independent verification
and the final result. Your reader can beep/speak progress according to its own
progress-bar settings, without app-generated speech or live regions.

Ctrl+Shift+N opens another window. Give it a name in Settings to identify it in
Alt+Tab. Names last until the window closes. All windows share the phone,
destination, settings and queue; result sounds occur once per backend result.
Reopening the desktop shortcut brings the most recently active window forward.

Active, paused and unfinished recordings are protected. Container validation is
conservative, not a full codec/playability test. Finish saving and Refresh.
Normal failure/cancellation removes temporary `.partial` copies; a forcibly killed
process may leave an abandoned partial file, which is not a successful copy.
Precommit phone journals recover the original on the next helper invocation.
Cancellation cannot reverse an already committed deletion; a verified desktop
copy remains available after a move.

## Troubleshooting

| Result | Action |
| --- | --- |
| No phone / multiple phones | Inspect `adb devices`; keep one authorized `device`, then Reconnect. |
| ADB not found | Choose Platform Tools `adb.exe`, Save and Reconnect. |
| Observer unavailable | Check phone installation restrictions/authorization. Reconnect; file operations still work. Remove an old development Debug Observer before installing the differently signed Release APK. |
| Capture unavailable | Open Camera must be active in video mode, with correct volume-key settings. |
| Unconfirmed recording | Inspect the phone; toggles are never retried. Reconnect to synchronize. Adjust the deadline if needed. |
| Destination exists | Choose another destination or rename the video; no overwrite occurs. |
| Source changed | Refresh and select the new file identity. |
| Protected video | Finish saving. Unrecognized/damaged containers require phone-side handling. |
| Media indexing warning | Mutation succeeded; refresh the phone media app. |
| Update check failed | Check GitHub connectivity; retry from Settings. Checking cannot install. |

Settings: `%APPDATA%\app.takedock\settings.json`. There is no telemetry or automatic
upload of video content. Finish recording/file work before choosing Install update;
installation closes the app and cancels pending work.
