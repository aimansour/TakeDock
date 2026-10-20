# Current-Take Discard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user confirm deletion of the current TakeDock-started take with a checkbox, then stop and delete its associated finalized video without delaying ordinary capture commands.

**Architecture:** A pure Rust take/discard machine lives under the recording model's existing authoritative lock. An independent helper watcher supplies pre-Start file evidence; metadata inspection and FileService perform external work without holding that lock. Deletion uses an explicit authorization handshake so a cancelled request cannot be revived by a queued worker or late observation.

**Tech Stack:** Existing Rust 1.98.1/Tauri 2, Svelte 5/TypeScript, libc on Android/Linux, real-process ADB fixtures, and the four existing WebdriverIO/WebView2 release cases. No new third-party dependency.

**Spec:** [Approved current-take discard design](../specs/2026-10-20-current-take-discard-design.md). Also read the [fast-Stop evidence](../../research/2026-10-19-recording-readiness-acceptance.md#fast-stop-finding-and-cleanup).

**Status:** User approved Native execution on 2026-10-20. Tasks 1–6 are implemented and verified. Task 7 implementation/software checks are present, but its four-case desktop gate is blocked by Windows Defender quarantining the external test fixture; physical acceptance also remains pending. See [actual acceptance evidence](../../research/2026-10-20-current-take-discard-acceptance.md). Task 7 is not complete.

## Global Constraints

- English: **Delete this take**. Arabic: **حذف هذه المحاولة**.
- Unchecked primary button: **Stop** / **إيقاف**. Checked primary button: **Stop and delete** / **إيقاف وحذف**. These supersede the existing Stop video/إنهاء الفيديو label in this panel; update its name assertions deliberately.
- The checkbox is window-local, initially unchecked, and resets for a new take, disappearing controls, leaving Recording, or generation change. Accepted operations are process-shared and idempotent per take.
- Checking a box has no backend effect. Activating Stop and delete immediately invalidates readiness and submits one explicit discard intent. No automatic replacement Start.
- Ctrl+Shift+R retains ordinary Start/Stop semantics and never deletes; Enter/Space on the primary button honor its checkbox.
- No TTS, live regions, confirmation dialog, unsolicited announcement, disabled inapplicable controls, or unsolicited focus changes. Preserve primary DOM identity and logical focus if the checkbox disappears.
- Associate only a TakeDock Start from confirmed idle with a complete pre-dispatch baseline, one new supported video, continuous device/inode identity, and the same generation. No newest-file selection or late eligibility recovery.
- The approved environmental assumption is Open Camera saving to `/sdcard/DCIM/OpenCamera` as its sole video writer. Filesystem correlation cannot authenticate its author; do not claim it can.
- Multiple candidates, splits, replacement of an old entry, unexplained activity, observation gaps, watcher overflow/loss, or unsupported observation retain the files. No retroactive adoption of an already running recording.
- A discard sends its single initial Stop no earlier than **600 ms after fresh Start confirmation**. Ordinary Stop remains immediate. Never retry a capture toggle automatically.
- Require fresh confirmation of this discard's own Stop, then an unchanged finalized identity across **two inspections at least 250 ms apart**. Finalization expires **10 seconds after confirmed Stop**; Start/Stop use existing `verification_ms`.
- Later ordinary capture intent, a new take, generation/eligibility loss, observer failure, or shutdown cancels uncommitted effects. No restart, recovery observation, or new window resumes deletion.
- Capture dispatch never waits for watchers, metadata inspection, hashing, file queues, sounds, or remote I/O. No session/capture lock spans a helper operation.
- Reuse the existing no-symlink root, journaled identity-checked delete, mutation serialization and media indexing. No raw `rm`, arbitrary frontend path, settings migration, permission change, or dependency upgrade.
- Keep exactly four mandatory desktop acceptance cases and existing release gating; extend their assertions. No release tag, publication, or personal-phone deletion belongs to unattended execution of this plan.

## Review Focus

1. A watcher can inherit the helper CLI's lifetime mutation lock and stall all file jobs; prove read-only watch/inspect bypass it in Task 1 and concurrent hashing in Task 3.
2. A cold, stale or incomplete baseline can become available after Start; that take must stay ineligible rather than adopting old/unrelated footage. Pin in Tasks 2 and 5.
3. A permit sent before a transport loss may already have deleted the file; never report it as safely retained. Pin the authorization/result boundary and `outcome_unknown` in Task 4.
4. Ordinary capture cancellation can be undone by queue-failure rollback, or a queued discard Stop can toggle after a newer intent. Pin permanently invalidated tickets and writer-side checks in Task 5.
5. A discard's own journal/claim events and terminal results can cause spurious cancellation or duplicate sound across windows. Pin exact authorized-operation handling in Tasks 2/5 and multi-window rendering in Tasks 6/7.

## File and Interface Map

| File | Responsibility |
| --- | --- |
| `file-helper/src/watch.rs` (new) | Versioned read-only watch protocol, identity manifest/inspect, Linux/Android event loop. |
| `file-helper/src/guarded.rs` (new) | Prepared/permit/result protocol and guarded invocation of existing deletion. |
| `file-helper/src/{lib,main}.rs` | Export modules, anchored inspection, route read-only commands before lifetime mutation locking. |
| `src-tauri/src/discard.rs` (new) | Pure take association, discard deadlines, effect tickets and wire snapshot types. |
| `src-tauri/src/helper.rs` (new) | Extract existing Remote deployment/quoting/inspection primitives for both FileService and watcher transport. |
| `src-tauri/src/file_watch.rs` (new) | Independent watcher process, bounded parsing, inspection worker and shutdown. |
| `src-tauri/src/{files,adb}.rs` | Serialized guarded deletion, duplex process handling; retain ordinary job contracts. |
| `src-tauri/src/{model,recording,session,events,desktop,lib}.rs` | Shared snapshot, causality/worker integration, narrow IPC and one terminal feedback owner. |
| `src/lib/{types,ipc,session.svelte}.ts`, `src/components/RecordingPanel.svelte`, `src/lib/i18n.ts`, `src/App.svelte` | Typed state, local confirmation, bilingual accessible UI and ordinary shortcut routing. |
| `tests/fixtures/adb/src/main.rs`, `tests/fixtures/adb/tests/{file_watch,discard}.rs` (new tests) | External-process watch/handshake simulation and integration evidence. |
| `src/components/RecordingPanel.test.ts`, `src/lib/session.test.ts` | Checkbox lifecycle, IPC causality and accessibility assertions. |
| `tests/desktop/{fixtures,critical.e2e}.ts` | Real EXE/WebView2 assertions within the existing four cases. |
| `tests/fixtures/adb/src/bin/device-discard-proxy.rs` (new, unbundled) | Explicitly invoked physical-test ADB proxy that backs up an allowlisted owned take before forwarding guarded deletion. |
| `docs/testing/current-take-discard.md` (new), bilingual setup/keyboard docs | Supported assumptions, test procedure, actual acceptance and limits. |

## Task 1: Read-only helper evidence without blocking file mutations

**Files:** Create `file-helper/src/watch.rs`; modify `file-helper/src/{lib,main}.rs`; unit tests in `watch.rs` and Linux/Android-gated tests in the helper crate.

**Interfaces:** Export `ManifestEntry { name: String, identity: Identity }`, `WatchFrame { version: u32, stream: String, sequence: u64, body: WatchBody }`, and `WatchBody::{BaselineBegin, BaselineEntry { entry: ManifestEntry }, BaselineEnd, Created { entry: ManifestEntry }, Changed { entry: ManifestEntry }, Closed { entry: ManifestEntry }, Removed { name: String }, Lost { code: String }, Heartbeat}`. Serialize the body with `#[serde(tag = "type", rename_all = "snake_case")]`. Export `Root::inspect(&self, name: &str) -> Result<Entry, String>`, `Root::manifest(&self) -> Result<Vec<ManifestEntry>, String>`, `watch::decode_frame(line: &[u8]) -> Result<WatchFrame, String>`, and `watch::run(root: &Root, stream: &str, output: &mut impl Write) -> Result<(), String>`. CLI: `watch STREAM` emits flushed JSON lines; `inspect NAME` emits one JSON `Entry`.

- [x] **Step 1: Add failing protocol/read-only tests.** Define unit-test `fn manifest_entry(name: &str, inode: u64) -> ManifestEntry` with device=1, the supplied inode, size=0 and both times=0. Pin the wire encoding:

```rust
#[test]
fn watch_protocol_rejects_truncation_and_unknown_version() {
    assert!(decode_frame(b"{\"version\":1").is_err());
    assert!(decode_frame(br#"{"version":2,"stream":"s","sequence":1,"body":{"type":"heartbeat"}}"#).is_err());
    let frame = decode_frame(br#"{"version":1,"stream":"s","sequence":1,"body":{"type":"heartbeat"}}"#).unwrap();
    assert_eq!((frame.stream.as_str(), frame.sequence), ("s", 1));
}
```

Add `watch_does_not_take_the_mutation_lock` (hold `Root::lock()` on another handle; watch still delivers baseline), `inspect_rejects_symlink_or_changed_source` (a replacement/change cannot yield a ready entry with the previous identity), `baseline_race_never_publishes_a_complete_manifest` (mutate during enumeration; no trusted end marker), and `linux_watch_reports_creation_close_and_loss` (real temporary-directory writes; root move/loss terminates trust). The Linux tests exercise the actual syscalls, not a mock event list.

- [x] **Step 2: Run RED.** Run `cargo test -p takedock-files --locked watch`. Expected: compilation/test failure for missing new interfaces/assertions, with existing helper tests unchanged. On Windows, explicitly record that Linux syscall tests require the existing Ubuntu CI job; a gated test is not local Linux evidence.
- [x] **Step 3: Implement the protocol and read-only commands.** Parse CLI arguments before acquiring the existing `_lock`. `watch` and `inspect` neither call `lock()` nor run recovery; ordinary list/read/hash/rename/delete retain their current lock/recovery behavior. `inspect` brackets finalization reading with two exact stats and fails on changes; it does not accept a caller's readiness flag. Enumerate through the opened directory descriptor on Linux/Android, and reject symlink/nonregular/invalid-name entries rather than following them.

Use existing libc inotify on Linux/Android; non-Linux production watch returns `file_watch_unsupported`. Install the watch before enumerating a baseline, and invalidate/restart the baseline if mutation events arrive during enumeration. Watch create/modify/close-write/attribute/delete/move/root loss; moves, overflow and root replacement are `Lost`, never guessed creates. Ignore read/open events. Bound each line to 64 KiB, a baseline to 10,000 entries, and baseline retries to three; excess/unknown events fail closed. Emit a sequenced heartbeat every 1000 ms without querying accessibility. Escape filenames through JSON. Existing reserved `.takedock-` metadata is excluded from candidates; do not follow it. Use an open-directory reference for the watch so replacing the path cannot silently redirect it. Linux event semantics and loss handling are defined by the [Linux inotify manual](https://man7.org/linux/man-pages/man7/inotify.7.html).

- [x] **Step 4: Run GREEN and regression.** Run `cargo test -p takedock-files --locked` and `cargo fmt --all --check`. Expected: all applicable tests pass; Linux watcher tests also pass under the existing Ubuntu Rust CI gate before final acceptance. Build all three Android helper ABIs in Task 7.
- [x] **Step 5: Commit.** Stage only this task's helper files; commit `feat: observe take file evidence without holding mutation locks`.

## Task 2: Pure take association and discard state machine

**Files:** Create `src-tauri/src/discard.rs`; export it in `src-tauri/src/lib.rs`. Keep this task independently testable before session integration.

**Interfaces:** Define `TakeToken { generation: u64, start_sequence: u64, id: String }`; `DiscardSnapshot { take: Option<TakeToken>, eligible: bool, reason: String, operation_id: Option<String>, phase: DiscardPhase }`; and snake-case `DiscardPhase::{Unavailable, Tracking, WaitingToStop, ConfirmingStop, Finalizing, Deleting, Completed, Retained, OutcomeUnknown}`. Generation/revision come from the enclosing SessionState. Define `FileEvidence { stream: String, sequence: u64, complete: bool, received_ms: u64, entries: Arc<BTreeMap<String, Identity>> }`, `DiscardReceipt { generation: u64, take: TakeToken, operation_id: String }`, and `DiscardEffect::{SendStop { operation_id: String }, Inspect { operation_id: String, name: String }, Delete { operation_id: String, video: Video }}`.

`DiscardMachine::new(generation: u64) -> Self`; `start_queued(&mut self, take: TakeToken, evidence: Option<FileEvidence>, confirmed_idle: bool, now_ms: u64)`; `file_frame(&mut self, frame: &WatchFrame, now_ms: u64)`; `verification(&mut self, result: &VerificationResult, now_ms: u64)`; `request(&mut self, take: &TakeToken, now_ms: u64, verification_ms: u64) -> Result<DiscardReceipt, String>`; `ordinary_intent(&mut self, action: RecordingAction)`; `stop_queued(&mut self, operation_id: &str, sequence: u64)`; `inspected(&mut self, operation_id: &str, entry: Result<Entry, String>, now_ms: u64)`; `poll(&mut self, now_ms: u64) -> Vec<DiscardEffect>`; `cancel(&mut self, code: &str)`; `authorize_delete(&mut self, operation_id: &str) -> bool`; `finish_delete(&mut self, operation_id: &str, outcome: DeleteOutcome)`; `snapshot(&self) -> DiscardSnapshot`. `DeleteOutcome::{Deleted, Retained(String), Unknown(String)}` is declared here for the worker callback in Task 4. Match all generation/sequence/token boundaries before state changes.

- [x] **Step 1: Write the clock/association tests.** Define test-only `managed_take() -> (DiscardMachine, TakeToken)` with generation=7, Start sequence=1, complete empty baseline received at 0, queued Start at 1, and a confirmed Start result received at 100. Define `created(name, inode, seq) -> WatchFrame` from Task 1's types. Tests use numeric clock inputs, never sleeps. A representative boundary assertion is:

```rust
#[test]
fn discard_stop_is_requested_once_at_the_600ms_boundary() {
    let (mut machine, take) = managed_take();
    machine.request(&take, 101, 10_000).unwrap();
    assert!(machine.poll(699).is_empty());
    let effects = machine.poll(700);
    assert!(matches!(effects.as_slice(), [DiscardEffect::SendStop { .. }]));
    assert!(machine.poll(701).is_empty());
}
```

Add `late_or_stale_baseline_never_adopts_current_take` (missing/incomplete/stale by >2500 ms at Start remains unavailable after later baseline), `unconfirmed_start_expires_without_stop`, `duplicate_request_returns_same_operation`, `only_own_confirmed_stop_can_finalize`, `pause_resume_preserves_tracking_but_cancels_pending_discard`, `multiple_or_replaced_files_are_retained`, `watch_sequence_gap_permanently_invalidates_take`, `second_inspection_requires_250ms_and_exact_identity`, `finalization_expires_at_10000ms`, `cancelled_authorization_cannot_be_revived`, `authorized_delete_result_is_not_rewritten_as_retained`, and `own_delete_events_do_not_cancel_an_authorized_operation`. Assert expected phase, count of effects, eligible flag, and retained/unknown reason in each named test.

- [x] **Step 2: Run RED.** Run `cargo test -p takedock --lib --locked discard::`. Expected: missing new state machine or failed assertions.
- [x] **Step 3: Implement the pure machine.** Snapshot the immutable baseline with `Arc::clone`, never clone a directory map under a capture lock. Track candidate identity by device/inode while size/times evolve; final inspections require equality of the full identity. Treat noncandidate mutation, a second candidate, rename/split, observation gap, or stream loss as permanent ineligibility for that take. An existing-file change is not a new candidate. Permit a request while Start awaits confirmation, but never arm Stop before its matching confirmation. Preserve association through ordinary Pause/Resume; ordinary Stop/New Start cannot turn a later file into this take. Every pending effect is operation-bound and emitted once until an explicit result advances it. An authorized delete is an irreversible attempt; its result may be Deleted, Retained only with explicit no-delete evidence, or Unknown, never a fabricated cancellation success.
- [x] **Step 4: Run GREEN.** Run `cargo test -p takedock --lib --locked discard::` then `cargo test -p takedock --lib --locked`. Expected: all new deterministic tests and existing backend tests pass.
- [x] **Step 5: Commit.** Commit `feat: model current take association and discard causality` with only this task's module/export.

## Task 3: Independent watcher deployment, transport and inspection

**Files:** Create `src-tauri/src/{helper,file_watch}.rs`, `tests/fixtures/adb/tests/file_watch.rs`; modify exports, `src-tauri/src/files.rs`, and `tests/fixtures/adb/src/main.rs`.

**Interfaces:** Extract the existing `Remote` deployment/command/output/stat behavior into crate-private `HelperClient`, preserving signatures and existing file-job behavior; keep job-specific cancellation in FileService. New public `FileWatch::start(lease: DeviceLease, resources: PathBuf, clock: Arc<dyn Fn() -> u64 + Send + Sync>, callback: Arc<dyn Fn(WatchUpdate) + Send + Sync>) -> Self`, `inspect(&self, operation_id: String, name: String) -> Result<(), String>`, and `shutdown(&self)`. `WatchUpdate::{Frame { frame: WatchFrame, evidence: Option<FileEvidence> }, Inspection { operation_id: String, entry: Result<Entry, String> }, Lost(String)}`. The clock returns milliseconds from the owning Engine's monotonic origin; tests supply their own origin. Start is local worker creation and returns without deployment/ADB I/O. No Engine interface changes yet.

- [x] **Step 1: Add real-process transport tests.** Build a fixture directory with copied fixture ADB, the existing ABI/helper resource layout, a known old video, and separate observer/capture fixtures when needed. Add:

```rust
// Assertions in the named transport tests, using the public FileWatch API:
assert!(baseline_arrived_before_hash_release);
assert_eq!(old_identity_from_frame, identity_read_from_fixture_root);
assert_eq!(inspection_operation_id, "op-1");
assert!(loss_received_after_truncated_or_gapped_stream);
assert!(watcher_children_exited_after_shutdown);
```

The test names are `watch_starts_while_hash_holds_mutation_lock`, `watch_and_inspect_preserve_unicode_identity`, `malformed_frame_and_heartbeat_loss_fail_closed`, and `shutdown_cancels_deployment_and_reaps_processes`. Define these assertions against callback-collected updates and fixture markers, with bounded receive deadlines. Hold/release fixture workers with explicit files and `finally`/Drop cleanup, not timing guesses.

- [x] **Step 2: Run RED.** Run `cargo test -p takedock-adb-fixture --test file_watch --locked`. Expected: missing FileWatch/new fixture protocol, then the listed assertions fail before transport implementation.
- [x] **Step 3: Implement extraction and transport.** Preserve the existing helper SHA-256 resource validation, unique temporary push and ABI allowlist. Independently deploy/start `exec-out <quoted-helper watch STREAM>`; one reader consumes bounded lines and validates version, stream, consecutive sequences and baseline structure. A stream UUID is new for every process. Maintain the immutable idle manifest off the model lock. Reject >64 KiB lines, >10,000 baseline entries, invalid names/identities, unknown frames, sequence gaps and missing heartbeat for >2500 ms. Separate stderr draining and child supervision must not let a blocked read prevent shutdown. Bounded callback backpressure invalidates trust rather than dropping an event silently.

For each valid frame, assemble the resulting immutable FileEvidence off the model lock and deliver it with that same frame in one callback, preserving stream order. `complete` becomes true only after BaselineEnd; a heartbeat updates `received_ms` without inventing file changes. Each mutation updates the manifest before publication. Malformed/lost input publishes no evidence. This prevents Start from combining a newer frame with an older independently published baseline.

Use a separate bounded inspection worker (capacity 8, duplicate operation/name requests coalesced) for read-only `inspect NAME`; do not place it behind FileService's copy/hash queue or pause reading the watch stream. A failed deployment/watch leaves ordinary capture and Videos operational. Restart a failed watcher only as background preparation for a future confirmed-idle take; an old take never regains eligibility. The external fixture implements the same frames on Windows using its private control files; actual Linux watcher coverage remains Task 1's real-syscall tests.
- [x] **Step 4: Run GREEN and preserve file behavior.** Run `cargo test -p takedock-adb-fixture --test file_watch --locked` and `cargo test -p takedock-adb-fixture --test files --locked`. Expected: watch/inspection progresses while a held file operation owns the mutation lock; all existing file integrity cases still pass.
- [x] **Step 5: Commit.** Commit `feat: stream take file evidence independently of transfer jobs` with this task's files.

## Task 4: Serialized deletion with a cancellable authorization boundary

**Files:** Create `file-helper/src/guarded.rs`; modify helper exports/CLI, `src-tauri/src/{files,helper}.rs`, fixture helper dispatch, and `tests/fixtures/adb/tests/discard.rs`.

**Interfaces:** Add `Root::delete_guarded(&self, name: &str, expected: &Identity, permit: impl FnOnce() -> Result<bool, String>) -> Result<(), String>`. It requires caller-owned mutation serialization, validates readiness/identity before requesting permit, and invokes the existing journaled `delete` only after permit and revalidation. CLI `delete-guarded NAME EXPECTED_JSON OPERATION_ID` sends `{ "type":"prepared", "operation_id":... }`, awaits exactly one `{ "type":"permit", "operation_id":..., "allow":true|false }`, then emits `{ "type":"result", "operation_id":..., "status":"deleted"|"retained", "code":... }`. Define these serde message types in `guarded.rs`; no secret capabilities enter frontend state.

Add `FileService::start_discard_delete(&self, lease: DeviceLease, video: Video, operation_id: String, authorize: Arc<dyn Fn() -> bool + Send + Sync>, complete: Arc<dyn Fn(DeleteOutcome) + Send + Sync>) -> Result<String, String>`. It queues behind ordinary mutations, uses one video only, and supplies final outcome once. Add a private job origin carrying the discard operation so its generic `file-job` event does not own sounds or overwrite the discard outcome.

- [x] **Step 1: Write failing guarded deletion tests.** Unit test the helper using a finalized temporary fixture and its actual identity; fixture test the duplex process. Include these exact assertions:

```rust
// rejected_permit_keeps_original_and_creates_no_claim
assert!(root.delete_guarded("take.mp4", &identity, || Ok(false)).is_err());
assert_eq!(root.stat("take.mp4").unwrap(), identity);
// source_replaced_during_preparation_is_retained
assert!(result.is_err());
assert_eq!(read_replacement_bytes(), replacement_bytes);
// authorized_delete_with_lost_result_is_unknown
assert!(matches!(outcome, DeleteOutcome::Unknown(_)));
```

Define `read_replacement_bytes()` as a test closure over that temporary file. Add `permit_eof_wrong_operation_and_timeout_never_delete`, `cancel_before_worker_authorization_keeps_original`, `successful_handshake_deletes_only_exact_identity`, `guarded_delete_is_serialized_with_ordinary_jobs`, and `late_cancellation_cannot_misreport_a_committed_delete`. Hold at prepared/permit/result using fixture-only markers. Record that a transport loss after an affirmative authorization is Unknown unless a positive result establishes the actual outcome.
- [x] **Step 2: Run RED.** Run `cargo test -p takedock-files --locked guarded` and `cargo test -p takedock-adb-fixture --test discard --locked`. Expected: missing helper/service interface or the corresponding failed behavior assertions.
- [x] **Step 3: Implement guarded helper and worker.** Route guarded deletion through the existing mutation lock/recovery and no-symlink root. The remote permit wait is bounded to 5000 ms; EOF, malformed input, wrong operation, false permit or timeout aborts before the journaled claim. Recheck finalized exact identity after permit. Do not hold host model locks during this wait or remote I/O.

Use a fresh duplex `adb shell -T <quoted helper command>` with stdin piped for the handshake, rather than adding it to the capture shell or using one-way exec-out. ADB documents stdin and `-T` in its [primary client source](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/client/commandline.cpp). Validate the prepared operation before calling `authorize`. That callback atomically checks the current generation/take/phase under the model lock and marks the irreversible attempt as authorized before returning; release the lock before sending permit. Earlier cancellation makes authorization fail permanently. Later cancellation must not claim that an authorized remote mutation was prevented. Bound the whole helper interaction to 30 seconds, drain stderr, reap the process and deliver one outcome. A negative/no authorization proves retention; a successful delete result proves deletion; a lost acknowledgement, helper error after possible unlink, or interrupted affirmative permit is Unknown. Do not retry or resume. A read-only reconciliation can report evidence, but absence of a file alone is not proof this operation deleted it.

Keep normal copy/move/delete/rename contracts unchanged. The discard callback owns terminal feedback/result; ordinary generic file-job updates remain useful for internal progress but must carry `discard_operation_id` so consumers can distinguish their owner. Index refresh after successful deletion uses the existing broadcast path.
- [x] **Step 4: Run GREEN.** Run `cargo test -p takedock-files --locked` and `cargo test -p takedock-adb-fixture --test discard --test files --locked`. Expected: all guarded, recovery and ordinary file integrity tests pass, including retained originals for every preauthorization failure.
- [x] **Step 5: Commit.** Commit `feat: authorize current-take deletion at a guarded commit boundary`.

## Task 5: Integrate take causality, safe Stop scheduling and shared results

**Files:** Modify `src-tauri/src/{model,recording,session,events,desktop}.rs`, fixture capture behavior, and `tests/fixtures/adb/tests/discard.rs`.

**Interfaces:** Add `SessionState.discard: DiscardSnapshot` with unavailable default, plus optional `discard_operation_id` on VerificationResult/Pending and FileService job events. Add `Engine::start_with_files(settings: Settings, apk: PathBuf, resources: PathBuf, files: Arc<FileService>, sink: EventSink) -> Result<Self, String>`; preserve existing `Engine::start(settings, apk, sink)` for ordinary engine users/tests, with discard unavailable if no file service was supplied. Runtime owns `Arc<FileService>` and uses the new constructor.

Add `Engine::request_discard(&self, take: TakeToken, generation: u64) -> Result<DiscardReceipt, String>` and the sole new frontend command `discard_take(take: TakeToken, generation: u64) -> Result<DiscardReceipt, String>`. It accepts/invalidates readiness immediately without any file/verification wait. RecordingMachine also exposes internal `withdraw_discard_stop(&mut self, operation_id: &str) -> Option<u64>` to remove only a still-uncommitted discard Stop and correct its pending/predicted state before validating a newer ordinary intent. Never reuse a withdrawn command sequence. `discard-result` payload is `{generation, operation_id, status: "completed"|"retained"|"outcome_unknown", code}` and is emitted once per operation. All windows get the enclosing revisioned `session-state`; bootstrap automatically contains the current discard snapshot.

- [x] **Step 1: Add the causal integration tests before wiring.** Use the new constructor with real fixture processes and the Task 3/4 marker controls. Add `discard_cancels_ready_cue_before_waiting_for_start`, `one_stop_after_confirmation_and_600ms`, `ordinary_stop_remains_immediate`, `no_delete_until_own_stop_and_two_stable_inspections`, `paused_take_can_be_discarded`, `missing_baseline_at_start_cannot_be_repaired_later`, `duplicate_window_requests_share_one_stop_and_delete`, `new_intent_cancels_writer_queued_discard_stop`, `new_start_after_withdrawn_stop_is_revalidated_without_a_wrong_toggle`, `queue_failure_cannot_restore_cancelled_discard`, `disconnect_or_observer_error_retains_uncommitted_take`, `new_start_during_held_delete_authorization_retains_old_file`, and `held_copy_does_not_delay_ordinary_capture`.

```rust
// Core assertions against collected commands, snapshots and actual files:
assert_eq!(discard_stop_count, 1);
assert!(delete_calls_before_matching_stop_confirmation.is_empty());
assert_eq!(newer_take_bytes, expected_newer_take_bytes);
assert_eq!(terminal_results_for_operation.len(), 1);
assert_eq!(feedback_events_for_operation.len(), 1);
```

Add unit tests in Events proving that a discard-owned Stop confirmation and file-job completion are silent, its single terminal outcome owns feedback, stale generations are ignored, and an operation already sounded does not sound again when another window bootstraps. Retain ordinary capture/file feedback tests.
- [x] **Step 2: Run RED.** Run `cargo test -p takedock-adb-fixture --test discard --locked` and `cargo test -p takedock --lib --locked events::`. Expected: new construction/IPC integration is missing or the named causality/feedback assertions fail.
- [x] **Step 3: Integrate under one authoritative model lock.** RecordingMachine owns DiscardMachine and its latest FileEvidence. Task 3's callback applies the frame and its accompanying immutable evidence together under that lock. A successful Start queue transaction captures confirmed-idle eligibility (`observed == Idle`, `predicted == Idle`, no pending commands, observer/foreground/video-mode ready), generation and command sequence before any shell write can create the file. Keep full directory serialization and event emission outside the lock. Failed Start enqueue restores ordinary prediction but does not resurrect cancelled discard authorization. Feed matching verification results, error/unknown/eligibility transitions and the 25 ms timer into the machine. Known/unknown loss before Start confirmation retains the existing verification deadline; any postconfirmation observation gap invalidates deletion.

The discard's Stop uses a private `CaptureOrigin::Discard { operation_id }`; ordinary requests use `CaptureOrigin::User`. Extend QueuedCommand with the origin and a shared ticket with states Pending/Cancelled/WriteCommitted. Before writing a discard-origin toggle, the writer atomically checks Pending and marks WriteCommitted under the same ordering lock as ordinary intent invalidation; release the lock before the pipe write/flush, which can block. A write already authorized before a later intent may finish in FIFO order; a ticket cancelled first never writes. An ordinary intent first withdraws an uncommitted discard Stop and recomputes prediction/pending state, then validates its own transition. A frontend Start based on the withdrawn speculative idle state may consequently be rejected: it must never be transmitted as a volume toggle that actually stops the still-running camera. Reconcile authoritative state and require a fresh explicit action; do not retry it automatically. Mark Stop sequence ownership only when its queue transaction succeeds. No automatic second Stop on delivery/verification failure. Do not clone mutable authorization state into queue rollback snapshots; cancelled tickets remain cancelled across rollback.

An independent effect worker handles watcher startup, inspections and delete queueing; the timer/IPC use bounded nonblocking effect submission. Queue rejection retains the file and surfaces an ordinary result. Inspect first after matching confirmed Stop; schedule the next request at least 250 ms after the first result, and stop all finalization work at +10000 ms. A delayed queued delete still revalidates current state in Task 4's authorization callback. Use Weak references in callbacks to avoid Engine/FileService cycles. Register watcher/inspection cleanup explicitly; preserve the existing teardown assumption that the Observer process is the Observer, rather than relying on a new last-process entry being it.

Deliver DeleteOutcome back to the machine, preserving an accurate authorized/unknown outcome after later intent. Once authorized, ignore only this operation's expected claim/removal for cancellation purposes; do not grant permission to another candidate. Add typed discard ownership to Events and generic job consumers. Exactly one discard terminal result owns optional success/failure feedback; suppress both its Stop verification feedback and associated generic file-job feedback. Never replay readiness on recovery or window bootstrap.
- [x] **Step 4: Run GREEN and regression.** Run `cargo test -p takedock-adb-fixture --locked` and `cargo test -p takedock --lib --locked`. Expected: the new real-process causality tests and existing session/file/updater/audio tests all pass. Run `cargo clippy --workspace --all-targets --features desktop --locked -- -D warnings`; expected zero warnings with real resources present (build resources in Task 7 if absent).
- [x] **Step 5: Commit.** Commit `feat: integrate generation-bound discard with capture and file workers`.

## Task 6: Accessible bilingual checkbox and frontend reconciliation

**Files:** Modify `src/lib/{types,ipc,session.svelte}.ts`, `src/lib/i18n.ts`, `src/components/RecordingPanel.svelte`, `src/App.svelte`, `src/components/RecordingPanel.test.ts`, `src/lib/session.test.ts`, and name assertions in existing UI tests where needed.

**Interfaces:** Mirror the Task 2/5 snapshot and receipt types in TypeScript; JobEvent gains optional `discard_operation_id?: string | null`. Add `ipc.discard(take: TakeToken, generation: number): Promise<DiscardReceipt>` invoking `discard_take`. Add `createSession().discard(): void` that synchronously captures the backend take/generation, invokes IPC from activation and reports failure only if that take/generation is still current; it does not predict deletion or send a separate Stop. RecordingPanel adds `onDiscard: () => void` and owns a local `deleteTake` boolean. Ordinary `onAction` and App keyboard routing remain unchanged.

- [x] **Step 1: Write the component/controller RED tests.** In RecordingPanel tests extend the existing ready fixture with a Task 2-shaped eligible take and add these assertions:

```typescript
const checkbox = screen.getByRole('checkbox', { name: 'Delete this take' });
expect(checkbox).not.toBeChecked();
const primary = screen.getByRole('button', { name: 'Stop' });
await user.click(checkbox);
expect(screen.getByRole('button', { name: 'Stop and delete' })).toBe(primary);
expect(onDiscard).not.toHaveBeenCalled();
await user.click(primary);
expect(onDiscard).toHaveBeenCalledTimes(1);
expect(onAction).not.toHaveBeenCalled();
```

Use `userEvent.setup()` from the installed testing library. Add `checkbox_resets_for_take_generation_and_remount`, `checkbox_selection_is_local_to_each_window`, `disappearing_checkbox_preserves_logical_focus`, `arabic_names_and_rtl_use_native_controls`, `readiness_and_checkbox_have_no_live_regions`, and `ordinary_shortcut_never_uses_discard_even_when_checked`. Assert Arabic names **حذف هذه المحاولة**, **إيقاف**, **إيقاف وحذف** and stable primary DOM identity. Render App for the shortcut test using the existing IPC mocks; render two panels for local-checkbox isolation. Controller tests capture the exact `{take,generation}` IPC request, stale receipt rejection, duplicate click reconciliation, authoritative shared snapshots, and one terminal refresh.
- [x] **Step 2: Run RED.** Run `npm run test:ui -- src/components/RecordingPanel.test.ts src/lib/session.test.ts`. Expected: missing checkbox/onDiscard/IPC behavior fails; existing assertions that expect the old stop name are updated to the spec's exact approved copy within this task.
- [x] **Step 3: Implement the minimal rendering/controller changes.** Show the checkbox only for a supported tracked recording/paused take with no accepted discard in progress. Otherwise keep ordinary Stop usable and show the backend ineligibility reason. Remove checkbox controls when inapplicable; never disable the primary capture path. Use the primary button's direct event handler to call onDiscard only when checked; choosing an inapplicable/stale take cannot fall back to deleting another take. On accepted discard, further primary activation is ordinary capture and can cancel pending uncommitted effects; render ordinary text explaining progress. Clear selection on all specified boundaries. Use the existing pre-effect/tick focus pattern only when an existing focused control disappears, not on background progress. Leaving the Recording view unmounts/resets the checkbox.

Add bilingual ordinary text for each phase/reason and for `discard_outcome_unknown`. Add the exact shortcut help **Deletion requires the Stop and delete button; Ctrl+Shift+R only starts or stops recording.** / **الحذف يتطلب تفعيل زر إيقاف وحذف؛ الاختصار Ctrl+Shift+R يبدأ التسجيل أو يوقفه فقط.** Add the assumption help **Automatic deletion requires Open Camera to save here with no other app adding videos during the take.** / **الحذف التلقائي يتطلب أن يحفظ Open Camera في هذا المجلد وألّا يضيف تطبيق آخر فيديوهات أثناء المحاولة.** Keep filenames in `<bdi>` if shown. Consume `discard-result` without frontend sound. Refresh Videos once after completed deletion and drop obsolete selections by identity; generic discard-owned file-job completion must not duplicate the refresh or overwrite Unknown/Retained with a generic completed status. Bootstrap/revision checks prevent older replies from rewinding a shared operation.
- [x] **Step 4: Run GREEN and frontend checks.** Run `npm run test:ui`, `npm run check`, `npm run format:check`, and `npm run build`. Expected: all component/controller tests pass, zero Svelte/type/accessibility warnings, formatting clean and Vite build succeeds.
- [x] **Step 5: Commit.** Commit `feat: add accessible per-take stop-and-delete confirmation`.

## Task 7: Release integration, documentation and owned-device acceptance

**Files:** Modify `tests/desktop/{fixtures,critical.e2e}.ts`, `docs/testing/desktop-e2e.md`, `docs/setup.md`, `docs/setup.ar.md`, `docs/keyboard.md`, `docs/keyboard.ar.md`; create `docs/testing/current-take-discard.md` and unbundled `tests/fixtures/adb/src/bin/device-discard-proxy.rs`. Save raw device evidence only in an ignored task-specific `test-results/` directory.

**Interfaces:** Reuse `restartFixture`, `button`, `shortcut`, `waitFor`, `commands`, `flag` and `inject`. Extend the existing fixture with `take-created`, `hold-watch-baseline`, `hold-delete-prepared`, and `drop-delete-result` markers, plus `delete-calls.jsonl` containing fixture operation/name/identity. These exist only in the external fixture. Preserve exactly four cases and existing acceptance resource hashes. No production hook or native test control is added.

- [ ] **Step 1: Extend release assertions and docs.** Extend the existing recording/focus case with an unchecked checkbox, native keyboard selection, same-node Stop and delete, a completed discard with no surviving owned fixture video, and a second window starting unchecked while sharing operation progress. Check missing baseline leaves ordinary Stop usable and no delete request. Add retained/unknown output assertions using held authorization/new Start and lost-result fixtures; prove the next take's bytes survive. In the existing held-copy case prove recording controls dispatch while deletion waits in the file queue; cancel before releasing that queue and check zero deletion. Update Stop-name assertions; preserve all prior readiness/focus/settings/copy checks and all four test titles.

Document the supported fixed folder/sole-writer assumption, unavailable/retained/unknown meanings, per-window checkbox, ordinary shortcut, no automatic replacement Start, and how to complete explicit deletion in Videos when association is unavailable. Keep English/Arabic user guidance synchronized. Record that split videos are retained for explicit selection, not partially deleted.
- [ ] **Step 2: Build resources and run release checks.** Rebuild the changed helper with `./scripts/build-file-helper.ps1`; reuse a verified unchanged Observer or run `./scripts/build-observer.ps1` with signing compatible with the owned test phone. Do not uninstall an existing Observer to bypass signing. Then run:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --features desktop --locked -- -D warnings
cargo test --workspace --locked
npm run format:check
npm run check
npm run test:ui
npm run build
node --test scripts/release-gate.test.mjs scripts/prepare-publication.test.mjs
npm run desktop:build -- --no-bundle
./scripts/check-updater.ps1
./scripts/setup-desktop-drivers.ps1
npm run test:desktop
```

Expected: every command exits 0, all four actual release desktop cases pass, and the report identifies the tested EXE/helper hashes. Read failure output and diagnose causality rather than relaxing assertions. Run the existing unprivileged desktop wrapper only if the environment requires it. Confirm Ubuntu CI executes the real inotify tests; Windows fixtures are not that evidence.
- [ ] **Step 3: Prepare owned-fixture preservation before physical deletion.** Add the external test-only Rust proxy binary, selected through the existing external-ADB setting only for the physical trial. It forwards ordinary ADB processes/streams directly to an explicitly configured real ADB executable, with no shell interpolation. Before starting a `delete-guarded` invocation, require a private test allowlist matching serial, filename and exact identity derived from a journaled pre-Start manifest and this dedicated test take. Back up the finalized original through a separate real-ADB invocation, verify local/remote SHA-256, unchanged remote identity and full decode, then forward the held guarded-delete command. Do this before spawning the remote permit wait so backup time cannot exhaust its 5000 ms deadline. Keep total invocation within the host's 30-second limit by using short dedicated clips; failure returns before authorization and retains the original. Unknown/unallowlisted targets are rejected, not forwarded. Unit-test that missing/wrong allowlist or failed verification never forwards delete, and that nonmutating watch/capture traffic continues while backup is pending. Run `cargo test -p takedock-adb-fixture --bin device-discard-proxy --locked`; expected PASS after watching these tests fail first. The proxy and its private manifest are never packaged or used as product configuration defaults.
- [ ] **Step 4: Perform separately coordinated real-phone acceptance.** The automated checks above do not authorize creating/deleting recordings on the user's active phone. Coordinate the physical trial with the user when software is ready. Use only dedicated test takes with pre-Start manifests and Step 3's verified backups before deletion. For a normal take, pause, an early request before the 600 ms boundary, and an explicitly held/cancelled discard, record command/observation order, exact file identity, existence/outcome and preserved unrelated fixture bytes. Check actual Android shared-storage watch delivery, root changes, screen-reader coexistence and that ordinary capture remains immediate during a held file job. Restore pre-test settings/accessibility state. Preserve raw clips and identifiers privately; publish only sanitized results. If phone access/user input is absent, record physical acceptance as pending; do not equate fixture success with device verification or silently certify unsupported watches.
- [ ] **Step 5: Commit verified integration and documentation.** Run the Task 7 Step 2 suite after final product changes; commit `test: verify current-take discard release workflow` with test/docs changes. Record completed software gates and physical acceptance separately; Task 7 is not physically complete while its explicitly required device cases remain pending.

## Preflight, final review and handoff

Before implementation, use the selected execution skill's per-plan workspace and ledger. Check interfaces in order: Task 1 protocol to 2/3; Task 2 effects/outcomes to 4/5; Task 3 transport and 4 authorization to 5; Task 5 wire shape/ownership to 6/7. Record rulings against the approved spec, not silent signature changes. Preserve the current design commits. Do not start product implementation on main; establish the execution workspace according to using-git-worktrees and existing user preferences.

After implementation, perform one whole-branch independent review using requesting-code-review, the approved spec, this plan, the five Review Focus items and ledger rulings. Do not open other chats or inspect their history. The reviewer gets only this branch's task materials. If Native execution is selected, implement tasks inline and reserve that one fresh reviewer for the end. Fix Important/Critical findings with a reproducing RED test, a GREEN test and the full suite; report unresolved assumptions and pending physical evidence accurately.

## Plan self-review and execution choice

Coverage checked: local checkbox/reset/focus and ordinary shortcut (Task 6); trusted pre-Start evidence/sole-writer limit and ambiguity (Tasks 1–3); safe one-shot Stop, finalization and loss boundaries (Tasks 2/5); identity-checked serialized deletion and unknown outcomes (Task 4); window-shared results/feedback (Tasks 5–7); bilingual guidance, four actual release cases and separate physical evidence (Task 7). All five Review Focus items have named tests in their owning tasks. No product dependency or unrelated backlog component is added.

Recommendation: **Native**. The seven tasks share causality and wire interfaces, so one inline implementer avoids repeated rediscovery; one independent branch review still checks the deletion boundaries. The user must review this written plan and choose Native or Subagent-driven before implementation.
