# Measuring complete video transfers

The implementation plan is
[2026-10-22-large-file-transfer](../superpowers/plans/2026-10-22-large-file-transfer.md).
Phone results are separate from deterministic correctness tests and hosted
desktop acceptance. No performance result has been established by the diagnostic
implementation alone.

## Opt-in phase diagnostics

Ordinary builds omit `transfer-diagnostics`. Their tracing facade has no state,
clock reads, environment lookup, writer or heap allocation. The private job key
also has no fields in that configuration. Diagnostics do not alter the helper
protocol, hashing order, file queue or capture commands.

For an instrumented release, run from the managed execution worktree:

```powershell
npm run build
npm run tauri -- build --no-bundle --features desktop,transfer-diagnostics
```

Archive the resulting executable and all resources with their SHA-256 values
before rebuilding. Record the clean source revision. An instrumented baseline
must be frozen before an optimization; a later build cannot be relabelled as the
old baseline. Keep the released v0.1.5 artifact separately to measure instrument
impact. This feature is never enabled in the ordinary release build command.

Set `TAKEDOCK_TRANSFER_TRACE` only in the benchmark application's environment to
an absolute, nonexistent JSONL file inside the current ignored private trial.
Its parent directory must already exist. The process creates it exclusively;
an existing or invalid path disables observation without changing file-operation
outcomes. It never replaces an existing evidence file. One process-owned writer
serves that application's file services. Use a new trace path for every launch.

The writer receives a bounded 256-record queue through nonblocking sends. It
flushes each diagnostic record on its own thread. Saturation, a closed writer or
an I/O error invalidates measurement; no file job waits for diagnostic I/O. No
per-chunk trace is emitted. Only operation IDs, generations and numeric file
indices are recorded, without filenames, paths, serials or network addresses.

Schema 1 records are:

- One `header`: `schema`, `record`, `clock`, decimal-string `frequency`, `process`.
- `edge`: `job_id`, decimal-string `generation`, optional `file_index`, `stage`,
  `edge` (`mark`, `begin`, `end`) and decimal-string `ticks`.
- A `job_complete` footer after the authoritative terminal event has been
  delivered: `job_id`, `generation`, `dropped` and `writer_status`.

The footer request follows the terminal sink; it is not the operation's finish
timestamp. An asynchronous writer can make the footer visible after the UI
result. Wait for the matching footer outside the measured UI interval. Reject
missing footers, nonzero dropped counts, writer errors, missing spans, invalid
timestamps or mismatched correlations. Inspecting an in-progress hash edge in a
correctness test does not establish a complete benchmark result.

On Windows, native timestamps use QueryPerformanceCounter and the PowerShell
harness uses Stopwatch's counter. Require identical frequencies before combining
them. See [Microsoft's timing guidance](https://learn.microsoft.com/en-us/windows/win32/sysinfo/acquiring-high-resolution-time-stamps).
Other platforms label their diagnostic clock `process_relative`; those records
can measure internal spans but cannot establish cross-process UI timing.

Stages cover acceptance, enqueue, worker start, helper preparation, initial stat,
streaming, synchronization, stored length, post-transfer stat, verification,
remote and local hashes, publication, Move's fresh stat/deletion, indexing and
the authoritative terminal outcome. Begin/end guards close spans on errors.
Sort by ticks, not JSONL arrival order. Hash spans overlap only in a candidate
that actually executes them concurrently; their sum is not verification time.

## Evidence and phone ownership

Keep raw records and manifests under a fresh
`test-results/transfer-performance/device-<GUID>` directory. Do not reuse earlier
plans' scratch folders, phone trials or backups. Public evidence must omit
identifying data and media. Always preserve unrelated phone files and exact
manifest-owned source identities; filenames alone do not authorize a mutation.

Baseline/candidate comparison measures 500,000,000 and 1,000,000,000 bytes, Copy
and Move, and both backend acceptance-to-terminal and native-action-to-visible
terminal intervals. Notice expiry and fixture preparation are outside operation
time. Instrumentation uncertainty is measured, not assumed away.

After each actual phone session, including failures, leave Open Camera, return
Home and put the screen to sleep. Do not leave the camera foreground while
building, reviewing or waiting between stages. Preserve the user's wireless
pairing, ADB server and startup configuration. Sleeping the screen is the user's
requested heat-prevention cleanup; no power-off or unpairing command is needed.

The quarantined fake ADB executable must never be rebuilt or executed locally.
Full fixture/native acceptance remains hosted. A real-phone harness must invoke
the actual supplied ADB directly and must not call `scripts/test-desktop.ps1`,
which prepares the fake executable.

## Real-phone harness

`scripts/measure-transfer.ps1` uses the actual supplied ADB and native WebDriver
actions. It has no fake/proxy ADB dependency. The separate pure test scripts can
run locally without contacting a phone:

```powershell
pwsh -NoProfile -File scripts/transfer-benchmark/Core.test.ps1
pwsh -NoProfile -File scripts/transfer-benchmark/Device.test.ps1
pwsh -NoProfile -File scripts/transfer-benchmark/Ui.test.ps1
```

The runner requires absolute paths for ADB, tauri-driver 2.1.0, a matching
EdgeDriver, FFmpeg and FFprobe. `-TrialRoot` must be a new `device-<GUID>` directory
under this checkout's `test-results/transfer-performance`. `-Stage Baseline`
compares `released` with `baseline`; `-Stage Compare` compares `baseline` with
`candidate`. `-Manifest` defaults to `manifest.json` inside the trial. The existing
profile must be closed before each owned native session. When using an elevated
development shell, launch the runner with the existing
`run-desktop-unprivileged.ps1` wrapper and an owned bootstrap script/output
directory, rather than weakening WebView2 security or changing the user's account.

`-Builds` is a private JSON file with `schema: 1` and the named build objects.
Each build supplies `exe`, a full source `revision`, executable `sha256` and a
`resources` array of `{path, sha256}` relative to the executable directory. Include
`observer.apk` and all three `file-helper/<ABI>/takedock-files` binaries. The
runner verifies their hashes before launch and requires identical helper bytes
across compared builds.

Preparation generates new short MP4 seeds and appends a valid `free` box of
random, nonsparse bytes to reach the exact 500 MB/1 GB sizes. These padded
containers model bulk file I/O, not a gigabyte of distinct decoded frames. Each
full payload is hashed and decoded; backups remain immutable. Preparation pins
the ABI helper to the chosen resource SHA and records unrelated-file identities
and hashes before adding fixtures. It reserves space before allocation and
rechecks it for each run. Every Move gets a newly journaled duplicate and never
selects a reusable master or an unrelated file.

UI timing brackets the native action and matching operation notice, not progress
100%. Backend timing comes from its complete matching trace. A missed result,
clock mismatch, warning, cancellation, failed byte check or incomplete diagnostic
invalidates the run. Logs preserve the cause without calling it a fast success.
Native UI observations are bounded at 100 ms plus actual WebDriver request cost,
which enters the uncertainty budget. Build/watch coordination uses process events
instead of repeatedly polling an unchanged remote build.

Baseline calibration retains initial warm-ups and one released/instrumented pair
per size/kind; its conservative uncertainty includes the largest observed paired
UI difference. Candidate acceptance uses three alternating pairs per size/kind.
Both total metrics must improve in every pair; median gains must exceed the larger
within-build range and measured uncertainty. Only ambiguous groups with positive
paired gains can receive two extra pairs; all earlier samples remain. There is
no loop that reruns until a favorable result appears.

Raw per-run profiles, traces, timing records, journals and decoded/hash-verified
backups remain private. The runner checks published copies independently before
removing those newly created temporary outputs; backups and reports stay. It
restores settings only if they still match its last write, preserving unexpected
versions for inspection. No observed difference is attributed to the user.
