# Current-take discard acceptance

Automatic association supports `/sdcard/DCIM/OpenCamera` with Open Camera as
the sole video writer during a take. The watcher proves file history under this
assumption; it does not authenticate an application's authorship. The camera
must start from confirmed idle through TakeDock with a complete, fresh baseline.
Missing/ambiguous evidence disables automatic deletion for that take. Multiple
video segments remain for explicit selection in Videos; no segment is partially
discarded. Ordinary recording remains available when file watching is unavailable.

The native **Delete this take** checkbox starts unchecked. Space selects it;
the same primary button becomes **Stop and delete**. Enter/click sends one
discard request. Each window owns its selection; all windows share backend
progress/results. Selection resets on a new take, connection generation, hidden
control or panel remount. Ctrl+Shift+R always performs ordinary Start/Stop.
Pause/Resume preserves the take association. A later ordinary intent cancels
work that has not reached deletion authorization. An already authorized result
may arrive after a newer take; it never identifies that newer take as its target.
If a speculative Start becomes invalid after cancelling a pending Stop, the
user must issue a fresh action; no replacement camera toggle is sent automatically.

An unavailable association means stop normally, refresh Videos, inspect and
select the intended completed video, then use the existing explicit Delete
confirmation. Retained means automatic deletion did not complete. Unknown means
authorization was sent but the outcome could not be established: refresh and
inspect Videos before another action. Absence alone is not reported as success.
The checkbox is confirmation for this attempt, not a persistent preference.
There are no new live regions, TTS or automatic focus changes for progress.

## Software evidence

Pure state tests exercise tokens, baseline loss, split files, timing and
cancellation; helper tests exercise identity/recovery/permit boundaries. Linux
tests run real inotify syscalls. External-process Windows tests run independent
watch/capture/file transports. The four release desktop cases retain their
existing titles/count and additionally cover native checkbox/window behavior,
successful deletion, unavailable baseline, retained/unknown outcomes and a
cancelled discard behind a held copy. Existing public bootstrap IPC is read only
to synchronize baseline readiness; capture/discard uses actual native UI controls.

Record actual gates/hashes in `test-results/desktop/acceptance.json`. Fixtures do
not establish Android shared-storage event delivery or screen-reader coexistence.
Physical acceptance remains pending until the separately coordinated cases below
have actually run. Do not label a build device-verified from fixture results.

## Private physical-trial preservation proxy

`cargo build --locked -p takedock-adb-fixture --bin device-discard-proxy` builds an
external Windows test tool. It is never a release resource or default ADB path.
Set these environment variables in the process that starts the test application:

- `TAKEDOCK_TRIAL_REAL_ADB`: absolute real Platform Tools `adb.exe` path.
- `TAKEDOCK_TRIAL_ALLOWLIST`: absolute private JSON journal path.
- `TAKEDOCK_TRIAL_BACKUP_DIR`: private directory with adequate free space.
- `TAKEDOCK_TRIAL_FFMPEG`: absolute verified ffmpeg executable path.

Start the real ADB server directly before selecting the proxy; a trial wrapper's
process job must never own the shared server's lifetime. Select the proxy EXE
through the existing external ADB setting. Normal watch,
capture and device queries use independent forwarded processes. A Windows job
terminates descendant tools when the host cancels a proxy process. Direct helper
delete/rename is refused during the trial. A guarded request requires this private
manifest (identity values are the helper's exact JSON strings, never JavaScript
floating-point conversions):

```json
{
  "version": 1,
  "serial": "PRIVATE_OWNED_SERIAL",
  "helper": "/data/local/tmp/takedock-files-EXACT_64_HEX_DIGEST",
  "pre_start_ms": 1,
  "take_started_ms": 2,
  "pre_start": [{"name": "existing.mp4", "identity": "EXACT_HELPER_IDENTITY_JSON"}],
  "takes": [{"name": "DEDICATED_TEST_TAKE.mp4", "identity": "EXACT_FINALIZED_IDENTITY_JSON"}]
}
```

Journal a complete pre-Start manifest before creating the dedicated take. Record
the Start timestamp and allow only that new take after finalization. Populate the
exact finalized identity before releasing a separately held file queue to the
guarded worker; an absent/incorrect journal rejects the request. Never allowlist
a preexisting filename or device/inode. Keep other apps from adding videos.
The proxy streams the original into a unique private partial file, compares
local/remote SHA-256, checks unchanged exact identity, fully decodes with ffmpeg,
rechecks identity and syncs the backup/receipt before starting the remote guarded
helper. Failed verification never forwards deletion. Partial backups remain private
for diagnosis. Backup is bounded to 20 seconds; forwarding to 27 seconds total,
inside the host's 30-second limit. Use short test clips; deadline failure retains
the original. The remote 5000 ms permit wait starts only after preservation.

Only with the user's physical-trial agreement, run a normal take, a paused take,
an early request before 600 ms and a held/cancelled discard. Include actual
shared-storage watch delivery, root changes, screen reader coexistence and
ordinary capture during a held file job. Record command/observation order,
identity, existence/outcome, backup hashes/full-decode results and unchanged
unrelated files. Restore ADB/settings/accessibility state. Keep clips, serials,
paths and raw manifests in ignored task-specific `test-results/`; publish only
sanitized findings. Do not uninstall Observer to bypass a signing mismatch.
