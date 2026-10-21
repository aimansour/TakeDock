# File results and accessible rename updates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expire clean file-operation successes after ten seconds across windows, retain unresolved outcomes until hidden, and expose confirmed renamed filenames without delaying recording or moving focus.

**Architecture:** A pure Rust result model owns deadlines, outcome revisions and acknowledgements; a small process service exposes fresh snapshots independently of capture and file workers. A frontend controller projects those deadlines locally, while confirmed rename events reconcile exact source identities before media indexing or another queued job. Native controls present notices, active work and collapsed history without automatic speech.

**Tech Stack:** Existing Rust/Serde/Tauri 2.12.1, Svelte 5/TypeScript, Vitest/Testing Library and WebdriverIO release acceptance; no new dependency.

**Spec:** [Approved design](../specs/2026-10-20-file-results-rename-design.md), approved on 2026-10-21. Base product: `4dc44e72498c1f455b4e43b40f8088a040ceb45d`; spec commit: `ea82dd4`. This plan requires review before implementation.

## Global Constraints

- "Completed success leaves the main result/activity presentation after 10,000 ms from its first authoritative terminal result." Use one process-monotonic clock, never wall-clock time.
- "Failed, cancelled/incomplete, retained and outcome-unknown results do not expire automatically." Completed-with-warning is persistent too; later success cannot dismiss an unresolved result.
- "Keep the latest twenty ordinary terminal history entries, plus unresolved non-success/warning entries pinned until acknowledged." Process memory only; a restart clears presentation history.
- "Queued/running file work stays visible and cancellable." Active jobs are not subject to the terminal-history bound.
- Preserve DOM-focused cards until focus leaves, and already displayed history records while the native disclosure is open. Do not claim DOM focus observes NVDA's virtual cursor.
- "Do not add TTS, live regions, alert/status roles, automatic announcements or disabled inapplicable controls." Preserve English/Arabic, RTL, `<bdi>`, stable row nodes and exact identity strings.
- "Recording dispatch remains independent of file jobs, result expiry/dismissal, history rendering, media indexing and directory refresh." Production capture/deletion acquires no backup/hash/full-decode requirement from this work.
- Discard-result owns its terminal notice/audio; its generic delete job supplies progress/details only. Capture/settings messages retain their existing behavior.
- Preserve atomic no-overwrite rename, extension/source checks and all guarded-discard authority. Acknowledging a notice authorizes no device operation.
- "Preserve exactly four mandatory actual release desktop test titles/case count." No production test hooks, direct mutation IPC in place of user actions, skipped gates or relaxed existing timeouts/assertions.
- Retain pinned/locked dependencies: Rust `1.98.1` (crate floor `1.98`), CI Node `24.21.0`, NDK `30.0.16248370`, tauri-driver `2.1.0`, EdgeDriver matching installed WebView2 exactly. No dependency upgrade or lockfile churn.
- Local external ADB fixture is quarantined: do not restore, rebuild/rename to evade, exclude or execute it locally. Run fixture/process regressions on fresh GitHub-hosted runners, as already authorized.
- Phone acceptance uses new, journaled dedicated test clips with verified local backups before mutation; preserve unrelated files/settings/accessibility. Keep wireless pairing/connection active. Prior clips/private plan workspaces are not fixtures or scratch space.
- Reuse the attached linked worktree and `abdo/file-results-rename-design` branch; preserve ignored backups. Execution state belongs only to `.superpowers/sdd/2026-10-21-file-results-rename/`. Do not inspect other chats.

## Review Focus

- A notice expires while a snapshot, duplicate event or acknowledgement is in flight: no revival, deadline extension or accidental dismissal of a newer warning (Tasks 1, 2, 4).
- More than twenty completions arrive while a failure or active job is being read: unresolved/active work remains available and open history does not remove the reader's record (Tasks 1, 2, 6).
- Rename commits immediately before cancellation/disconnection or inspection failure: report the committed destination accurately without guessed metadata, retry or false rollback (Task 3).
- Two rename updates arrive backwards, or an old listing completes after either update: newest verified name/identity reaches the same selected checkbox; replacement objects remain untouched (Task 5, native proof Task 7).
- Another window hides a focused notice, or passive expiry meets keyboard/NVDA navigation: preserve local focus, keep acknowledgement global, supply no unsolicited speech and leave capture responsive (Tasks 4, 6, 7).

## Files and boundaries

| Files | Responsibility |
| --- | --- |
| New `src-tauri/src/file_results/{mod,model,service,tests}.rs` | Public result DTOs; pure lifecycle/normalization; short-lock process facade; deterministic tests. |
| `src-tauri/src/{lib,events,desktop}.rs` | Register the module, preserve active job snapshots/audio, wire result events/bootstrap/narrow IPC. |
| `src-tauri/src/files.rs`, `guarded_delete.rs` | Confirmed rename metadata and per-file name fields; mechanical defaults for existing result constructors. |
| `tests/fixtures/adb/{src/main.rs,tests/files.rs}` | External-only failure/hold controls and real helper/process rename assertions. |
| New `src/lib/file-results.ts`, `file-results.test.ts` | Typed snapshot reconciliation, local monotonic expiry and disposal; no DOM or phone work. |
| New `src/lib/rename-updates.ts`, `rename-updates.test.ts` | Exact-source rename chaining; no focus or IPC effects. |
| `src/lib/{types,ipc,session.svelte,session.test}.ts`, `videos.ts` | Contracts and session integration, including listing invalidation/loading ownership. |
| New `src/components/FileResultsPanel.svelte`, `FileResultsPanel.test.ts` | Ordinary notices, native hide buttons and stable collapsed history. |
| `src/App.svelte`, `App.test.ts`, `components/ActivityPanel.svelte`, `ActivityPanel.test.ts`, `VideosPanel.test.ts`, `lib/i18n.ts`, `styles.css` | One main file-result presentation, active cancellation/progress, labels and layout; no styling framework. |
| `tests/desktop/{critical.e2e,fixtures}.ts` | Extend the existing four native release flows, without changing titles/count. |
| New `docs/testing/file-results-rename.md`, `docs/research/2026-10-21-file-results-rename-acceptance.md`; `docs/{architecture,keyboard,keyboard.ar}.md` | Repeatable acceptance instructions, measured evidence and bilingual user guidance. |

Paths below are repository-relative. Local commands run from the linked worktree with the pinned tools; every command checks its exit status. Fixture commands marked **hosted only** must not execute on this Windows host. Observe a failing regression before implementing each changed behavior; extend already-correct label tests with full-flow evidence instead of inventing a failure.

## Shared contracts

Define result DTOs in Rust `file_results/mod.rs` and `RenameUpdate` in `files.rs`,
mirrored in `src/lib/types.ts` with snake_case wire fields:

```text
ResultKey { generation: u64, owner: ResultOwner(job|discard), operation_id: String }
ResultItem { name: String, status: String, message: String,
             requested_name: Option<String>, destination_name: Option<String> }
FileNotice { key: ResultKey, revision: u64, kind: JobKind,
             outcome: FileOutcome(completed|failed|cancelled|retained|outcome_unknown|warning),
             code: String, warnings: Vec<String>, name: String,
             completed: usize, count: usize, results: Vec<ResultItem>,
             expires_at_ms: Option<u64> }
FileResultSnapshot { revision: u64, now_ms: u64,
                     notices: Vec<FileNotice>, history: Vec<FileNotice> }
RenameUpdate { generation: u64, id: String, source_name: String,
               source_identity: String, video: Video }
```

Outcome revisions belong to one result; snapshot revisions belong to the whole model. `destination_name` exists only after acknowledged mutation, even if subsequent inspection fails; `requested_name` alone never claims success. All generation/identity safety checks stay exact. TypeScript `resultKey(key: ResultKey): string` encodes all three key fields with `JSON.stringify([generation, owner, operation_id])`.

### Task 1: Pure result lifecycle, warnings and bounded history

**Files:** Create `src-tauri/src/file_results/{mod,model,tests}.rs`; modify `src-tauri/src/lib.rs`.

**Interfaces:** Produce `FileResults::receive(&mut self, event: &str, payload: &serde_json::Value, now_ms: u64) -> bool`, `snapshot(&self, now_ms: u64) -> FileResultSnapshot`, `acknowledge(&mut self, key: &ResultKey, revision: u64) -> bool`. A `true` mutation increments the global revision once. No clock reads, I/O, mutexes or audio inside this model.

- [ ] **Step 1: Add deterministic failing model tests.** Test builders `job(id, status)`, `warning(id, code)` and `discard(id, status)` create complete generation-7 payloads. Assert `success_deadline_is_first_terminal_plus_10000` exactly:

  ```rust
  let mut model = FileResults::default();
  model.receive("session-state", &json!({"generation": 7}), 0);
  model.receive("file-job", &job("a", "completed"), 50);
  assert_eq!(model.snapshot(10_049).notices[0].expires_at_ms, Some(10_050));
  model.receive("file-job", &job("a", "completed"), 9_000);
  assert!(model.snapshot(10_050).notices.is_empty());
  assert_eq!(model.snapshot(10_050).history.len(), 1);
  ```

  Add `warnings_survive_completed_and_old_ack`, `failures_outlive_success_and_connection_change`, `discard_has_one_notice`, `history_keeps_twenty_plus_unresolved`, and `evicted_duplicate_cannot_revive`. Assertions cover all persistent outcomes, warnings both before/after terminal, duplicate/progress regression, old generation, idempotent acknowledgement and owner namespaces. In the warning race, acknowledging revision 1 cannot hide warning revision 2; acknowledging revision 2 hides it globally. In the discard test, its generic terminal adds per-file details without a second notice/deadline, and a later index warning keyed by generic job ID updates that same discard notice only.
- [ ] **Step 2: Run** `cargo test --locked -p takedock --lib file_results` **and record failure** for the missing model/incorrect deadline. This command does not build/run the external fixture.
- [ ] **Step 3: Implement the contracts and normalization.** Accept generation advancement from `session-state`, terminal ordinary `file-job`, authoritative `discard-result`, `file-index-warning` and new `file-metadata-warning` (`{generation,id,code}`). Stage warnings received before terminal; a completed operation with warnings becomes persistent `warning`, while failed/cancelled outcomes retain their accurate outcome plus warnings. Retain generic-job-to-discard-key aliases from tagged progress/terminal events: enrich the canonical discard's names/per-file history without overriding its outcome, changing its outcome revision or rearming its deadline; route index warnings through that alias. Discard-tagged jobs never become duplicate notices/history entries. Detail-only changes increment the global snapshot revision, while new warnings increment outcome revision too. Reject stale generations while retaining settled earlier-generation descriptions. Keep compact seen-key/deadline/revision/ack tombstones for the process lifetime, independently of the twenty-entry history, so eviction cannot rearm a duplicate. Unknown/malformed event types change nothing. Snapshot filters expired/acknowledged notices without mutating deadlines.
- [ ] **Step 4: Rerun the filtered test command.** Expect all named cases passing; `cargo fmt --all --check` also passes after formatting only changed Rust files.
- [ ] **Step 5: Commit** only this deliverable: `feat: model shared file result lifetimes and acknowledgements`.

### Task 2: Shared process service, bootstrap and narrow result IPC

**Files:** Create `src-tauri/src/file_results/service.rs`; modify `file_results/{mod,tests}.rs`, `src-tauri/src/{desktop,events}.rs`, `src/lib/{types,ipc}.ts`.

**Interfaces:** Produce `FileResultStore::new(clock: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self`, `receive(&self, event: &str, payload: &Value) -> Option<u64>`, `snapshot(&self) -> FileResultSnapshot`, `acknowledge(&self, key: &ResultKey, revision: u64) -> Option<u64>`. Tauri commands: `file_results(state) -> FileResultSnapshot`, `acknowledge_file_result(key: ResultKey, revision: u64, state) -> FileResultSnapshot`. Bootstrap adds `file_results: FileResultSnapshot`. Frontend produces `ipc.fileResults(): Promise<FileResultSnapshot>` and `ipc.acknowledgeFileResult(key, revision): Promise<FileResultSnapshot>`.

- [ ] **Step 1: Write failing `snapshots_share_one_clock_and_ack`, `late_warning_does_not_lose_ack_race`, `replay_has_no_audio`, and `active_jobs_survive_twenty_terminals` tests.** Use an atomic fake clock and two readers of one store: at 9,999 ms both see success; at 10,000 neither does; a third fresh reader cannot replay it. Ack an old-generation notice without changing a capture/session value or invoking a sink. In Events, twenty-one newer completions must not evict the running job; existing sound-once/discard tests remain unchanged and pass. Add a private service unit test, `unchanged_capture_state_bypasses_result_lock`: hold its result-model mutex, send same-generation capture state from another thread, assert receipt completes before releasing the mutex, then release/join even on failure.
- [ ] **Step 2: Run** `cargo test --locked -p takedock --lib file_results` and `cargo test --locked -p takedock --lib events::tests`; expect the new store/active-retention regressions to fail before their fixes.

  ```rust
  assert_eq!(first.snapshot().notices.len(), 1); // fake clock: 9_999
  assert!(second.snapshot().notices.is_empty()); // fake clock: 10_000
  assert!(events.jobs.iter().any(|job| job["id"] == "held" && job["status"] == "running"));
  ```
- [ ] **Step 3: Implement the store and wiring.** Own an `Arc<FileResultStore>` in Runtime, using elapsed milliseconds from one `Instant`. Its own mutex protects short pure model work; snapshot/ack never acquire the Engine, capture or FileService locks. Use an atomic generation fast path so ordinary same-generation capture/session events and irrelevant events return before clock/model-lock work; actual generation advancement still reaches the checked model. On relevant sink events, feed the store and emit `file-results-changed {revision}` after releasing its lock, without recursively entering the sink. Emit that notification after acknowledgement too. Clone DTOs under the lock and serialize/emit outside it. Keep `Events::receive` as audio owner; neither store reads nor commands call feedback. Bootstrap uses a fresh store projection. Register the two commands and exact TS mirrors.
- [ ] **Step 4: Preserve all active job snapshots.** In `Events`, retain every queued/running entry plus at most twenty terminal entries rather than truncating the whole vector; generation changes still retire active jobs. Apply the same rule to frontend accepted-job placeholders in Task 4. Result history remains owned separately by Task 1.
- [ ] **Step 5: Run the two filtered Rust suites,** then `cargo check --locked -p takedock --features desktop --lib` and `npm run check`. Expect no errors/warnings. This is type/build verification, not a claim of native IPC acceptance (Task 7).
- [ ] **Step 6: Commit:** `feat: synchronize file results through process-owned IPC`.

### Task 3: Acknowledged rename metadata before indexing/queued work

**Files:** Modify `src-tauri/src/{files,guarded_delete}.rs`, `file_results/tests.rs`, `tests/fixtures/adb/{src/main.rs,tests/files.rs}`.

**Interfaces:** Produce `confirmed_rename_video(source: &Video, destination_name: &str, entry: takedock_files::Entry) -> Result<Video, String>` and serializable `RenameUpdate` in `files.rs`; emit `file-renamed` with that DTO. Extend `FileResult` with optional `requested_name` and `destination_name` fields, matching `ResultItem`; all non-rename constructors default them to `None`. Metadata failure emits `file-metadata-warning {generation,id,code:"rename_refresh_pending"}`; indexing keeps `file-index-warning` and `media_index_pending`.

- [ ] **Step 1: Add Task 7's native held-index rename regression to case 3 alone.** Reuse its existing fixture controls; retain all four case titles and existing assertions. Commit/push this native test without adding failing Rust tests yet, open a draft PR on this branch and attach it to this chat.
- [ ] **Step 2: Run the existing hosted Validate workflow on that test commit.** Its Rust tests must still pass so the desktop job actually builds/runs the release EXE. Record case 3 failing because the old native name remains while indexing is held. This establishes the full-flow gap before either backend or frontend rename changes; an earlier Rust compilation failure would not establish it.
- [ ] **Step 3: Add failing pure/process tests and their external controls.** Pure tests: `confirmed_rename_requires_expected_name_and_inode` and `confirmed_rename_preserves_exact_large_identity`, asserting fresh ctime/opaque identity, whole destination metadata, wrong-name/device/inode rejection and device/inode above JS's safe integer limit. Include mixed-case extension and Arabic/apostrophe names. Process tests: `rename_update_precedes_held_index_and_next_job`, `committed_rename_inspection_failure_is_warning`, `late_cancel_after_rename_ack_keeps_completed`, `unacknowledged_rename_never_patches_rows`; retain collision/no-overwrite byte assertions. Add external `fail-rename-inspect` only for a successfully renamed destination, record `rename-acknowledged` after `Root::rename` returns, and let `hold-rename-ack` wait there to simulate loss of process acknowledgement. Capture named events as well as file-job payloads. Release every owned hold in teardown; no product test switch.
- [ ] **Step 4: Run** `cargo test --locked -p takedock --lib files::tests` locally and `cargo test --locked -p takedock-adb-fixture --test files` **hosted only**. Record the missing update and wrong postcommit-outcome failures. Stage the pure/process regression commit only after Step 2's native evidence exists; no CI ordering/skip workaround is needed.

  ```rust
  assert_eq!(fixture.result(&id)["status"], "completed"); // inspection failure after ack
  assert_eq!(metadata_warning["code"], "rename_refresh_pending");
  assert_eq!(confirmed.video.name, "renamed.MP4");
  assert_ne!(confirmed.video.identity, original.identity);
  ```
- [ ] **Step 5: Implement rename outcome handling.** On successful helper rename acknowledgement, set the committed destination immediately. Use the existing `inspect` command through the already prepared helper, with a bounded `5_000` ms read-only call on the same lease. Deserialize `Entry`, verify expected name and exact device/inode in Rust, and construct a complete new `Video` using checked timestamp conversion and freshly serialized identity. Emit `file-renamed` before either index broadcast or the next queued job. Inspection/index failure after acknowledgement is a warning, never a failed/cancelled mutation or a retry. Cancellation before acknowledgement keeps existing fail-closed behavior and produces no confirmed update. No helper CLI change, extra scanning/hash/decode, optimistic metadata or capture-path work.
- [ ] **Step 6: Run the pure tests and hosted process suite again.** In the held-index case assert the update already contains the destination, no next-job hash has begun, then allow indexing and prove a later held job does not gate the update. Disconnect/cancel after `rename-acknowledged` and verify that receipt versus loss of process acknowledgement selects completed-with-warning versus unconfirmed failure evidence correctly. Re-run existing held-hash capture and guarded-discard regressions on the hosted runner. Native label red becomes green after frontend integration in Task 5; do not claim it green at this backend stage.
- [ ] **Step 7: Commit:** `fix: publish verified rename metadata before queued file work`.

### Task 4: Frontend result projection and session integration

**Files:** Create `src/lib/file-results.ts`, `file-results.test.ts`; modify `src/lib/{types,ipc,session.svelte,session.test}.ts` and `src/App.test.ts` bootstrap mocks.

**Interfaces:** Produce `resultKey(key: ResultKey): string` and `FileResultsController` with constructor dependencies `{read, acknowledge, now, schedule, cancel, changed, failed}`. Their types are `read: () => Promise<FileResultSnapshot>`, `acknowledge: (key: ResultKey, revision: number) => Promise<FileResultSnapshot>`, `now: () => number`, `schedule: (callback: () => void, delay: number) => ReturnType<typeof setTimeout>`, `cancel: (timer: ReturnType<typeof setTimeout>) => void`, `changed: (view: FileResultView) => void`, `failed: (error: unknown) => void`. Produce `FileResultView {notices: FileNotice[]; history: FileNotice[]}`, methods `accept(snapshot: FileResultSnapshot, requestedAt: number): void`, `refresh(minRevision?: number): Promise<void>`, `hide(notice: FileNotice): Promise<void>`, `dispose(): void`. Session exposes `data.fileResults: FileResultView` and `hideFileResult(notice: FileNotice): Promise<void>`; hide rejection is reported once by `failed` and remains rejected so the caller can cancel focus-return intent.

- [ ] **Step 1: Write failing fake-clock/deferred-IPC tests.** In `expiry_uses_original_deadline_despite_snapshot_latency`, request at local 100, receive at 400 a snapshot with server now 1,000/deadline 11,000: local expiry is no later than 10,100. A duplicate snapshot cannot move it later; assert visible one millisecond before the computed cutoff and absent at it. Add `older_snapshot_cannot_restore_acknowledged_notice`, `warning_revision_survives_old_hide_response`, `notification_during_read_triggers_fresh_projection`, `late_new_window_does_not_replay_success`, and `dispose_cancels_timers_and_pending_reads`. Assert no call to videos/recording/sound from this controller.
- [ ] **Step 2: Run** `npm run test:ui -- src/lib/file-results.test.ts src/lib/session.test.ts`; expect the new lifecycle tests to fail before implementation. Add a session regression with twenty-one completions and a held active job: the active ID remains cancellable, and an unrelated verification/settings message is not erased when a file success expires.

  ```ts
  expect(view.notices).toHaveLength(1); // local clock: 10_099
  expect(view.notices).toHaveLength(0); // advance to 10_100
  expect(view.history).toHaveLength(1);
  expect(app.data.jobs.some((job) => job.id === 'held')).toBe(true);
  ```
- [ ] **Step 3: Implement deadline projection.** Use `performance.now()` in production. At response receipt, remaining time is at most `max(0, expires_at_ms - snapshot.now_ms - (receivedAt - requestedAt))`; intersect it with any existing local cutoff for the same notice revision. Retain expired cutoffs until disposal so a delayed equal-revision snapshot cannot recreate a removed success. The same success's absolute deadline never changes across revisions. A persistent warning has no expiry and supersedes its old success timer. Timer callbacks re-evaluate current keys/revisions and time; schedule only the next due expiry. Coalesce reads, remember notifications received while reading, and reject lower global revisions. A failed/older hide response cannot remove a newer outcome; successful hide accepts its authoritative snapshot. Disposal fences all pending callbacks.
- [ ] **Step 4: Wire listeners before bootstrap.** Subscribe to `file-results-changed`, feed bootstrap's projection using its request-start time, and fetch again if a newer revision was observed. Result notifications remain process-wide even when their settled entries describe an earlier connection. Stop writing file-job/discard/index outcomes into `data.result`; preserve their necessary refresh/progress side effects and capture/settings message ownership. Keep the canonical discard refresh deduplicated, and add all listener/timer cleanup. Preserve every queued/running accepted-job placeholder plus at most twenty terminals; do not truncate active work. Update mocks and exact subscription counts to reflect added listeners without weakening cleanup assertions.
- [ ] **Step 5: Run** the two targeted suites, `npm run check`, then `npm run test:ui`. Expect all passing, including existing immediate capture dispatch tests; no native-accessibility claim yet.
- [ ] **Step 6: Commit:** `feat: project shared file result lifetimes in each window`.

### Task 5: Exact rename reconciliation and stale listing protection

**Files:** Create `src/lib/rename-updates.ts`, `rename-updates.test.ts`; modify `src/lib/{session.svelte,session.test,types}.ts`, `src/components/VideosPanel.test.ts`; preserve `src/lib/videos.ts` exact-string key behavior.

**Interfaces:** Produce `RenameUpdates.reset(generation: number): void`, `record(update: RenameUpdate): boolean` and `apply(videos: Video[], generation: number): Video[]`. `record` deduplicates operation IDs in the current generation and buffers unmatched exact-source links; `apply` follows matching `(source_name, source_identity)` links to the latest complete destination, retaining unmatched links until a matching listing/update arrives or generation resets. It has no IPC/DOM side effects.

- [ ] **Step 1: Add failing pure/session tests** `backwards_rename_updates_chain_without_rollback`, `old_listing_cannot_restore_renamed_identity`, `rename_invalidation_releases_loading_owner`, and `replacement_or_generation_mismatch_is_untouched`. Use A->B->C with both delivery orders; duplicate A->B after C cannot restore B. After invalidating an in-flight refresh, its resolve/reject/finally must neither overwrite videos nor clear a newer refresh's loading flag. Assert whole destination metadata replacement, exact large identity strings, no capture/sound call and no guessed destination on failed rename.
- [ ] **Step 2: Run** `npm run test:ui -- src/lib/rename-updates.test.ts src/lib/session.test.ts src/components/VideosPanel.test.ts`; record the relevant missing-event/stale-listing failures.

  ```ts
  expect(reconciler.apply([a], 7)).toEqual([c]); // record B->C before A->B
  expect(app.data.videos[0].identity).toBe(c.identity); // old listing resolves
  expect(app.data.loading).toBe(false); // invalidated request owns no spinner
  expect(checkbox).toBe(screen.getByRole('checkbox', { name: 'Select renamed.mp4' }));
  expect(checkbox).toHaveFocus();
  ```
- [ ] **Step 3: Implement the pure reconciler and `file-renamed` listener.** For a current-generation update, record its exact source link, invalidate the old refresh serial, explicitly release that request's loading ownership, then reconcile current videos synchronously. Reconcile valid fresh listing responses through buffered links too. Clearing a generation/disconnecting resets the links and pending listing ownership. Never replace a different object merely because its filename or device/inode key resembles the source. Reject malformed or mismatched destination identity before it reaches a selected row. A stale queued progress event cannot change this data path.
- [ ] **Step 4: Strengthen the existing same-inode component test.** Retain the actual `HTMLInputElement`, select/focus it, rerender with the confirmed full destination `Video`, then assert the same object is checked, still focused, and has accessible name `Select renamed.mp4`. Assert subsequent `onJob` receives the new identity, not the old ctime. Also cover Arabic labels and readiness changes removing inapplicable selection; do not change keyed-row rendering merely to satisfy this test.
- [ ] **Step 5: Run** the targeted suites, `npm run check`, and the full UI suite. Expect all passing. Computed native label and audible NVDA acceptance remain Task 7.
- [ ] **Step 6: Commit:** `fix: reconcile renamed rows without stale listing rollback`.

### Task 6: Native notices, stable collapsed history and Activity

**Files:** Create `src/components/FileResultsPanel.svelte`, `FileResultsPanel.test.ts`; modify `src/App.svelte`, `App.test.ts`, `src/components/{ActivityPanel.svelte,ActivityPanel.test.ts}`, `src/lib/i18n.ts`, `src/styles.css`, `docs/{architecture,keyboard,keyboard.ar}.md`.

**Interfaces:** `FileResultsPanel` props are `{notices: FileNotice[], history: FileNotice[], language: 'en'|'ar', onHide: (notice: FileNotice) => Promise<void>}`. `ActivityPanel` adds `notices: FileNotice[]` and `onCancel: (id: string) => void` to existing jobs/loading/language. Its active list uses queued/running jobs only; terminal summaries come exclusively from eligible notices. App passes `data.fileResults` and `app.hideFileResult`.

- [ ] **Step 1: Write failing panel/App tests.** In `success_expires_from_notice_activity_and_expanded_job_surfaces`, advance fake time to 10,000 ms and assert main success/100% disappear, Activity says Ready, and details still exist only inside the initially closed native history disclosure. In `failure_survives_later_success`, expire a later success and assert the failed notice/hide button remain. In `remote_ack_and_passive_expiry_keep_focused_card`, focus its hide button, remove the notice from props, assert the same focused button remains until focus actually leaves. Also test `local_hide_does_not_reclaim_moved_focus`, `open_history_pins_displayed_records_past_twenty`, active cancellation during terminal churn, English/Arabic labels and absence of `[aria-live], [role=alert], [role=status]`.
- [ ] **Step 2: Run** `npm run test:ui -- src/components/FileResultsPanel.test.ts src/components/ActivityPanel.test.ts src/App.test.ts`; expect the new behavior regressions to fail against the old permanent job/result presentation. Add Task 7's native lifecycle/history assertions now and record a hosted native red run before changing these view components; UI test failures do not prevent the independent desktop job from building the unchanged views. Preserve assertions for indeterminate listing/verifying and completed-copy 100% while its success is eligible.

  ```ts
  expect(container.querySelector<HTMLDetailsElement>('details')?.open).toBe(false);
  expect(document.activeElement).toBe(focusedButton); // remote ack/passive expiry
  expect(screen.getByRole('button', { name: /Hide result/ })).toBe(focusedButton);
  expect(container.querySelector('[aria-live], [role="alert"], [role="status"]')).toBeNull();
  ```
- [ ] **Step 3: Implement notice/history rendering.** Key cards and history records with `resultKey`. Each card exposes ordinary outcome/per-file text and a native hide button; acknowledgement never cancels a job. Always render a native `<details><summary>` for history, initially closed per mount/window, with an empty-history sentence when appropriate. While open, merge incoming records but defer eviction of already displayed keys until closure. Render source and committed destination with separate `<bdi>` elements; a requested name on failed rename must not appear as a committed destination. `rename_refresh_pending` explains that the acknowledged rename completed while list metadata awaits refresh; preserve existing identity validation on later actions.
- [ ] **Step 4: Implement focus-safe retirement.** Retain a removed card locally if it contains `document.activeElement`; recheck after focusout settles and remove only when focus leaves. A newer outcome for the same key replaces text in place and cancels an old expiry. Await an explicit local `onHide` request, then a Svelte tick; move focus once to the next visible hide button or history summary only if the clicked control is still focused, its result key is now absent and the card is being removed. If the user moved focus, a newer warning remains or the request rejects, clear that local intent without moving focus. Remote acknowledgement/passive expiry never actively moves focus. Unmount releases retained references; no synthetic announcement or hidden live copy.
- [ ] **Step 5: Integrate Activity and remove duplicate terminal surfaces.** Show all active jobs with native cancellation, preserving current-file progress and batch counts. Running work precedes queued work, then eligible terminal summary; terminal progress is settled (100 for fully completed outcomes, 0 for unsuccessful/incomplete ones), never indeterminate running work. With no active work/notice, listing stays indeterminate while loading and otherwise Ready/0. Remove App's permanently open terminal-job section and file-specific writes to the generic result section; keep unrelated capture/settings messages intact.
- [ ] **Step 6: Add bilingual copy and guidance.** New exact labels: `operationHistory`: "Operation history" / "سجل العمليات"; `hideFileResult`: "Hide result" / "إخفاء النتيجة"; `noFileHistory`: "No completed file operations yet." / "لا توجد عمليات ملفات مكتملة بعد."; `rename_refresh_pending`: "The rename completed, but the video list still needs to refresh." / "تمت إعادة التسمية، لكن قائمة الفيديوهات ما زالت تحتاج إلى تحديث." Qualify hide buttons with operation/filename in native text. Document ten seconds, persistent failures, process-local history, focus exception and shared acknowledgements in both keyboard guides; document capture-independent result ownership in architecture.
- [ ] **Step 7: Run** the targeted suites, `npm run format:check`, `npm run check`, `npm run test:ui`, `npm run build`. Expect all passing and no warnings. Update only legitimate renamed/new behavior expectations, not timing/coverage shortcuts.
- [ ] **Step 8: Commit:** `feat: present transient file results and accessible operation history`.

### Task 7: Release-flow, physical phone and whole-branch acceptance

**Files:** Modify `tests/desktop/{critical.e2e,fixtures}.ts`; create `docs/testing/file-results-rename.md` and `docs/research/2026-10-21-file-results-rename-acceptance.md`. Only if an actual fixture gap requires it, extend `tests/fixtures/adb/src/main.rs` with external controls. Put raw reports/harness files under a new ignored `test-results/file-results-rename/<trial-id>/`; no private identifiers in committed evidence.

**Interfaces:** Reuse `button`, `shortcut`, `waitFor`, `flag`, `commands`, `hash`, `restartFixture` and the existing `hold-index`/`hold-hash` controls. WebdriverIO's installed `element.getComputedLabel(): Promise<string>` proves the browser-computed native label; keep a retained element handle to test DOM-node identity. Read-only bootstrap/result projections may synchronize assertions, while actions including rename/hide/capture/new-window remain native UI clicks/keys.

- [ ] **Step 1: Extend release regressions before the corresponding product fix.** Introduce assertions from this task alongside their owning implementation task and capture a hosted red run before its fix; do not wait until everything is implemented to create all native regressions. Keep these four titles exactly:

  1. `launches offline with named keyboard controls and silent success defaults`
  2. `dispatches all four actions while verification is held and retains logical focus`
  3. `selects two videos, hides batch rename and copies verified bytes through Rust IPC`
  4. `persists Arabic, destination and optional success sound across application restart`

  Case 1 preserves offline/native naming/silent defaults and asserts history initially closed. Case 2 retains existing dispatch/discard checks and absence of live announcements. Case 3 retains all copy/hash/cancel/source-preservation/100% checks and adds the rename/result scenarios below. Case 4 retains actual restart/preferences and asserts Arabic history/hide labels. Preserve the existing 90,000 ms case and 15,000 ms helper bounds; overlap expiry observation with held-work/capture steps instead of adding fixed long sleeps.
- [ ] **Step 2: Add native rename/result scenarios inside case 3.** Select the dedicated fixture `رحلة.mp4` and rename it to `renamed-رحلة.mp4`; retain its checkbox wire element ID and initial identity from a read-only listing before the worker is held. Set `hold-index`, submit the rename form, wait for `index-started`, then use a new bounded 1,000 ms label check inside the existing 5,000 ms index-call bound. Assert the newly queried checkbox has the original wire ID, exposes the new name and remains selected; do not let WebdriverIO's stale-element auto-refetch stand in for node identity. Focus that checkbox before releasing the hold and prove later completion does not steal it. Queue copying through the updated row while indexing holds the worker; release indexing, hold its hash and assert recording Start/Stop reach the persistent command process. Successful copying validates the updated ctime identity; after the worker is free compare a fresh listing's ctime/device/inode to the initial identity. Never await serialized listing while the worker is held. Preserve byte equality and no-live-region checks.

  ```ts
  assert.equal(renamedBox.elementId, originalElementId);
  assert.equal(await renamedBox.getComputedLabel(), 'Select renamed-رحلة.mp4');
  assert.equal(await renamedBox.isSelected(), true);
  assert.equal((await $$('[aria-live], [role="status"], [role="alert"]')).length, 0);
  ```
- [ ] **Step 3: Exercise shared lifetime and acknowledgement in the same case.** Observe one completed clean operation in two UI-opened windows, with the same original deadline from read-only snapshots. Do not keyboard-focus its hide button during the normal expiry test. Continue capture/held-work assertions while waiting for eligibility to end; both main presentations must remove success by the common deadline plus their existing polling tolerance. A window opened afterward shows no expired success, while closed history retains details. Produce a separate fixture failure; both windows retain it through a later clean success. Hide through one window's native button and assert the other removes it unless focus-protected, then removes it after focus leaves. Deferred old-listing/reversed-update delivery stays deterministically covered in Task 5; the native held-index/next-job flow proves real IPC timing and labels.
- [ ] **Step 4: Run the complete existing hosted Validate workflow on the final code tree.** Rust: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`. UI: `npm ci`, `npm audit --audit-level=moderate`, formatting/check/tests/build and both existing Node release-gate tests. Observer: existing tests/lint/build. Windows: all three NDK helper ABIs, desktop Clippy, workspace tests, actual release build, `scripts/check-updater.ps1`, driver setup and unprivileged `scripts/test-desktop.ps1`. Expect successful rust/ui/observer/desktop/quality-gate jobs and exactly 4/4 desktop cases; retrieve actual run/acceptance evidence, not just a green summary. No workflow weakening or signed publication is required.
- [ ] **Step 5: Write the repeatable physical test procedure, then perform it within the existing phone authorization.** Identify the existing authorized wireless connection read-only; leave pairing/server intact. Capture initial settings/accessibility and unrelated-file identities/hashes into the private trial. Create new, explicitly journaled short test clips through ordinary capture controls; verify a local backup and full decode before any fixture rename/delete. Use the existing preservation-proxy source only if needed, with a new allowlist/trial root; never reuse old clip manifests. Its backup/verification is trial tooling, not production behavior. Record the actual EXE/helper hashes and source tree.
- [ ] **Step 6: Exercise the physical behaviors and record distinct evidence levels.** Rename only the new fixture, check its full current row/checkbox native label, selection and focus with NVDA running. Check ten-second success, persistent failure via a no-overwrite collision between two newly owned test clips, collapsed history, another window's acknowledgement and focus-protected expiry. Metadata/index-warning injection remains a hosted fixture test; do not damage phone services to induce it. Record capture dispatch while file work is held and while expiry occurs, using monotonic observations and an unchanged-path baseline rather than an invented latency threshold. Ask the user for audible NVDA observations when needed; computed label/DOM focus alone cannot certify speech or virtual-cursor behavior. Restore temporary settings/accessibility, verify unrelated bytes/identities, preserve backups, and confirm wireless still connected. If the device is temporarily unavailable, complete independent checks and report physical evidence pending instead of claiming acceptance.
- [ ] **Step 7: Commit public-safe evidence** with named cases, red/green run links, tool versions, tested tree/EXE/helper hashes, native-versus-audible evidence, timings and limitations. Run `git diff --check`; rerun gates affected by any product/test fixes and verify final code tree equals the tested tree. Evidence-only commits may record an identical product tree explicitly.
- [ ] **Step 8: Request one fresh whole-branch code review using the selected execution workflow.** Supply the approved spec, this plan, final diff and evidence, including all five Review Focus items. Resolve actionable findings, run their regressions and affected gates, and verify the final tree again. The review is mandatory for Native execution; no per-task subagents are needed for that method.
- [ ] **Step 9: Finish the branch through the applicable Superpowers finishing workflow.** Update/attach the draft PR with final behavior and validation evidence, retain required checks and request user review. This plan does not authorize merging this new PR or publishing a release. Preserve the managed worktree, phone connection and private test backups; do not revive the prior plan's blocked cleanup.

## Plan self-review and handoff

- [x] Checked each approved spec section against its task: lifecycle/history (1), synchronization/audio/bootstrap (2), committed rename truth (3), local clock/race/disposal (4), exact row reconciliation (5), UI/focus/bilingual copy (6), actual four-case/phone/final-tree evidence (7).
- [x] Checked all DTO fields and method names against Shared contracts and Interfaces; no unnamed cross-task helper or placeholder remains. Every Review Focus item has its named regression above. Fixed asynchronous hide return types, stale cutoff retention and hosted red-test ordering during review.
- [x] Kept prose at the decision/signature/assertion level; implementation bodies belong in product code after approval. The seven deliverables each have their own red/green cycle and commit.
- [ ] Present this saved plan for user review. Preserve the previously selected Native execution method if confirmed applicable: implement in this session through `superpowers:executing-plans`, then one fresh whole-branch reviewer. Do not implement while plan review remains pending.
