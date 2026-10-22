# Measured large-file transfer improvements

Date: 2026-10-22
Status: specification and Native implementation plan approved by the user on 2026-10-22; execution in progress.
Baseline: released v0.1.5, `75476b3c65a0c6719c5ac65c52490c5f9bef308d`.

## Intent and agreed outcome

The user wants large video copies and moves to finish sooner. Their priority is
the complete operation, including transfer and verification, rather than only the
wait after the transfer counter reaches 100%. This is the next component listed
in the [current-take specification](2026-10-20-current-take-discard-design.md)
after transient results and accessible rename updates.

The approved approach is to measure the existing stages first, choose an
improvement by its effect on total elapsed time, and give priority to overlapping
independent verification. Reading the actual desktop file and protecting the
phone original remain requirements. No speed percentage or measured result has
been established yet.

This specification covers the measurement contract and one focused optimization
of the existing copy/move pipeline. It does not authorize implementation until
the written specification and subsequent implementation plan have been reviewed.
The user's existing Native execution preference carries forward to that plan.

## Existing pipeline and chosen boundary

`src-tauri/src/files.rs` currently processes one selected file at a time on a
bounded file queue, independently of capture dispatch. Copy and Move:

1. Validate the selected device generation, full source identity and destination.
2. Stream the helper's binary `read` output into a unique `.partial` file in the
   destination directory, with bounded buffers and exact byte-count checks.
3. Synchronize that file with `sync_all`, check its stored length and stat the
   source again.
4. Run the phone helper's full SHA-256 operation, then read and hash the stored
   desktop file. These two full reads currently run sequentially.
5. Compare digests, check cancellation and publish with `persist_noclobber`.
6. For Move, recheck cancellation and source identity, request deletion, then
   perform the existing best-effort media-index notification.

The Android helper holds the same exclusive root lock for remote `read` and
remote `hash`. Starting those two commands together would not establish useful
remote concurrency. The selected first candidate instead overlaps the phone's
post-transfer hash with the desktop's post-sync readback hash. Both must finish
successfully before publication. The helper command protocol and its locking,
journaling and guarded-deletion algorithms remain unchanged.

Three approaches were considered:

| Approach                                                       | Benefit and limit                                                                                                                          | Decision                                                                |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------- |
| Measure first, then overlap the two post-write hashes          | Can remove the shorter independent hash from the verification critical path; preserves the current transport and real destination readback | First candidate, subject to total-time evidence                         |
| Return an operation-bound digest in a newly framed read stream | Could avoid another phone read, but requires a new framing/acknowledgement design and broader helper/client changes                        | Separate design amendment if measurements justify it                    |
| Tune streaming chunks or buffering                             | May help when data transfer dominates, but an assumed larger buffer is not evidence of improvement                                         | Only a narrow measured adjustment within the current streaming contract |

Do not combine speculative tuning with the concurrency candidate before measuring
them independently. If the existing transport is the limiting factor and neither
in-scope candidate clears the acceptance gate, report that result and present the
larger transport design for review. Do not weaken verification to obtain a gain.

## Measurement contract

Measure both Copy and Move for files of 500,000,000 and 1,000,000,000 bytes. These
are decimal MB/GB; record exact bytes and label any throughput unit explicitly.
Use finalized, valid, task-owned video fixtures with representative non-sparse,
nontrivially-compressible contents. The baseline and candidate must receive the
same bytes. Small clips and held fake operations establish correctness, not
large-file throughput.

The primary measure is user-visible elapsed time: one host-monotonic interval
from issuing the native Copy/Move action to observing the matching settled result
in that window. Also record backend request acceptance, queue entry, worker start
and authoritative terminal outcome. Report queue/startup overhead and the delay
between backend completion and UI observation separately. The ten-second success
notice lifetime is not operation time. Do not stop the timer at transferred 100%,
at a digest becoming available, or before Move's actual terminal outcome.

Collect monotonic durations for:

- Device/helper preparation and source/destination checks before streaming.
- Binary streaming through the final destination write and child-process outcome.
- `sync_all`, stored-size validation and post-transfer source validation.
- Phone SHA-256, desktop stored-file SHA-256, and the enclosing verification
  interval. Concurrent spans overlap; adding them is not total elapsed time.
- No-overwrite publication; Move's fresh source check and deletion acknowledgement;
  media-index work; terminal result delivery and presentation.

Use narrow benchmark/test diagnostics with operation correlation and relative
times. Do not add a general telemetry service, new public IPC, runtime progress
schema or production log of private filenames. Diagnostics must not hold capture
locks, write per-chunk logs or change transfer buffering. Measure the timing
harness's observation interval/overhead and use equivalent instrumentation for
the instrumented baseline and candidate. Compare the instrumented baseline with
the released build to detect instrumentation changing the result; retain that
distinction in the evidence.

Record source revisions, desktop/helper binary hashes, build profile, tool
versions, connection type, phone/host storage, power/thermal conditions, available
space and cache/order conditions. Raw serials, addresses, filenames and device
manifests remain in an ignored private trial directory. The public report contains
sanitized conditions, durations, byte counts and aggregate results.

### Repeatability and performance decision

Use the same release build configuration, phone, ADB transport, network and local
destination volume for each comparison. Separate preparation and one untimed
warm-up per build/size/kind from measured runs. Alternate baseline/candidate order
across three matched pairs for each size and operation kind. Copy reuses unchanged owned
sources; each Move uses a fresh owned duplicate with the same payload. State that
OS/phone caches are uncontrolled when applicable. Report first-use helper startup
and initial-copy observations separately; warm-run gains do not prove cold-run
gains. Do not require root, flush
phone caches, change antivirus exclusions or switch to USB merely to improve a
wireless result.

For each size/kind group, publish every measured total, the median, full range,
paired differences and stage breakdown. A performance improvement is accepted
only when:

1. The candidate is faster in every matched pair for both total backend time and
   user-visible elapsed time.
2. For each of those two measures, baseline median minus candidate median exceeds
   the larger of the baseline's and candidate's within-group ranges, and exceeds
   the measured timing/observation uncertainty. This is a practical evidence gate,
   not a claim of statistical significance or a universal speed guarantee.
3. Both file sizes and both operation kinds pass, and all preservation,
   cancellation, capture responsiveness and release checks remain satisfied.

If variation prevents a decision, document its cause and collect at most two
additional matched pairs for the affected group under the same declared method.
Retain the earlier results; do not discard inconvenient runs or keep testing
until a favorable sample appears. An interrupted/environmentally invalid run is
recorded with its reason and is not silently treated as a fast success.

If the gate still fails, the output is the measured bottleneck and a scoped next
recommendation. Do not merge a performance change as successful merely because
its hash stage improved. Any streaming adjustment is judged by this same gate.

## Verification coordinator and resource ownership

This section describes the conditional paired-hash candidate. If the baseline
gate selects the approved narrow write-buffering fallback instead, retain the
existing serial verification and its checks; apply the reviewed exact-byte,
flush/error and cancellation barriers in the
[buffering amendment](../plans/2026-10-22-transfer-buffering-amendment.md).
The user approved that amendment as Native on 2026-10-22. It does not authorize
combining candidates or changing the helper/transport protocol.

Keep streaming, durable write and stored-size validation before verification.
Preserve the post-transfer source stat. Start one phone-hash task and one
desktop-readback task for that file; bound the concurrency to those two tasks and
the existing bounded ADB readers. Do not parallelize files in the batch or create
an unbounded worker pool.

Bind both tasks to the same job ID, device lease/generation, complete source
identity and exact temporary file. The local reader must read the stored file
from its beginning after synchronization, using an owned handle for that same
file or an equivalently checked identity. A digest computed from incoming network
buffers does not replace destination readback. Use bounded chunks and checked
byte counts; memory must not scale with file size. Require the expected complete
SHA-256 response, successful remote exit status, a complete local read, and equal
digests. Keep shell-v2 remote status for the helper hash command.

The coordinator owns both task lifetimes, results and any verification-local
abort signal. It joins both tasks before allowing publication, removing the
partial, advancing to another file or emitting a settled outcome. A peer failure
cancels outstanding verification work without converting that failure into a
user cancellation or setting a job-wide cancellation flag that skips independent
batch entries. Report the causal error; a secondary peer-abort error cannot
replace it.

Both tasks observe job cancellation, device-generation replacement and peer abort.
The desktop reader checks between bounded reads instead of completing a full
gigabyte after cancellation. Stop and reap the outstanding ADB child through the
existing cancellation path. Use one verification deadline, no longer than the
existing 600-second remote-hash limit; parallel execution must not restart or
double that budget. The coordinator retains ownership while any OS read/sync
call is still returning. It cannot promise to interrupt an uninterruptible OS
operation, detach a live reader or claim cleanup has completed early.

Keep helper-deployment hashing separate from this job-scoped cancellable reader:
the existing `hash_path` also verifies the bundled helper. Do not accidentally
introduce job cancellation or a changed digest format into deployment checks.

## Publication, mutation and cancellation contract

Preserve the full source identity (device, inode, size, modification and change
times) as opaque data without JavaScript number conversion. The helper continues
to validate identity before and after remote reads/hashing, reject unsafe paths
and unfinalized files, and enforce its existing locks/recovery rules.

Both successful hash results and a fresh job/lease cancellation check are required
before no-overwrite publication. A destination created during verification still
wins the collision: retain it, do not replace it, and retain the phone original.
Read/write/sync/size/hash/source-identity errors cannot publish an unverified file
or authorize deletion.

Move may request deletion only after verified publication, the existing fresh
source identity check and another job/lease check. Cancellation, disconnect or
stale identity observed before deletion is dispatched prevents that request and
retains the phone original. Once a deletion request is in flight, cancellation
alone cannot establish whether its destructive commit happened. If a verified
destination was already published, do not erase it to simulate rollback.
After deletion actually commits, a later cancellation, lost acknowledgement or
indexing warning cannot imply the original was restored. Keep committed outcomes,
retained originals and uncertain acknowledgements truthful under the existing
result model. There is no automatic mutation retry.

Ordinary capture, current-take discard and standalone Delete do not acquire this
copy/move verifier. Full video decoding and preservation backups belong to the
physical test harness; they must not become extra production deletion steps.
Capture Start/Stop/Pause/Resume continues through its independent dispatch path
while transfer, sync, either hash, cancellation or media indexing is held.

## Interface and accessibility

Keep existing native progress semantics: copying reports transferred bytes;
verification remains an explicitly named indeterminate phase; 100% transferred
does not mean the operation has settled. No fabricated overall percentage,
automatic focus movement, TTS, live region or alert/status announcement is added.

Preserve English/Arabic wording and RTL, cancellation controls, selected row
identity, confirmed rename names, shared ten-second success deadlines, persistent
errors/warnings and focus-protected result/history behavior. Instrumentation and
peer task completion must not emit duplicate terminal results or duplicate audio.
Computed accessible names and stable focus remain automated requirements. Audible
NVDA/virtual-cursor assessment remains deferred under the user's instruction;
do not claim automated checks establish the listening experience.

## Verification and physical-test preservation

Use observed failing regressions for the behavior being changed. Tests must prove
actual independence and barriers rather than merely inspect implementation names:

- Hold each hash independently and demonstrate both tasks start before either is
  released. The existing sequential implementation must fail this assertion.
- Corrupt the stored partial after the input bytes were accepted; destination
  readback must detect it. Cover truncated/extra bytes, sync/read errors, malformed
  or mismatching remote digests, remote nonzero exit and changed source identity.
- Create a destination during held verification; it and the source survive.
  Neither early hash completion nor a late old-generation result can publish or
  delete. A successful Move deletes only the exact verified source.
- Cancel or replace/disconnect the lease with either task held; both tasks stop
  and join, the partial is cleaned only after readers finish, and no deletion is
  dispatched from cancelled verification. Cover peer failure, deadline, worker panic and a batch
  continuing after an independent file failure. Preserve the existing held-hash
  cancellation bound and capture-dispatch assertions without relaxing timeouts.
- Exercise accurate post-publication outcomes and unchanged shared result expiry,
  naming and focus behavior. Preserve exactly four mandatory actual release
  desktop cases, their titles, 90-second case limit and shared 15-second waits.
  Extend their existing flows where necessary; do not substitute direct mutations
  for native actions or add product-only test controls.

Run applicable pure Rust/helper/UI/format/Clippy/build checks and the full hosted
validation gates on the final tree. The external fake-ADB executable remains
quarantined locally: do not restore, rebuild, rename, exclude or run it locally,
including through WSL. Full fixture/workspace integration and actual release
desktop acceptance run on GitHub. Local library/helper checks remain available.
Correctness fixtures and hosted CI timing are not phone throughput evidence.

The physical benchmark is a separate scheduled test stage, not something started
by writing or approving this specification. Within the user's existing owned-clip
test authorization, establish phone availability and a fresh private trial with
adequate free space on both devices before allocating large fixtures. Never reuse
earlier private journals/backups as scratch space or select personal media.

Journal every newly created fixture. Before any rename/move/delete, preserve a
local backup, verify its size/SHA-256 and fully decode the video. These checks and
fixture generation are outside the measured operation and are equivalent across
builds. Preserve an unrelated-file identity/hash baseline and all prior test
backups. Retain enough space for the originals, backups, temporary and published
copies; stop preparation if capacity is insufficient. Clean only manifest-owned
temporary artifacts after readers/processes have finished, retaining evidence
and verified backups.

Use the current observed app/settings/accessibility state as the trial baseline;
do not restore obsolete settings from an earlier version. A difference alone is
not evidence that the user changed a setting. Keep the installed desktop app
closed except when the trial requires it. Preserve wireless pairing, the ADB
server and its startup configuration; do not disconnect/unpair, kill the server
or change connection/security settings for the benchmark. Leave the phone
connected afterward.

User amendment on 2026-10-22: after each phone session, including failures, leave
Open Camera, return Home and put the screen to sleep. Do not leave the camera
foreground between test stages; preserve pairing and the ADB server.

## Scope and review handoff

Installer language, bundled ADB and pairing UI, wireless recovery features,
automatic trimming, unrelated dependency/translation/automation audits and a new
release are separate components. Compatible dependency updates remain authorized
by the user, but do not add unrelated upgrades to a transfer measurement change.
The larger framed-stream protocol needs an explicit reviewed design amendment.

Review this specification for complete timing boundaries, a falsifiable
performance gate, bounded task ownership, cancellation/commit races and unchanged
preservation/accessibility guarantees. Commit it and present the written file to
the user. Written-spec approval permits `superpowers:writing-plans`; that plan
must then be reviewed before Native implementation begins. The present document
records a design, not completed benchmarks or an established speed improvement.
