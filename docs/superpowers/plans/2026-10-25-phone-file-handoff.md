# One-way Phone File Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove desktop video transfers and make Mark move a finalized video into the phone's TakeDock folder, then remove it from application ownership.

**Architecture:** Keep OpenCamera as the sole managed source. A guarded helper command performs an atomic no-replace move on the same phone filesystem; a confirmed, operation-bound receipt removes the source row across windows. Reuse file-job scheduling, current-take protection and shared result presentation while removing desktop payload transfer and destination settings.

**Tech Stack:** Existing Tauri 2 / Rust 1.99.1 / Svelte 5 / TypeScript; Android file helper, real SDK ADB, WebView2 native acceptance.

**Spec:** `docs/superpowers/specs/2026-10-25-phone-file-handoff-design.md` — user-approved on 2026-10-25. Read it with this plan.

**Execution:** Native, as previously selected by the user: implement inline, then one fresh whole-branch reviewer. This plan awaits user review before implementation. Work in the existing isolated worktree on `abdo/phone-file-handoff`; base implementation is `1dbb32e`, design commit `93fb8b3`. Do not change the primary checkout or inspect other chats.

## Global Constraints

- Source: `/sdcard/DCIM/OpenCamera`. Fixed destination: `/sdcard/DCIM/TakeDock`.
- Mark is one-way. After confirmed handoff, do not list/watch/manage the destination collection or offer unmark/return. Exact operation-target inspection during completion/recovery is allowed; no sweep.
- Finalized regular videos only; validate exact identity, generation, directory containment and no-overwrite behavior. No cross-filesystem copy fallback, payload hash, full decode or Windows payload staging in production marking.
- Preserve recording, readiness, current-take discard, source Delete/Rename and independent capture dispatch.
- Preserve shared success expiry at 10,000 ms, persistent failures/uncertainty, collapsed history, Arabic/English controls, stable keyboard focus and no unsolicited speech.
- Never label a committed handoff retained/cancelled because of late cancellation or index failure; never automatically replay a mutation.
- Keep legacy settings readable; remove the Windows destination field from new settings without touching user files/folders or unrelated settings.
- Never build/restore/run the external fake ADB locally, including through workspace tests, `scripts/test-desktop.ps1`, `tests/desktop/fixtures.ts`, WSL or cached binaries. Those paths run on GitHub only.
- Preserve existing shared SDK ADB/pairing. Phone tests use only fresh owned fixtures with locally preserved, size/hash-checked, fully decoded backups; leave the phone Home/asleep after each session.
- Compatible dependency changes remain authorized when needed; do not add unrelated upgrades. No merge/release is authorized by this plan.

## Review Focus

1. A user-created destination file wins a last-moment collision; its bytes and the original survive unchanged (Task 1).
2. A late removal/rename event must not hide a different video that reused a source basename, including opaque identities beyond JavaScript's safe integer range (Task 3).
3. Cancel after permit/commit, dropped acknowledgement and failed index/intent cleanup must produce truthful distinct outcomes without a second mutation (Tasks 1–2, hosted Task 4).
4. A pre-existing TakeDock folder and its unrelated files survive startup, refresh, reconnect and source Delete/Rename; an unusable destination does not disable those source features (Tasks 1, 4).
5. An old settings file with a missing, relative or differently typed obsolete destination loads/saves its unrelated preferences without creating any Windows folder (Tasks 2–3, hosted Task 4).

## File Responsibilities

- `file-helper/src/handoff.rs` (new): handoff outcome/receipt, guarded exchange and same-filesystem relocation/recovery support; `file-helper/src/lib.rs`, `main.rs`: integrate fixed destination and compatible journal recovery. `file-helper/tests/handoff.rs` (new): filesystem/protocol cases; retain guarded-delete/watch coverage.
- `src-tauri/src/handoff.rs` (new): bounded host exchange and receipt validation; `files.rs`: Mark/Delete/Rename queue and reservations; `helper.rs`: helper delivery checksum retained outside removed payload code.
- `src-tauri/src/discard.rs`, `session.rs`, `session/discard_runtime.rs`, `desktop.rs`: retire the exact handed-off take and connect commit feedback without blocking capture. `events.rs`, `file_results/`: accurate operation history/outcomes.
- `src-tauri/src/model.rs`, `settings.rs`: remove obsolete destination; `src-tauri/src/lib.rs`, `Cargo.toml`, `file-helper/Cargo.toml`: module/dependency integration, only where usages require it.
- `src/lib/handoff-updates.ts` (new), `types.ts`, `session.svelte.ts`, `ipc.ts`: confirmed source removal and pending command state. `VideosPanel.svelte`, `SettingsPanel.svelte`, `ActivityPanel.svelte`, `App.svelte`, `i18n.ts`: actions, focus and removal of desktop-transfer UI.
- Corresponding existing `*.test.ts`, new `src/lib/handoff-updates.test.ts`, helper/backend unit tests: behavior proofs. `tests/fixtures/adb/`, `tests/desktop/`, `.github/workflows/ci.yml`: GitHub-only process/native acceptance cutover.
- `scripts/test-phone-handoff.ps1` (new): real-device native test controller, isolated from fixture imports. `docs/research/2026-10-25-phone-file-handoff.md` (new): actual coverage/timing and limitations. Retain earlier research and private trial evidence as history.

---

### Task 1: Guarded same-filesystem helper handoff

**Files:** Create `file-helper/src/handoff.rs`, `file-helper/tests/handoff.rs`; modify `file-helper/src/lib.rs`, `file-helper/src/main.rs`, `file-helper/Cargo.toml`, and existing helper tests when a removed API is referenced.

**Interfaces:** Export `handoff::HANDOFF_ROOT = "/sdcard/DCIM/TakeDock"` and these serde types (`Outcome` uses `#[serde(tag = "status", rename_all = "snake_case")]`):

```rust
pub struct Receipt {
    pub operation_id: String,
    pub source_name: String,
    pub destination_name: String,
    pub identity: Identity,
}
pub enum Outcome {
    HandedOff { receipt: Receipt, warning: Option<String> },
    Retained { code: String },
    OutcomeUnknown { code: String },
}
impl Root {
    pub fn handoff_to(&self, destination: &Root, name: &str, expected: &Identity,
        operation_id: &str, permit: impl FnOnce() -> Result<bool, String>) -> Outcome;
}
```

`handoff::serve(source: &Root, destination: &Root, name: &str, expected: &Identity, operation_id: &str, read_permit: impl FnOnce() -> Result<Vec<u8>, String>, output: &mut impl Write) -> Result<(), String>` produces one bounded exchange. `handoff::Message` uses a snake_case `type` tag with `Prepared { operation_id }`, `Permit { operation_id, allow }` and `Result { operation_id, outcome: Outcome }`. Maximum message 4096 bytes, stdin permit timeout 5000 ms, matching operation ID required. The release CLI adds `handoff-guarded <name> <identity-json> <operation-uuid>`; it chooses/creates the fixed destination itself, never a caller-supplied path.

- [ ] **Step 1: Establish the allowed baseline.** Run `cargo test --locked -p takedock-files --lib --test guarded --test watch` and `cargo test --locked -p takedock --lib`. Confirm failures are not introduced before editing; never substitute workspace tests locally.
- [ ] **Step 2: Write the failing filesystem/protocol tests.** Build two temporary sibling roots and a finalized MP4 using the existing small atom fixture pattern in `file-helper/tests/guarded.rs`. Define the test-only `two_roots(name)` fixture to return its temp directory, source root, destination root and source `Identity`. Pin the main behavior:

```rust
let (_trial, source, target, identity) = two_roots("رحلة '1.mp4");
let outcome = source.handoff_to(&target, "رحلة '1.mp4", &identity,
    "4fb0ef42-f12b-4b61-ae88-cf0bd0650084", || Ok(true));
assert!(matches!(outcome, Outcome::HandedOff { .. }));
assert!(source.list().unwrap().is_empty());
assert_eq!(target.stat("رحلة '1.mp4").unwrap().inode, identity.inode);
assert_eq!(target.stat("رحلة '1.mp4").unwrap().size, identity.size);
```

Add named cases `collision_at_publish_retains_both`, `wrong_or_denied_permit_never_publishes`, `changed_or_unfinished_source_is_protected`, `destination_symlink_or_cross_device_is_rejected`, `claim_crash_restores_source`, `committed_crash_never_returns_destination`, `destination_removed_after_commit_never_blocks_source_operations`, and `legacy_delete_journal_still_recovers`. Assert exact bytes and unrelated sentinels, not just existence. Use private `#[cfg(test)]` fault injection for publication/recovery boundaries, with no shipped switches. Cover replacement of the destination directory after opening: an anchored move may never enter the replacement; return uncertainty rather than success unless the exact target in the expected location is confirmed.

- [ ] **Step 3: Observe RED.** Run `cargo test --locked -p takedock-files --test handoff`. Confirm missing handoff contract/failed new assertions, not environment failure.
- [ ] **Step 4: Implement the contract in `handoff.rs` and integrate recovery/CLI.** Reuse the shared mutation lock and verified source claim. Extend intent records with a distinct handoff operation/destination name and retain version-1 recovery. Anchor both directories; use Linux/Android `renameat2(..., RENAME_NOREPLACE)` across their descriptors and the corresponding no-replace Windows test implementation. Verify source claim before publishing; check device/inode/size/mtime afterward, accepting changed ctime. Handle post-commit cleanup as committed-with-warning, not source retention. Only open the destination during an actual handoff or recovery of its exact pending intent. Remove payload `read`/`hash` release CLI commands and library methods once callers are accounted for; replace retired transfer-only tests with equivalent source/identity/mutation guards. Keep any checksum dependency still needed by tests as test-only.
- [ ] **Step 5: Observe GREEN.** Run `cargo test --locked -p takedock-files --lib --test handoff --test guarded --test watch` and `cargo clippy --locked -p takedock-files --all-targets -- -D warnings`. Expected: all selected tests pass; no warnings. Linux/Android-specific cases are additionally required in Tasks 4–5.
- [ ] **Step 6: Commit** the helper contract and its tests as `feat: hand off finalized videos within phone storage`.

### Task 2: Cut the backend over to Mark/Delete/Rename

**Files:** Create `src-tauri/src/handoff.rs`; modify `files.rs`, `helper.rs`, `lib.rs`, `desktop.rs`, `model.rs`, `settings.rs`, `discard.rs`, `session.rs`, `session/discard_runtime.rs`, `events.rs`, `file_results/model.rs` and their unit tests. Adjust `guarded_delete.rs` for shared job-schema changes. Remove transfer-only runtime code/trace stages, retaining non-payload queue timing and helper-delivery checksum verification.

**Interfaces:** `JobKind = { Mark, Delete, Rename }`; reject deserializing `copy`/`move`. Remove byte-transfer `bytes`/`total` from `JobEvent`; retain phase, completed/count and per-file results. Define `HandoffUpdate` and `HandoffCommit` in `files.rs`:

```rust
pub struct HandoffUpdate {
    pub generation: u64,
    pub job_id: String,
    pub operation_id: String,
    pub sequence: u64,
    pub source_name: String,
    pub source_identity: String,
}
pub type HandoffCommit = Arc<dyn Fn(&HandoffUpdate) + Send + Sync>;
// FileService:
pub fn start_job(&self, lease: DeviceLease, kind: JobKind, videos: Vec<Video>,
    new_stem: Option<String>, on_handoff: Option<HandoffCommit>) -> Result<String, String>;
```

`handoff::perform(helper: &HelperClient, lease: &DeviceLease, video: &Video, operation_id: &str, cancel: &AtomicBool) -> Outcome` consumes Task 1's protocol. Use a fresh per-file UUID and monotonic per-file sequence; duplicate batch receipts cannot collapse onto the job ID. `Engine::retire_handed_off(&self, update: &HandoffUpdate)` retires only a matching current ended take. Desktop passes a weak-runtime callback; invoke it before emitting `file-handed-off`, outside file/capture locks.

- [ ] **Step 1: Write failing pure tests** for `confirmed_receipt_only_removes_exact_source`, `acknowledged_commit_beats_late_cancel`, `ack_lost_after_permit_is_unknown`, `batch_conflict_continues_and_receipts_are_unique`, `same_identity_cannot_be_reserved_twice`, and `handoff_retires_only_matching_take`. Test the classifier/state transition with injected messages, not a locally spawned ADB fixture:

```rust
assert!(serde_json::from_str::<JobKind>("\"copy\"").is_err());
assert!(serde_json::from_str::<JobKind>("\"move\"").is_err());
assert_eq!(serde_json::from_str::<JobKind>("\"mark\"").unwrap(), JobKind::Mark);
// A result after permit loss must project OutcomeUnknown, with no expiry;
// an acknowledged handoff must project Completed even if cancel is now true.
```

Add `legacy_destination_is_ignored_without_filesystem_access`: vary the old field through a missing path, relative text and JSON object; assert language, ADB path, sounds/readiness/verification survive and new JSON has no destination. Add an existing TakeDock sentinel to source-only recovery tests: source Rename/Delete must not visit it.

- [ ] **Step 2: Observe RED** with `cargo test --locked -p takedock --lib` after adding the new assertions. Capture the actual failing contract before changing runtime behavior.
- [ ] **Step 3: Implement host handoff and remove desktop transport.** Bound subprocess input/output and joins like guarded delete; deadline 30,000 ms. Before permit transmission, cancel/replaced lease means no commit authorization. From the first attempted permit write, a lost result is uncertain; after a matching success receipt, cancellation only stops remaining batch items. Validate receipt UUID/name and unchanged device/inode/size/mtime against the requested source. Per-file `FileResult.status` distinguishes `completed`, `retained`, `outcome_unknown`, `failed` and `cancelled`; an uncertain handoff uses code `handoff_outcome_unknown`. Keep the existing terminal job states (`completed`/`failed`/`cancelled`), with uncertainty/failure taking precedence over cancellation; update result projection to preserve the precise per-file facts. Emit `file-handed-off` only for confirmed receipts, once each, before best-effort source/target index notifications. Index failure adds a persistent warning. Reserve accepted mutation identities until settled so competing windows receive `file_busy`, and release reservations on every finish/enqueue-failure/shutdown path.
- [ ] **Step 4: Integrate exact take retirement and settings migration.** Under a short generation/identity-checked model lock, invalidate only the matching ended discard target and any uncommitted discard authorization; a newer take is unaffected. Remove `destination`, `ensure_destination` and desktop directory lookup/picker usage. Remove Windows video payload streaming/publication and obsolete transfer phases. Move the existing `hash_path` checksum helper to `helper.rs` for resource deployment; do not remove its `sha2` use or confuse helper upload/checksum with video transfer. Do not touch existing user directories during migration.
- [ ] **Step 5: Observe GREEN** with `cargo test --locked -p takedock --lib`, `cargo clippy --locked -p takedock --lib -- -D warnings` and Task 1's helper suite. Expected: all pass, including existing readiness/discard/result-expiry tests; no local external fixture execution.
- [ ] **Step 6: Commit** as `feat: replace desktop transfer jobs with guarded phone handoff`.

### Task 3: Source-only Mark UI, reconciliation and focus

**Files:** Create `src/lib/handoff-updates.ts` and `.test.ts`; modify `src/lib/types.ts`, `ipc.ts`, `session.svelte.ts`, `App.svelte`, `components/VideosPanel.svelte`, `SettingsPanel.svelte`, `ActivityPanel.svelte`, `lib/i18n.ts`, and corresponding component/session/result tests.

**Interfaces:** TypeScript `JobKind = 'mark' | 'delete' | 'rename'`; `HandoffUpdate` matches Task 2 exactly, with `source_identity: string` left opaque. `HandoffUpdates.reset(generation: number): void`, `.record(update: HandoffUpdate): boolean`, `.apply(videos: Video[], generation: number): Video[]`. Add `pendingKeys?: string[]` to `VideosPanel` using existing `videoKey`; session tracks initiating selections until IPC rejection/terminal settlement and rejects further local commands for those keys. Backend reservations remain authoritative across windows.

- [ ] **Step 1: Add failing UI/controller tests.** In `handoff-updates.test.ts`, assert a matching source disappears, a reused basename with another identity remains, duplicates/older generations are ignored, multiple per-file receipts from one job all apply, and a delayed rename/list response cannot resurrect a handed-off exact identity. Include integer identity fields beyond 2^53 as strings. Component tests pin:

```ts
expect(screen.queryByRole('button', { name: 'Copy to computer' })).toBeNull();
expect(screen.queryByRole('button', { name: 'Move to computer' })).toBeNull();
await user.click(
  screen.getByRole('checkbox', { name: `Select ${video.name}` }),
);
expect(onJob).not.toHaveBeenCalled();
await user.click(screen.getByRole('button', { name: 'Mark selected' }));
expect(onJob).toHaveBeenCalledWith('mark', [video], undefined, generation);
```

Add direct per-row Mark, protected/pending rows, failure retention and focus removal cases (next row, previous row, empty list toolbar). Add settings tests proving no destination field/folder picker and preserved unrelated values after legacy bootstrap. Keep rename accessible-name, silent-result/history and native refresh-selector regressions meaningful.

- [ ] **Step 2: Observe RED** using `npm run test:ui -- src/lib/handoff-updates.test.ts src/components/VideosPanel.test.ts src/components/SettingsPanel.test.ts src/lib/session.test.ts`.
- [ ] **Step 3: Implement the interfaces and source actions.** Use native action buttons: per-row `Mark` / `تعليم` with filename in the accessible name, and `Mark selected` / `تعليم المحدد` for the selected batch. Keep selection checkboxes for choosing Delete/Rename/Mark; they do not themselves move files. Add a short explanation that Mark sends the file to `DCIM/TakeDock` and removes it from this list. No marking confirmation, destination view or return action. Apply confirmed updates across windows, invalidate in-flight source refreshes, and filter stale rename/list results using exact source identity. Pending source claims never optimistically remove rows. Unknown outcomes request only a fresh source listing, never a mutation retry or destination traversal. Preserve focus only when the removed row owned focus; do not move it for unrelated background updates.
- [ ] **Step 4: Remove desktop UI/settings/progress.** Remove the folder picker from `ipc`/`App`/`SettingsPanel`, Copy/Move labels and payload MB/percentage display. Keep ordinary active job counts, cancellation, indeterminate progress, result expiry/history and all recording/updater behavior. Ensure no rendered control or accepted frontend job kind can launch a desktop video copy.
- [ ] **Step 5: Observe GREEN** with `npm run test:ui`, `npm run check`, `npm run format:check`, `npm run build`. Expected: all UI tests pass; zero checker warnings/errors; formatting and build pass. Do not interpret DOM tests as a listening NVDA audit.
- [ ] **Step 6: Commit** as `feat: hand off marked source videos and remove desktop transfer controls`.

### Task 4: GitHub-only process/native acceptance cutover

**Files:** Modify `tests/fixtures/adb/src/main.rs`, its `tests/files.rs`, `discard.rs`, `discard_engine.rs` and other compilation callers; `tests/desktop/fixtures.ts`, `critical.e2e.ts`, `wdio.conf.ts` only where needed; `.github/workflows/ci.yml`. Adapt `src-tauri/examples/device-files.rs` to source Delete/Rename/Mark with manifest-owned targets and the new `start_job` signature. Review diagnostic-feature references in both Cargo manifests and workflow commands. No local build/run/import of the fixture.

**Interfaces:** Fixture storage has `videos` as managed source and `handoff` as destination; only its peer implementation maps the production fixed root to this test folder. Implement `handoff-guarded` through Task 1's real library/protocol, with hosted-only peer controls `hold-handoff-before-permit`, `hold-handoff-after-commit`, `drop-handoff-result`, `fail-handoff-index`, and `handoff-started` evidence. No application test IPC or shipped fault switches. Legacy settings are test inputs, never Windows transfer targets.

- [ ] **Step 1: Add the new integration assertions before implementing the peer handoff handler.** Adjust retired API callers/obsolete transfer handlers enough to compile, then require actual source relocation, no destination overwrite, retained/unknown distinctions, duplicate reservation rejection, no payload `read`/`hash` request, and source-only listing after restart. In native E2E, retain exactly four mandatory cases: offline keyboard/silent defaults; recording dispatch/focus while verification/file work is held; source file management including batch Mark, rename, discard and result behavior; Arabic/unrelated settings persistence plus obsolete destination migration. Replace held-hash scenarios with the corresponding held handoff boundary while preserving their original independence/focus/result assertions. Capture pre-move expected hashes, then assert:

```ts
assert.equal(existsSync(join(fixtureRoot, 'videos', name)), false);
assert.equal(hash(join(fixtureRoot, 'handoff', name)), expectedHash);
assert.equal(await sourceRow(name).isExisting(), false);
```

Define `sourceRow(name)` as a filename-safe native row locator in `critical.e2e.ts`; never interpolate an unescaped XPath literal. Add an unrelated handoff sentinel before startup and prove refresh/reconnect, source rename/delete and process restart leave it intact. Include destination conflict created after preparation, dropped result after commit and warning after success.

- [ ] **Step 2: Observe hosted RED.** Push the test commit on `abdo/phone-file-handoff` and create/attach a draft PR with the new scope using a body file or structured argument. Use the existing Validate workflow. Confirm the new handoff case fails because the peer does not implement it yet, not because a runner/toolchain is unavailable. Preserve that run as RED evidence. Wait on meaningful workflow transitions with backoff; do not repeatedly poll unchanged jobs. Do not merge the older transfer PR.
- [ ] **Step 3: Implement the fixture handoff peer and finalize acceptance adaptation.** Remove retired copy/hash paths from active acceptance and feature-specific diagnostic tests; preserve relevant source/mutation safety cases under Mark. Keep four required native cases, strict no-skip/error enforcement, Observer/updater/security/packaging gates and the all-jobs quality gate. Retain historical transfer reports/tools without claiming they test the new workflow. Update project usage documentation that still directs users to desktop Copy/Move.
- [ ] **Step 4: Observe hosted GREEN on the resulting final tree.** Required GitHub commands include `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the desktop-feature build/checks and actual release WebView2 four-case acceptance through the existing workflow. Locally run only allowed pure library/helper/UI/format checks. Require all five Validate jobs successful, four native cases passed, no skips; pin artifact/head/tree provenance. Do not weaken a gate to obtain green.
- [ ] **Step 5: Commit/push** the completed acceptance cutover as `test: validate source-only handoff through native desktop IPC` and record the hosted RED/GREEN evidence. Re-run only for new changes, failures or unresolved concerns.

### Task 5: Real-phone verification, latency evidence and Native review

**Files:** Create `scripts/test-phone-handoff.ps1`, `docs/research/2026-10-25-phone-file-handoff.md`; maintain a fresh private `test-results/phone-handoff/<trial-id>/` manifest/journal. Keep all prior trial evidence read-only.

**Interfaces:** Test controller parameters: `Executable`, `Adb`, `Serial`, `Manifest`, `TrialRoot`, `TauriDriver`, `EdgeDriver`, all explicit strings. Use the actual native application and already audited native WebDriver transport, never `tests/desktop/fixtures.ts` or a substitute ADB. Reuse existing preservation/path-containment utilities where compatible; a new handoff manifest must never authorize paths from an old transfer journal.

- [ ] **Step 1: Prepare a reviewable owned-clip trial.** Verify the real SDK/physical phone, current pairing, native binary/resources/source provenance, current settings and unrelated-file baselines. Build the changed Android helper with the existing `scripts/build-file-helper.ps1` and native app through the scoped Tauri build, without any workspace/fixture build. Use the ordinary release-feature binary for behavior. If backend phase timestamps require the existing diagnostic feature, use a separately identified matching-source `desktop,transfer-diagnostics` timing build and distinguish it from release acceptance in the report. Use fresh names and a small finalized clip plus a 500,000,000-byte finalized test video, derived only from owned verified test assets. Preserve/check/decode local reference backups before mutation, and bind each new source's size/hash/identity to that reference in the new manifest. Never select personal recordings. Source operations can run with the phone Home/asleep; open the camera briefly only for an actually required recording check.
- [ ] **Step 2: Exercise actual native controls and capture evidence.** Mark a single clip and a batch, rename/delete dedicated source fixtures, retain a collision source, and confirm handed-off files appear through the registered Phone Link root with the expected bytes/hash. Leave a handed-off sentinel through app refresh/restart and source operations; assert it is untouched. Measure action-to-visible-settlement and backend acceptance/commit/result separately for first-use and warm small/large marks. Payload hash/decode and backup preparation stay outside operation timing. A short metadata exchange, rather than video bytes or full-file hashing, must be the production path; report actual timings without a promised target.
- [ ] **Step 3: Close every trial safely, including failures.** Preserve current settings and shared ADB/pairing; close only owned app/driver processes, return Home/sleep and verify the camera is not left foreground. Reconcile uncertain mutations before any further mutation. The test harness may inspect its exact owned destination fixtures for verification, but must never add destination management to the app or sweep a folder. Record actual Windows/Android and what screen-reader checks were and were not performed.
- [ ] **Step 4: Write the real report and verify the final tree.** Include native timings, integrity/ownership/failure results, hosted acceptance references and limitations. Run appropriate allowed local checks once after final changes; use a new hosted run only if tested code/resources changed. Complete a spec-to-tests coverage check, including retirement of the exact discard target and absence of release desktop-transfer paths.
- [ ] **Step 5: Perform Native whole-branch review.** Invoke the required requesting-code-review/Native review workflow with a fresh reviewer on the most capable allowed model, supplying the approved spec, this plan, base/head, full diff and actual verification evidence. Resolve material findings and run the affected checks; do not claim a reviewer message alone proves success. Finish the branch using the preserved workflow, update/attach the draft PR with accurate final scope and evidence, and ask only for any still-ungranted integration action. No automatic merge or release.

## Plan Self-Review

- Helper identity/no-overwrite/journal/cancellation contracts: Tasks 1–2; platform behavior: Tasks 4–5.
- User-selected one-way ownership boundary, row removal, pending state and no destination collection: Tasks 2–5.
- Delete/Rename/discard/recording and shared result/accessibility preservation: Tasks 2–5.
- Desktop feature/settings removal, legacy migration and release acceptance: Tasks 2–4.
- Review Focus items each have an owning test step above. `Outcome`, `Receipt`, `HandoffUpdate`, `HandoffCommit`, `start_job`, `retire_handed_off` and `HandoffUpdates` use the same names/types throughout.
- This is one coupled feature cutover, not separate product projects. No product code, dependencies, tests, phone mutations, CI dispatch or release action is authorized merely by writing this plan; wait for its review, then execute using the user's preserved Native choice.
