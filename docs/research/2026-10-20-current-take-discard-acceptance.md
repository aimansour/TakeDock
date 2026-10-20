# Current-take discard software acceptance — 2026-10-20

Implementation is present; release/device acceptance is **incomplete**. No
release tag, publication, installer installation or physical-phone trial was run.

| Gate | Actual result |
| --- | --- |
| Windows locked Rust workspace | 150 tests passed after the queued-discard preflight fix |
| Ubuntu/WSL locked Rust workspace | 153 tests passed, including 8 actual inotify/watch cases |
| Desktop-feature Clippy / Rust formatting | Passed, warnings denied |
| UI suite | 52 tests passed; svelte-check 0 errors / 0 warnings; Prettier passed |
| Vite / Windows release EXE | Built successfully with pinned Rust 1.98.1 |
| Android helper resources | All three supported ABIs rebuilt using NDK 30.0.16248370 |
| Release/publication script tests | 4 passed |
| Updater validation example | Passed signed/corrupt/version/downgrade checks; no installation |
| Actual release desktop suite | Latest run 3/4; mandatory gate **not passed** |
| Physical Android / screen reader | Pending separate coordination |

The release recording case passed checkbox keyboard behavior, stable primary
button identity, window-local selection/shared progress, exact owned-fixture
deletion, missing baseline, a newer take cancelling authorization, and a lost
result reported as unknown. The expanded copy case first exposed an incorrect
test ordering: a newer cancelled discard legitimately replaced the completed
copy in Activity. The copy's existing 100% assertion is now retained before a
second held copy tests queued cancellation. A redundant initial-baseline reason
wait after an existing take was also removed. The final form of that second
held-copy release scenario has not yet passed the complete desktop gate.

The third run failed before the copy scenario could start. Windows Defender
quarantined the external fixture binary/copies with
`Trojan:Win32/Bearfoos.B!ml` (ThreatID 2147731849), causing `adb_tracking_failed`.
Read-only Defender detection records confirmed the exact test-owned paths and
successful quarantine. The affected file was not restored, renamed/rebuilt to
evade detection, excluded or executed again. This finding is not classified here
as a false positive. The external fixture is never packaged in TakeDock.
Resolve the detection/provenance question and rerun the unchanged four-case
gate in a trusted environment before claiming release acceptance.

Earlier isolated observations remain part of the record: one Linux guarded
delete test timed out in an earlier parallel workspace run; its isolated run and
later full workspace runs passed without an inferred product fix. One existing
VideosPanel UI test exceeded its original 5000 ms deadline during concurrent
compilation; its isolated run and final full UI run passed without raising the
deadline. These do not establish a flake-free suite.

The external preservation proxy passed four unit and two Windows real-process
tests, including exact copied bytes and refusal after failed read/hash/decode.
The test decoder is a fixture peer; actual ffmpeg decoding and Android watch
delivery still require the [coordinated physical acceptance](../testing/current-take-discard.md).
The proxy refuses unallowlisted identities and backs up/fully decodes dedicated
clips before starting the remote five-second permit wait.

The implementation remains on `abdo/current-take-discard-implementation` in its
managed worktree. Local test logs are retained in this plan's ignored workspace;
the latest failed desktop run is not a successful `acceptance.json` report.
