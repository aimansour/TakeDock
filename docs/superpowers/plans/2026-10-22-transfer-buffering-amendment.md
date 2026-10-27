# Bounded Transfer Write Buffering Implementation Plan Amendment

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this amendment task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Test whether coalescing small desktop writes reduces complete large-file Copy/Move time without changing transport or verification safeguards.

**Architecture:** After the original plan's baseline gate, replace its paired-hash candidate with one fixed 256 KiB desktop write buffer. Preserve the existing 256 KiB input buffer, one stream worker, serial stored-file verification, helper protocol, job queue and capture dispatch. Accept the candidate only through the original complete real-phone total-time gate.

**Tech Stack:** Rust standard `Read`, `Write` and `BufWriter`; existing Rust/PowerShell/native-UI test tools. No dependency upgrade or additional runtime service.

**Spec:** [Approved transfer specification](../specs/2026-10-22-large-file-transfer-design.md), particularly the measured narrow streaming/buffering alternative.

**Parent plan:** [Large-file transfer plan](2026-10-22-large-file-transfer.md). Tasks 1–2 and their evidence remain; the tasks below replace only Tasks 3–4 if the completed baseline still rules out paired verification. Parent Task 5 and its complete acceptance gate remain binding. Preserve the parent's Native ledger and frozen baseline artifacts.

**Status:** Approved explicitly by the user on 2026-10-22: «أعتمد التعديل؛ نفّذ Native». Baseline collection continues independently. Execute after the parent baseline decision; no further execution-method or plan confirmation is needed.

## Why this amendment exists

The measured 500 MB Copy pair spends 107738.5 ms streaming and 3488.6 ms verifying
(phone hash 981.2 ms; desktop hash 2507.3 ms). The measured 500 MB Move spends
146001.5 ms streaming and 3515.1 ms verifying (phone hash 969.4 ms; desktop
2545.7 ms). The calibration's maximum absolute paired UI difference is already
20668.4 ms. Perfectly overlapping these measured hashes could remove at most
the shorter hash duration, under one second at 500 MB, before coordinator cost.
The 1 GB diagnostic Copy warm-up likewise spends 353573.2 ms streaming versus
8204.6 ms verifying (phone 3148.3 ms; desktop 5056.2 ms). These measurements do
not establish any buffering gain; they identify which stage merits a separate
bounded experiment. Finish the baseline matrix and report all samples first.

The hypothesis is that coalescing fragmented pipe reads into fewer file writes
may reduce streaming overhead. It can fail. Do not combine it with concurrent
hashing, chunk-size tuning, compression or a new transport/framing protocol.

## Global Constraints

All parent/spec global constraints remain binding, including:

- "A digest computed from incoming network buffers does not replace destination readback."
- "There is no automatic mutation retry."
- Keep exact opaque source identities, exact byte counts, `sync_all`, stored-size and post-transfer identity checks, full phone and actual stored-file SHA-256, no-overwrite publication and fresh Move identity/delete acknowledgement.
- Buffer memory is bounded: one 256 KiB input buffer plus one 256 KiB write buffer per active stream; files remain serial. The stream stage ends only after the explicit final flush succeeds and its worker joins.
- Discard any unflushed tail on a detected read/write/size error; do not let `BufWriter` drop retry a failed flush. Existing cancellation kills the owned child and joins the stream worker before partial cleanup.
- Preserve exactly four mandatory actual release desktop cases, their titles, 90-second case limit and shared 15-second waits. No UI/locale/result-lifetime changes or new product test controls.
- Never build/run/restore the quarantined fake ADB locally, including through WSL. Fixture integration and native release gates run on GitHub.
- Use only the parent's owned physical trial, verified backups and immutable builds. Sleep the phone outside each session, preserve pairing/server, current settings and unrelated media. Full decode/backup checks stay in the harness.
- A broader transport/framing change requires its own reviewed design amendment. No merge or release publication is authorized by this amendment.

## Review Focus

1. EOF after a buffered tail: every exact byte must reach the file before stream completion; pinned by Task 3's tail/flush test.
2. A short/extra source or reader failure while bytes remain buffered: return failure without implicit drop-time writes; pinned by Task 3's failure matrix.
3. A short/zero/erroring underlying write, including a final flush failure: never report a successful stream; pinned by Task 3's injected-writer tests.
4. Cancellation or replacement while a tail is buffered: retain the source and join before deleting the partial; pinned by Task 4's existing cancellation/generation regressions and its short-tail cancellation extension.
5. Stored-file corruption or a destination collision after streaming: detect through actual readback/no-overwrite publication; pinned by Task 4's held-hash integration cases and native case 3.

## Task 3: Exact bounded stream copier

**Files:** Create `src-tauri/src/files/stream_copy.rs`; its tests remain in that module. Declare the private module in `files.rs` so its tests compile; integrating it into the active worker belongs to Task 4.

**Interfaces:** Produce private `copy_exact<R: Read, W: Write, P: FnMut(u64)>(reader: &mut R, writer: &mut W, expected_size: u64, progress: P) -> Result<(), String>`. Successful return means exact EOF, explicit successful final flush and no retained buffered bytes. Counts use checked addition; truncation, excess and overflow retain `copy_size_mismatch`. I/O errors retain the existing string mapping. No new verification/cancellation DTO or public API.

- [ ] **Step 1: Write failing tests.** A reader returning 4096-byte fragments supplies 1 MiB + 17 deterministic nonuniform bytes. A counted writer must receive identical bytes with at most five underlying data writes, one explicit successful flush and a final progress count equal to the exact size. Include a tail shorter than 256 KiB and empty exact input. Add reader failure, short/extra input, short writes, zero write, write error and flush-error cases. Detected input errors before a buffer fills must cause zero underlying tail writes; a failed write must not receive a drop-time retry. Assert all failures remain failures.
- [ ] **Step 2: Observe RED.** Run `cargo test --locked -p takedock --lib files::stream_copy`. First establish the baseline direct-write implementation under the declared helper interface; the fragmented-write-count assertion must fail before buffering is introduced. Save the failure output. Never count a merely missing-module compile error as the coalescing regression.
- [ ] **Step 3: Implement the fixed buffer.** Use `BufWriter::with_capacity(256 * 1024, writer)` around the existing bounded read loop. Check exact size before the final explicit flush. Consume the buffer through `into_parts` on exit so an error cannot trigger implicit drop flushing/retry. Progress reports accepted bytes; it never becomes a terminal result or a verifying-phase signal.
- [ ] **Step 4: Verify GREEN.** The named tests and `cargo test --locked -p takedock --lib` pass; `cargo fmt --all --check` and the parent diagnostic library Clippy command pass. Save counts/results to the existing ledger.
- [ ] **Step 5: Commit.** Commit the independently tested private copier as `perf: coalesce exact transfer writes in a bounded buffer`.

## Task 4: Integrate buffering and prove existing commit barriers

**Files:** `src-tauri/src/files.rs`, `tests/fixtures/adb/{tests/files.rs,src/main.rs}`, and the existing native case 3 file only where the parent plan already requires its held-hash collision flow. The fixture-only tail gate lives in its existing fake helper implementation; no real helper or dependency changes.

**Interfaces:** Consume Task 3's copier inside the existing stream worker. Its progress closure updates the existing atomic byte count and throttled event (100 ms). Keep controller cancellation/inactivity handling, child exit/stderr checks, joins, stream trace boundaries and all subsequent verification/publication/deletion code in the same order. The copier flush must finish before the caller's `sync_all` and stored-file hash.

- [ ] **Step 1: Add failing integrated safety cases.** Extend the fixture tests with `stored_partial_corruption_during_hash_retains_source` and `destination_created_during_hash_is_retained`: hold remote hashing, change only the fixture-owned desktop partial or create the final destination sentinel, release hashing and assert failure, source preservation, sentinel preservation and partial cleanup. Add `cancellation_discards_a_held_buffered_tail`: the fixture-only `hold-read-tail` mode sends 64 KiB through its existing slow writer, creates `read-prefix-held` and blocks further output while that flag exists; use a 128 KiB payload. Wait for that marker and positive byte progress, assert the partial is still length zero, cancel and require the existing cancelled/source-preserved/no-delete/partial-cleaned outcome. The zero-length assertion must fail on direct writes before integration. Keep existing batch, remote nonzero/hash, replacement and capture-dispatch assertions.
- [ ] **Step 2: Run hosted RED where necessary.** Commit/push the added regression cases on the draft feature PR and run the fixture integration on GitHub; do not build it locally. For safety assertions already satisfied by the serial implementation, record the pre-change pass instead of inventing a failure. The meaningful behavioral RED for this candidate is Task 3's write-count regression; do not weaken safety tests to manufacture RED.
- [ ] **Step 3: Integrate just the copier.** Replace the worker's direct read/write loop with `copy_exact`, preserving its event/progress closure. Remove no source-size/identity/digest/child-status check. Add no hashing from input buffers, new worker, delay, protocol, runtime baseline switch or translated message.
- [ ] **Step 4: Preserve the native flows.** Complete the parent's held-hash destination-race extension in native case 3 through exact native selection and Copy action. Keep all four titles, focus/result/name checks, capture controls and time limits unchanged.
- [ ] **Step 5: Verify the integrated tree.** Run the parent's safe local library/helper/diagnostic/format/Clippy/UI/build commands and all 24 current benchmark pure cases (plus any added cases). Require all hosted Rust/UI/Observer/desktop/quality jobs and the diagnostic fixture coverage on this tree. Preserve the held-hash two-second cancellation assertion. No local fake executable or gate relaxation.
- [ ] **Step 6: Commit and freeze.** Commit as `perf: buffer desktop transfer writes before durable verification`. Freeze default and diagnostic executables/resources and hashes with the same release configuration and verified public helpers as the preserved serial baseline. Do not overwrite the baseline or silently change helper bytes.

## Parent Task 5: unchanged decision gate

Before the next physical stage, add and test a harness-only preparation option
in `scripts/transfer-benchmark/Device.psm1` and its tests, consumed by the runner:
create a fresh owned Move duplicate on the phone from the preserved owned master
using the unchanged identity-bound helper `read`, with an exclusively created
shell redirection (`set -C`) to the new GUID name. Validate the immutable local
backup/decode receipt and master identity/hash first, journal the planned creation,
then verify the new duplicate's fresh identity and full SHA against that backup
before selection. Masters still use real ADB push. Reject replacement identities,
wrong backup bytes, unowned/existing targets and failed copy acknowledgements;
never retry automatically or change the master. Adapter tests must observe RED
before implementation. This saves repeated wireless uploads outside measured
operations; disclose the preparation method and use it equally for both builds
in each comparison group. It does not change the production transfer protocol.

After the reviewed candidate is correct and frozen, run the same separate native
phone comparison: four size/kind groups, one warm-up per build/group, three
alternating matched pairs, both backend and native UI totals improving in every
pair, median gain greater than within-build ranges and measured uncertainty.
Retain all earlier valid samples; at most two extra pairs for ambiguous positive
groups. An ineffective candidate is rejected; do not advertise it as faster.
Apply a predeclared futility stop only to candidate comparison: after any valid
matched pair fails strict improvement in either total metric, preserve it, write
an explicitly rejected partial comparison and skip remaining candidate runs.
Retaining every pair makes later acceptance mathematically impossible after
that failure. Unrun cells are labelled unrun, never passed. Full four-group,
three-pair evidence is still mandatory for acceptance; baseline calibration
still completes all its cells. Test this stop with positive, negative, equal,
incomplete/mismatched and warm-up inputs before wiring the runner.
Keep the single fresh whole-branch review at the end under the preserved Native
method. Report actual sample counts, all totals/limits, preservation and every
ledger ruling. Merge and release still await a later user instruction.

## Self-review and handoff

Coverage: parent Tasks 1–2 retain calibration/instrumentation; amendment Task 3
owns bounded byte/flush semantics; Task 4 owns integration, cancellation and
commit barriers; parent Task 5 owns physical acceptance, preservation and the
fresh final review. The generic copier signature and progress type match their
consumer. All five review-focus cases have named owners/tests. No gain is
prewritten. Written review is complete; preserve Native and finish the already
authorized baseline work before implementing the amended candidate.
