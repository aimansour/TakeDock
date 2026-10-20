# Current-take discard local acceptance snapshot — 2026-10-20

This table records local pre-review evidence at `c283d7a`. The user subsequently
authorized GitHub CI; corrected-head software validation is tracked in
[draft PR #19 checks](https://github.com/wip-bunker-42/TakeDock/pull/19/checks).
Physical acceptance remains **incomplete**. No release tag, publication,
installer installation or physical-phone trial was run.

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
| Local actual release desktop suite | Latest local run 3/4; local gate **not passed** |
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

The third local run failed before the copy scenario could start. Windows Defender
quarantined the external fixture binary/copies with
`Trojan:Win32/Bearfoos.B!ml` (ThreatID 2147731849), causing `adb_tracking_failed`.
Read-only Defender detection records confirmed the exact test-owned paths and
successful quarantine. The affected file was not restored, renamed/rebuilt to
evade detection, excluded or executed again. This finding is not classified here
as a false positive. The external fixture is never packaged in TakeDock.
The user approved a fresh source build and the unchanged four-case gate on
GitHub's hosted CI. That validation does not restore or classify the quarantined
local executable; its provenance/detection question remains unresolved.

## Independent source review and corrections

An independent `gpt-6.1-sol` reviewer completed the whole-branch source review
after the initially selected `gpt-6-astra` request failed before review due to a
usage limit. It found three Important issues and no Critical/Minor issues:

- Observer sequence gaps could preserve discard eligibility. Three deterministic
  tests failed before the fix, then passed for tracking, withdrawal of a deferred
  Stop and finalization. Later observations cannot restore the invalidated take;
  ordinary capture still reconciles normally.
- A known successful deletion waited behind media indexing before publication.
  The held-index/disconnect process regression failed on GitHub with
  `outcome_unknown` instead of `completed` in
  [the regression run](https://github.com/wip-bunker-42/TakeDock/actions/runs/39464113365).
  Completion is now published before best-effort indexing. Corrected-head full
  regression evidence belongs to the PR checks, not this earlier local table.
- Visible association help did not name the fixed directory. Both language
  assertions failed before the help named `/sdcard/DCIM/OpenCamera`, then passed.

The reviewer did not certify application-authenticated file authorship, the
quarantined binary's provenance, physical Android/NVDA behavior, real ffmpeg
preservation, a complete desktop pass or root causes of the earlier timeouts.
The executor retains the approved sole-writer assumption, requires real-device
and decoder evidence before physical acceptance, retains local quarantine and
requires green current-head CI. No timeout root cause or flake-free claim is made.
The subsequent transfer/trim/installer/wireless/audit components remain separate
backlog. These limitations are not silently treated as completed work.

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
the latest failed local desktop run is not a successful `acceptance.json` report.
