# Recording Speech-Readiness Cue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Play one dependable PC-headphone speak-now cue for a confirmed new take, without delaying capture dispatch or replaying stale cues.

**Architecture:** A pure Rust readiness state machine is owned by the existing recording machine under its authoritative lock. The session timer advances it; a process-owned audio worker claims each cue immediately before playback. Svelte renders state and settings but never plays or schedules the cue.

**Tech Stack:** Existing Rust/Tauri 2, Svelte 5/TypeScript, Vitest/Testing Library, real-process ADB fixtures, WebdriverIO/WebView2, and the Windows `Beep` API. No new third-party dependency.

**Spec:** [Approved recording-readiness design](../specs/2026-10-15-recording-readiness-design.md). Read the spec and its [measurement report](../../research/2026-10-15-recording-startup-measurements.md) before executing.

**Execution update, 2026-10-19:** The user selected Native execution. Tasks 1–6
are implemented, with the one whole-branch review and its TDD fix pass. The user
later authorized an automated external-speaker/known-speech procedure in place
of manual headphone/hum trials. Current-device evidence and the native 500 ms
fast-Stop prerequisite are in the
[acceptance report](../../research/2026-10-19-recording-readiness-acceptance.md).

## Global Constraints

- One 1100 Hz, 100 ms tone through the Windows session's default audio output; do not change system volume or switch the user's output device.
- `ready_sound` is enabled by default and independent of `success_sound`.
- `ready_delay_ms` is initially 800 ms, integer multiples of 50 in the range 500–5000, measured after confirmation of the current Start.
- `speech_readiness` is `inactive`, `preparing`, or `ready`; the Rust session owns it.
- Exact state copy: **Preparing to speak** / **جارٍ التجهيز للكلام** and **Ready to speak** / **جاهز للكلام**.
- The cue is for a newly requested Start only. Resume and connecting to an already recording phone do not replay it.
- Settings changes need no reconnect: a new delay and re-enabling apply to the next Start; disabling invalidates pending playback while visual readiness can still advance.
- When enabled for a Start, suppress that Start's generic success tone. Preserve other success/failure feedback.
- Keep command delivery independent of verification, readiness timing, audio playback, and file jobs. Never automatically retry a capture toggle.
- Generation, Start sequence, and a unique readiness token identify a candidate; cancellation before playback commit prevents the cue. A 100 ms tone already committed may finish.
- No TTS, live regions, automatic focus moves, new audio package, production test server, or test-control IPC.
- Preserve existing settings and supported Windows/Android targets. Compilation and fixtures do not certify phone audio or audible routing.
- No automatic trimming, modification of Open Camera, dependency upgrade, version publication, or unrelated requested feature in this component.

## Review Focus

1. An old ready event delivered after a new generation/take must not replace its queued cue; pin this in Task 4's `older_ready_event_cannot_replace_newer_pending_cue`.
2. Queue failure after a later capture intent must not restore an old readiness token from model rollback; pin this in Task 3's `queue_failure_cannot_resurrect_readiness`.
3. Disable/re-enable between enqueue and playback must not revive a cue for the current take; pin this in Tasks 2 and 4's held-playback tests.
4. Unknown state before confirmation may recover before the verification deadline; unknown after confirmation cancels permanently; pin both paths in Task 2.
5. A failed audio backend must report an ordinary error once without recursively scheduling failure sounds or holding capture locks; pin this in Task 4 and its held-audio integration test.

## File Structure

| File | Responsibility |
| --- | --- |
| `src-tauri/src/model.rs`, `settings.rs`; `src/lib/types.ts` | Wire/persistence contracts and legacy defaults |
| New `src-tauri/src/readiness.rs` | Pure readiness timing, candidate identity, cancellation and playback claim |
| `src-tauri/src/recording.rs`, `session.rs`, `events.rs` | Integrate capture causality, timer effects and feedback suppression |
| New `src-tauri/src/audio.rs`; `desktop.rs` | One audio worker, system tone backend and runtime wiring |
| `src/components/RecordingPanel.svelte`, `SettingsPanel.svelte`; `src/lib/i18n.ts` | Bilingual ordinary state and labeled settings |
| Existing Rust/process/component/Desktop E2E tests | Regression and integration evidence |
| New `docs/testing/recording-readiness.md` | Physical acceptance procedure and limits |

## Task 1: Persist backward-compatible settings and wire state

**Files:** Modify `src-tauri/src/model.rs`, `src-tauri/src/settings.rs`, `src/lib/types.ts`; tests in `settings.rs`. Update explicit typed literals found with `rg 'Settings \{|SessionState \{|Settings =|SessionState =' src src-tauri tests` only where required.

**Interfaces:**
- Produces Rust `SpeechReadiness { Inactive, Preparing, Ready }`, serde snake_case, default `Inactive`; `SessionState.speech_readiness: SpeechReadiness`.
- Produces `Settings.ready_sound: bool = true` and `Settings.ready_delay_ms: u64 = 800`, using the existing serde default strategy.
- Produces matching TS `SpeechReadiness`, `SessionState.speech_readiness`, and settings fields; `initialSession` uses `inactive` and `defaultSettings` uses `true`/`800`.
- Existing `settings::load` and `settings::save` signatures remain unchanged; validation rejects delays outside 500–5000 or not divisible by 50 with `invalid_settings`.

- [x] **Step 1: Establish the execution checkout and baseline.** After plan approval/method selection, use `superpowers:using-git-worktrees`, inspect applicable instructions, and start from this branch's approved spec/plan commits. Install missing compatible build tools rather than weakening checks. Run `npm ci`, then `cargo test --workspace --locked`, `npm run check`, and `npm run test:ui`; record any pre-existing failure before changes.
- [x] **Step 2: Write failing legacy/default and validation tests in `settings.rs`.** Assert loading JSON with only the five old fields preserves all five, adds `ready_sound == true` and `ready_delay_ms == 800`; edited `false`/`1250` survives save/load. For `499`, `501`, `525`, and `5001`, save must fail and retain the last good bytes. `500`, `800`, and `5000` must pass. Assert default session readiness serializes as `"inactive"`.

```rust
#[test]
fn legacy_settings_default_readiness_without_replacing_language() {
    let saved: Settings = serde_json::from_str(
        r#"{"language":"ar","destination":"","adb_path":"","success_sound":false,"verification_ms":10000}"#,
    ).unwrap();
    assert_eq!(saved.language, "ar");
    assert_eq!((saved.ready_sound, saved.ready_delay_ms), (true, 800));
}
```

- [x] **Step 3: Run `cargo test -p takedock --lib settings --locked`.** Expected RED: the new fields/type or checks are missing, not an unrelated environment failure.
- [x] **Step 4: Add the contracts and validation above.** Update required TS literals without adding UI behavior yet.
- [x] **Step 5: Run the focused Rust tests and `npm run check`.** Expected PASS, with no Svelte/TS diagnostics. Run `cargo fmt --all --check` after formatting changed Rust.
- [x] **Step 6: Commit** `feat: persist speech-readiness settings and state`, staging only this task's contracts/tests.

## Task 2: Implement the pure readiness state machine

**Files:** Create `src-tauri/src/readiness.rs`; export it in `src-tauri/src/lib.rs`. Tests live in the new module.

**Interfaces:** Consumes Task 1's `SessionState`, `SpeechReadiness`, and settings values. Produces these types and exact methods:

```rust
pub struct ReadinessPolicy { pub sound: bool, pub delay_ms: u64 }
pub struct ReadyToken { pub generation: u64, pub start_sequence: u64, pub id: String }
pub struct Readiness { /* private state */ }
impl Readiness {
    pub fn new(generation: u64) -> Self;
    pub fn begin(&mut self, token: ReadyToken, policy: ReadinessPolicy) -> bool;
    pub fn confirm(&mut self, start_sequence: u64, now_ms: u64);
    pub fn reconcile(&mut self, state: &SessionState);
    pub fn poll(&mut self, now_ms: u64, state: &SessionState) -> Option<ReadyToken>;
    pub fn claim(&mut self, token: &ReadyToken, state: &SessionState) -> bool;
    pub fn disable_sound(&mut self);
    pub fn cancel(&mut self);
    pub fn state(&self) -> SpeechReadiness;
}
```

`ReadinessPolicy` is Copy/Clone; `Readiness` is Clone; `ReadyToken` is Clone/Debug/PartialEq/Eq/Serialize/Deserialize. Tests supply deterministic token IDs. `begin` rejects a foreign generation without mutation. `poll` updates state even if it returns no audible cue. A successful `claim` consumes playback permission once.

- [x] **Step 1: Write deterministic tests with explicit clock values.** Use an eligible recording `SessionState` at generation 7, sequence 1, zero pending. Begin token `one` with sound on/800 ms; polling at 10,000 before confirmation returns none; confirm sequence 1 at 20,000; poll 20,799 returns none/preparing; poll 20,800 returns token one/ready; repeated poll returns none; claim succeeds once then fails. Reconfirm at 20,400 must not shift the deadline.

```rust
#[test]
fn deadline_begins_at_confirmation_and_cue_is_claimed_once() {
    let session = SessionState {
        generation: 7, command_sequence: 1, connected: true,
        observer_ready: true, foreground: true, video_mode: true,
        predicted: RecordingState::Recording, observed: RecordingState::Recording,
        ..Default::default()
    };
    let token = ReadyToken { generation: 7, start_sequence: 1, id: "one".into() };
    let mut readiness = Readiness::new(7);
    assert!(readiness.begin(token.clone(), ReadinessPolicy { sound: true, delay_ms: 800 }));
    assert_eq!(readiness.poll(10_000, &session), None);
    readiness.confirm(1, 20_000);
    readiness.confirm(1, 20_400);
    assert_eq!(readiness.poll(20_799, &session), None);
    assert_eq!(readiness.state(), SpeechReadiness::Preparing);
    assert_eq!(readiness.poll(20_800, &session), Some(token.clone()));
    assert_eq!(readiness.state(), SpeechReadiness::Ready);
    assert_eq!(readiness.poll(20_801, &session), None);
    assert!(readiness.claim(&token, &session));
    assert!(!readiness.claim(&token, &session));
}
```

- [x] **Step 2: Add cancellation/policy assertions.** `cancel`, foreign generation, later command sequence, stop, pause, and loss of eligibility prevent claim. `unknown_before_confirmation_may_recover` stays preparing and may confirm later; `unknown_after_confirmation_cancels_permanently` cannot recover a cue. `disable_then_reenable_does_not_revive_current_cue` asserts disabling between enqueue/claim makes claim false while visual state can become ready; creating another policy must not restore the old token. Sound-off starts become visually ready without a cue. Test 500/5000 ms guards and saturating deadline arithmetic near `u64::MAX`.
- [x] **Step 3: Run `cargo test -p takedock --lib readiness --locked`.** Expected RED for the absent state machine.
- [x] **Step 4: Implement the interfaces using a private waiting/warming/ready candidate.** Reconciliation checks generation, latest command, eligibility, predicted/observed recording and absence of later pending capture. Do not use wall-clock time, sleeping threads, or sound APIs here.
- [x] **Step 5: Run the readiness tests.** Expected PASS deterministically without a phone or audio device.
- [x] **Step 6: Commit** `feat: model confirmed speech-readiness deadlines and cancellation`.

## Task 3: Bind readiness to actual capture causality

**Files:** Modify `src-tauri/src/recording.rs`, `session.rs`, `events.rs`, `model.rs`; extend `tests/fixtures/adb/tests/session.rs`. Existing Observer protocol stays unchanged.

**Interfaces:**
- `RecordingMachine` owns Task 2's `Readiness` and current `ReadinessPolicy` under the existing session model lock.
- Change `RecordingMachine::observe(&mut self, observation: Observation, now_ms: u64) -> Vec<VerificationResult>` and update all callers/tests. Preserve existing causality rules.
- Add `RecordingMachine::set_readiness_policy(&mut self, policy: ReadinessPolicy)`, `invalidate_speech_readiness(&mut self)`, `poll_speech_readiness(&mut self, now_ms: u64) -> (bool, Option<ReadyToken>)`, and `claim_ready_cue(&mut self, token: &ReadyToken) -> bool`. The tuple's bool means the serialized readiness state changed.
- Add `Engine::set_readiness_policy(&self, policy: ReadinessPolicy)`, `invalidate_speech_readiness(&self)`, and `claim_ready_cue(&self, token: &ReadyToken) -> bool`.
- Add `VerificationResult.action: RecordingAction` and `suppress_success_sound: bool`, captured in each `Pending` entry. Existing frontend consumers tolerate the additional fields.
- Emit `speech-ready` with serialized `ReadyToken` only once; `session-state` carries the ordinary readiness state. `Engine::start` keeps its current three-argument signature.

- [x] **Step 1: Write failing model and process regression tests.** `confirmed_start_arms_readiness_but_resume_does_not`, `superseded_confirmation_never_arms_readiness`, `queue_failure_cannot_resurrect_readiness`, and `ready_settings_apply_without_reconnect` assert the spec's ordering. For queue failure, use a private `#[cfg(test)]` session harness with a full one-slot command channel, an eligible recording model and an armed token; call the actual `Engine::dispatch_for(Stop, generation)` and assert capture prediction rolls back while readiness becomes inactive and the token cannot be claimed. Do not add production control IPC. In `events.rs`, assert a confirmed Start with `suppress_success_sound: true` produces no generic tone, duplicates remain silent, and failures/other actions retain their feedback.

```rust
#[test]
fn confirmed_start_does_not_play_early_success_feedback() {
    let mut events = Events::default();
    events.receive("session-state", &serde_json::json!({"generation": 7}));
    let result = serde_json::json!({"generation":7,"sequence":1,"action":"start",
        "status":"confirmed","suppress_success_sound":true});
    assert_eq!(events.receive("verification-result", &result), None);
    assert_eq!(events.receive("verification-result", &result), None);
}
```

- [x] **Step 2: Extend the existing process fixture tests.** Hold observation, dispatch Start/Pause/Resume/Stop, and assert all four commands reach `commands.jsonl` before release, with no `speech-ready` event. For a normally confirmed Start, wait for exactly one event and ready state; engine claim succeeds once. Disable then re-enable before the deadline and assert no current-take cue; a later Start uses the new settings. Assert reconnect/retired generations cannot claim old tokens.
- [x] **Step 3: Run** `cargo test -p takedock --lib recording --locked`, `cargo test -p takedock --lib session --locked`, `cargo test -p takedock --lib events --locked`, and `cargo test -p takedock-adb-fixture --test session --locked`. Expected RED for missing readiness behavior/interfaces.
- [x] **Step 4: Integrate the lifecycle.** Generate each production token ID with the already-used `uuid` crate. Begin a candidate while queueing Start under the model lock; a failed enqueue must restore capture state and then invalidate readiness before unlocking. Emit any resulting state outside locks. Record the Start's sound-enabled snapshot for generic-feedback suppression.
- [x] **Step 5: Extend the existing 25 ms session timer.** Collect expiry results, readiness state changes, and a possible cue under one model lock; emit effects afterward. Cancel through all spec boundaries, including channel failure, lost eligibility, shutdown and generation replacement. A transient unknown before confirmation waits; one after confirmation cancels. Engine claims recheck the current generation while holding the authoritative model lock.
- [x] **Step 6: Run all Task 3 commands and `cargo test --workspace --locked`.** Expected PASS; no capture retry or verification/audio wait enters the command dispatch path.
- [x] **Step 7: Commit** `feat: integrate speech readiness with capture verification`.

## Task 4: Serialize and validate native audio playback

**Files:** Create `src-tauri/src/audio.rs`; export in `lib.rs`; modify `desktop.rs`; tests in `audio.rs` and `tests/fixtures/adb/tests/session.rs`.

**Interfaces:** Consumes `ReadyToken` and `Engine::claim_ready_cue`. Produces:

```rust
pub enum Tone { Success, Failure, SpeechReady }
pub type PlayTone = Arc<dyn Fn(Tone) -> Result<(), String> + Send + Sync>;
pub type ClaimReady = Arc<dyn Fn(&ReadyToken) -> bool + Send + Sync>;
pub type AudioFailure = Arc<dyn Fn(Option<ReadyToken>, String) + Send + Sync>;
pub struct AudioDispatcher { /* queue and one worker */ }
impl AudioDispatcher {
    pub fn start(play: PlayTone, claim: ClaimReady, failed: AudioFailure) -> Self;
    pub fn feedback(&self, success: bool);
    pub fn ready(&self, token: ReadyToken);
    pub fn shutdown(&self);
}
pub fn play_system_tone(tone: Tone) -> Result<(), String>;
```

`Tone` is Copy/Clone. A mutex/condition-variable queue holds at most one newest ready token, sixteen failure requests, and one coalesced optional success. After the current tone, process ready, then failure, then success. Ready token replacement compares `(generation, start_sequence)` so late older events cannot displace newer candidates.

- [x] **Step 1: Write failing audio tests using injected players and explicit barriers.** Assert one playback despite duplicate events; `older_ready_event_cannot_replace_newer_pending_cue`; cancellation/disable then re-enable while the player is held prevents the pending cue; readiness precedes queued optional success; shutdown clears unplayed cues. Do not assert scheduler behavior using short sleeps.
- [x] **Step 2: Pin failure and dispatch independence.** A fake player returning an error must invoke the ordinary-error callback once for that ready token, make no retry, and never enqueue a failure tone through that callback. Hold a currently playing tone and prove capture commands still reach the real fixture process. These tests must not play actual sound.

```rust
#[test]
fn failed_ready_tone_reports_once_without_audio_retry() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let played = calls.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let token = ReadyToken { generation: 7, start_sequence: 1, id: "one".into() };
    let audio = AudioDispatcher::start(
        Arc::new(move |_| { played.fetch_add(1, std::sync::atomic::Ordering::SeqCst); Err("device_error".into()) }),
        Arc::new(|_| true),
        Arc::new(move |token, _| { tx.send(token).unwrap(); }),
    );
    audio.ready(token.clone());
    assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap(), Some(token));
    audio.shutdown();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(rx.try_recv().is_err());
}
```

- [x] **Step 3: Run `cargo test -p takedock --lib audio --locked` and the process session tests.** Expected RED for the absent dispatcher/behavior.
- [x] **Step 4: Implement the dispatcher and system backend.** Recheck/claim only immediately before playback; never hold queue/model/settings locks while calling the player. Preserve existing success notes `(880,60),(1175,90)` and failure notes `(330,110),(220,160)`; SpeechReady is `(1100,100)`. Check Windows `Beep` failure. The non-Windows backend returns `sound_output_unavailable`; injected players keep portable tests independent of hardware.
- [x] **Step 5: Wire the process-owned runtime.** Make `Runtime.engine` an `Arc<RwLock<Option<Arc<Engine>>>>`, created before the event sink. The audio claim callback holds a Weak reference to that slot, clones the current Engine under the read lock, releases the slot lock, then calls its claim method. This avoids ownership cycles and holding runtime locks during playback.
- [x] **Step 6: Route existing feedback and new ready events through this dispatcher.** Replace spawning one audio thread per feedback. `play_feedback` still honors the success preference. Serialize settings save with engine replacement using the existing reconnect mutex: acquire reconnect, then settings, persist/update settings, clone the current engine from the slot and release the slot lock, update policy, then broadcast under settings serialization. Reconnect/shutdown must extract an old engine from the slot in a separate scope before calling or joining it; never hold the slot while acquiring settings/model locks or playing/joining audio. This preserves authoritative settings ordering and prevents a newly replaced engine using an older policy. Readiness audio failures emit `operation-error` with `ready_sound_failed` directly through the app handle, bypassing the sound-generating event sink to prevent recursion. Deduplicate continuous generic audio failures with `feedback_sound_failed`; do not claim they were heard. Join the worker at shutdown after invalidating engine state.
- [x] **Step 7: Run the focused audio/process tests and the workspace suite.** Expected PASS with no audible hardware dependency.
- [x] **Step 8: Commit** `feat: dispatch readiness audio once with cancellation checks`.

## Task 5: Expose bilingual readiness and independent controls

**Files:** Modify `src/components/RecordingPanel.svelte`, `RecordingPanel.test.ts`, `SettingsPanel.svelte`, `SettingsPanel.test.ts`, `src/lib/i18n.ts`, `src/lib/session.test.ts`; update necessary typed fixtures from Task 1 only.

**Interfaces:** Consumes backend `SessionState.speech_readiness` and Task 1 settings. Adds ordinary paragraph `#speech-readiness` with `data-readiness="preparing"` or `"ready"`; omit it for inactive. Add inputs `#ready-sound`, `#ready-delay` and stable `#success-sound`; retain the current SettingsPanel callbacks and form submission API.

- [x] **Step 1: Write failing component tests.** Render preparing then ready with the existing capture button focused; assert exact English/Arabic spec copy, the same focused button, and no `[aria-live]`, `role="alert"`, or `role="status"`. Assert inactive omits the paragraph and a rerender never calls an audio IPC method.
- [x] **Step 2: Write failing settings tests.** Readiness is checked by default while optional success is unchecked. Save `ready_sound: false`, `ready_delay_ms: 1250`, language `ar` and independent success state; assert callback values and RTL. Assert `min=500`, `max=5000`, `step=50`, `required`, native labels and help associations. Invalid/empty input cannot submit. A settings broadcast updates a second session without calling `ipc.sound`.

```ts
it('defaults readiness independently of optional success feedback', () => {
  render(SettingsPanel, {
    settings: defaultSettings, onSave: vi.fn(), onFolder: vi.fn(),
    onAdb: vi.fn(), onReconnect: vi.fn(),
  });
  expect(screen.getByRole('checkbox', { name: 'Play the ready-to-speak cue' })).toBeChecked();
  expect(screen.getByRole('checkbox', { name: 'Play a sound on success' })).not.toBeChecked();
  expect(screen.getByLabelText('Speech-readiness delay after confirmation (milliseconds)')).toHaveValue(800);
});
```

- [x] **Step 3: Run** `npm run test:ui -- src/components/RecordingPanel.test.ts src/components/SettingsPanel.test.ts src/lib/session.test.ts`. Expected RED for missing UI/state or copy.
- [x] **Step 4: Implement the small render/form changes.** Use the exact setting labels in the approved spec. Add `readySound`, `readyDelay`, `readyDelayHelp`, `preparingToSpeak`, `readyToSpeak`, `ready_sound_failed`, `feedback_sound_failed` in both dictionaries. The helper says the interval follows confirmation, applies next Start, and reducing it requires checking initial words. Do not add frontend timers, audio playback, or new listeners for cue replay.
- [x] **Step 5: Run the focused tests, `npm run check`, `npm run format:check`, and `npm run build`.** Expected PASS; format only changed files if needed.
- [x] **Step 6: Commit** `feat: expose accessible speech-readiness state and settings`.

## Task 6: Prove release integration and physical speech acceptance

**Files:** Modify `tests/desktop/critical.e2e.ts`, `tests/desktop/fixtures.ts`, `docs/testing/desktop-e2e.md`, `docs/setup.md`, `docs/setup.ar.md`, `docs/keyboard.md`, `docs/keyboard.ar.md`; create `docs/testing/recording-readiness.md`. Record sanitized measured results in that document; keep raw phone clips/logs in ignored `test-results/`.

**Interfaces:** Use existing `restartFixture`, `button`, `shortcut`, `waitFor`, `commands`, `flag`, `inject` and the normal rendered state/settings. Keep exactly four mandatory release Desktop E2E cases; extend their assertions without adding a production control hook.

- [x] **Step 1: Extend the four existing E2E cases before final implementation polish.** Use explicit `#success-sound` selectors instead of ambiguous first-checkbox selectors. Assert default readiness on/800, a held confirmation remains preparing, rapid Start/Pause/Resume/Stop never becomes ready for the cancelled take, and a separately confirmed Start does become ready after its guard. Open a second window and assert the shared state/settings. Save readiness off/1250 with Arabic and prove restart persistence without reconnect. Retain existing copy/integrity/focus assertions.
- [x] **Step 2: Run the extended integration cases against the new release build.** Unit/component RED evidence from earlier tasks remains required; do not rebuild an older binary solely to duplicate it or weaken the four-case release gate. Add coverage for recording becoming ready while a fixture file job is held, using the existing `hold-hash` flag and releasing it in `finally`. If an integration assertion fails, diagnose the real IPC/state behavior before changing expectations.
- [x] **Step 3: Build the real resources and run complete checks.** Reuse verified unchanged Observer/helper resources if available; otherwise use `scripts/build-observer.ps1` and `scripts/build-file-helper.ps1`, installing compatible missing tools. Use a signing-compatible Observer for the physical phone and do not uninstall its existing package to bypass signature failure. Run the following once after final changes:

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --features desktop --locked -- -D warnings
cargo test --workspace --locked
npm run format:check
npm run check
npm run test:ui
npm run build
node --test scripts/release-gate.test.mjs scripts/prepare-publication.test.mjs tests/desktop/tooling.test.mjs
npm run desktop:build -- --no-bundle
./scripts/check-updater.ps1
./scripts/setup-desktop-drivers.ps1
npm run test:desktop
```

Use the existing `run-desktop-unprivileged.ps1` wrapper only if the current execution context needs it; inspect that script before use. Expected: all checks pass, four real release desktop cases pass, and an acceptance report identifies the tested executable/resources. No release tag/publication is part of this task.

- [x] **Step 4: Perform the approved physical acceptance with the new release EXE.** Record its hash and current phone/Open Camera/Windows versions; save the existing app settings and phone accessibility state. Select real ADB, 800 ms and PC headphones; open Open Camera idle in video mode. For ten starts through the real app, the user supplies continuous audible hum before Start and marked words after the ready cue finishes. Stop each take, copy only newly created test fixtures, capture the native logs, and decode both tracks with `ffprobe`/`ffmpeg`. Verify complete words and that the cue follows the measured onset/ramp; record timing uncertainty honestly. Headphone-only PC tones cannot serve as an acoustic reference reaching the phone. An external microphone is not required for this acceptance.
- [x] **Step 5: Exercise cancellation/output cases on the release app.** Stop/Pause before the guard, disconnect before the guard, open another window during preparation, disable/re-enable while pending, and verify a background file operation does not block recording. Record whether each pending cue was suppressed and whether one ordinary Start gave exactly one headphone cue. Restore settings only if no intervening user change superseded the saved value. Remove only test-created phone fixtures after verified copies, and leave capture stopped. If acceptance fails, use systematic debugging and revise the design/default with the user; do not declare the cue safe or just keep increasing sleeps.
- [x] **Step 6: Document the new workflow and evidence.** Explain that speaking starts after the PC cue, its separate setting, the meaning of preparing/ready, cancellation, and the remaining preamble. Synchronize English/Arabic user guidance. Report actual build/device coverage and any unperformed physical case; never substitute fixture success for audible/phone acceptance.
- [x] **Step 7: Obtain the selected workflow's independent review, address actionable findings with regression tests, then rerun only affected checks.** Use verification-before-completion and finishing-a-development-branch at the appropriate execution stage.
- [x] **Step 8: Commit** `test: verify release speech-readiness workflow` when its checks and evidence are accurate.

## Plan Self-Review and Handoff

Coverage map: persistence/migration → Task 1; timing and cancellation → Task 2; command causality, stale events and suppression → Task 3; global audio, claiming, routing and errors → Task 4; bilingual accessible controls → Task 5; EXE integration and actual speech evidence → Task 6. All five Review Focus items have named owning tests. No broader requested work is dropped; it remains listed in the approved spec.

The user approved the plan and chose **Native** on 2026-10-19. Implementation,
the one independent review, its regression-tested correction, and the amended
real-device acceptance are complete. Integration remains the user's decision;
the other requested components remain listed in the spec.
