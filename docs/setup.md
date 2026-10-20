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

For quicker phone access on the tested Samsung Android 16 device, enable the
native **Wireless debugging** tile under Developer options → Quick settings
developer tiles, then add it to Quick Settings. Its long press opens the wireless
debugging page directly, including pairing and connection details. This was
verified on that phone; other manufacturers can arrange these controls differently.

The destination defaults to your Windows Videos folder / TakeDock and is created
automatically. Existing selected folders are preserved. Optionally change the
destination in Settings and Save. Set `adb.exe` if absent from
`PATH`. ADB/deadline changes apply on Reconnect; language, folder and optional
success sound persist. Failure feedback is enabled; optional success feedback
defaults off. The ready-to-speak cue has its own setting and defaults on.

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

## Starting a take

After Start, **Preparing to speak** is ordinary text. A fresh camera confirmation
starts the saved guard interval. **Ready to speak** appears when it expires, and
one short PC cue plays through the Windows session's default audio output. Begin
speaking after this cue finishes. Keep your headphones selected in Windows if
you want to hear it there. The camera's own phone beep can precede actual capture.

Settings provides **Play the ready-to-speak cue**, enabled independently of
optional success feedback, and **Speech-readiness delay after confirmation
(milliseconds)**. The initial guard is 800 ms; accepted values are 500–5000 in
steps of 50. A changed delay or re-enabled cue applies to the next Start without
reconnecting. Disabling invalidates pending playback while the ordinary readiness
text can still advance. Reduce the delay only after checking the first spoken
words on your phone; see the [acceptance procedure](testing/recording-readiness.md).

Stop, Pause, lost camera eligibility, an unknown state after confirmation, or a
connection change cancels a pending cue. Resume and opening another window do
not replay it. Capture controls remain usable while preparation, audio or file
operations run. The recorded preparation preamble stays in the video.

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

### Stop and delete the current take

When available, select **Delete this take** beside **Stop**, then activate
**Stop and delete**. The checkbox starts unchecked in each window and resets for
each take/reconnection or when its panel closes. Progress/results are shared.
Ctrl+Shift+R remains ordinary Start/Stop. Pause/Resume keeps the take association.
Automatic association requires Open Camera to be the sole video writer in
`/sdcard/DCIM/OpenCamera`, with file watching ready before TakeDock starts the
take. Split or ambiguous recordings are retained for explicit selection.
When unavailable, stop normally and select the intended completed video in
Videos to delete it with the existing confirmation. A retained result means
automatic deletion did not complete. An unknown result means deletion was
authorized without a confirmed outcome; inspect/refresh Videos before acting.
A newer action cancels uncommitted deletion; an already authorized mutation may
finish. If a new Start is rejected after cancellation, issue a fresh explicit
action after inspecting the state; the program never substitutes another toggle.
See [acceptance and device limits](testing/current-take-discard.md).

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
