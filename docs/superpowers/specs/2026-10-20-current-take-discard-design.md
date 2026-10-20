# Discard the current TakeDock recording

Date: 2026-10-20. Status: approved by the user, including the file-association
limitation, on 2026-10-20. Checkbox placement was selected first; the user then
approved the written design. The implementation plan requires its own review.

## Intended outcome and context

The user is continuing the broader TakeDock work after the completed
[speech-readiness component](2026-10-15-recording-readiness-design.md).
The next requested component is checkbox-confirmed stopping and deletion of
the current take, directly from the Recording panel. It should save a trip to
the Videos panel when rejecting a take, while preserving recording responsiveness,
keyboard access, and the existing file-integrity protections.

The user explicitly selected this interaction on 2026-10-20: a checkbox next to
Stop changes that button to Stop and delete when checked. It does not start a
replacement recording. The existing project design remains authoritative for
accessibility, independent capture dispatch, device generations, and file safety.

## Approaches and recommendation

1. **Track a TakeDock-started take and use the existing identity-checked deletion
   path (recommended).** Associate a new recording with a complete pre-Start
   directory baseline and subsequent file evidence. Require a confirmed Stop,
   a finalized file, and unchanged identity before deletion. Document the
   association assumptions and preserve files when evidence is ambiguous.
2. **Select the newest video after Stop.** This cannot distinguish a take from
   older videos, unrelated additions, or a replacement file. Do not use it.
3. **Obtain an authoritative recording identifier from Open Camera.** This would
   provide stronger provenance, but the current Observer protocol has no file
   identifier. Supporting one would require a separately demonstrated upstream
   integration, rather than assuming that accessibility state exposes a path.

## User-visible behavior

While a supported current take is recording or paused, show a native unchecked
checkbox beside the existing primary recording button:

- English: **Delete this take**. Arabic: **حذف هذه المحاولة**.
- Unchecked primary button: **Stop** / **إيقاف**.
- Checked primary button: **Stop and delete** / **إيقاف وحذف**.

The checkbox is confirmation for this take only. Checking it does not send Stop,
cancel readiness, delete a file, or persist a preference. Unchecking restores
ordinary Stop. Activating the checked button submits one discard request and
immediately invalidates any pending speech-readiness cue.

Reset the checkbox when the take changes, its controls disappear, the Recording
panel is left, or the connection generation changes. A newly opened window starts
unchecked. Selecting the checkbox in one window does not check it in other windows;
an accepted discard operation is shared across all windows.

Tab, Space, and Enter use native checkbox/button semantics. No confirmation dialog,
live region, TTS, unsolicited announcement, or automatic focus change is added.
The primary button retains its DOM identity. If a disappearing checkbox held focus,
restore focus to the logical primary recording control, following the existing
focus-continuity pattern.

The existing Ctrl+Shift+R shortcut continues to mean ordinary Start/Stop and never
deletes a take. Short help text beside the checkbox explains that deletion requires
activating the Stop and delete button. This preserves the documented shortcut's
existing meaning; the new button remains fully keyboard operable.

Ordinary bilingual text shows the operation's progress: waiting to stop, confirming
Stop, waiting for the video to finish saving, deleting, and completed or retained
with a reason. Starting a new recording remains an explicit user action.

## Supported association and its limit

The current `Observation` contains recording state, sequence, session, and event
time, but no video path or recording identifier. A `Video` already carries an opaque
exact remote identity, including device, inode, size, modification and change times.
That identity protects a selected file from replacement; it does not by itself
prove which camera action created the file.

Support automatic discard only for a take started through TakeDock from a fresh,
confirmed idle state, on the same connection generation, with a complete directory
baseline established before Start dispatch. Track the Start command sequence with
a unique take token. An already recording phone, a missed baseline, an observation
gap, or an ambiguous file history cannot acquire discard eligibility retroactively.

Prepare and maintain the baseline on an independent file-observation worker while
idle. Capture dispatch never waits for a directory listing, a file job, or baseline
preparation. Start remains usable when the baseline is unavailable; that take has
ordinary Stop and an explanation that automatic deletion is unavailable.

Associate only one newly created supported video in the fixed
`/sdcard/DCIM/OpenCamera` directory, with continuity of its device/inode through
recording and finalization. Do not associate by filename order, modification date
alone, size, or a listing begun only after Start. Replacement of a pre-existing
entry is not a new eligible take. Multiple candidates, split recordings, directory
watch overflow or transport loss, and unexplained file activity invalidate automatic
association. Retain every candidate and direct the user to Videos for explicit
selection instead of guessing.

**Approved provenance limitation:** this filesystem correlation assumes
Open Camera is configured to save to the supported directory and is its only writer
during the managed take. Filesystem events alone cannot authenticate the creating
application. In particular, a sole unrelated video added while Open Camera saves
elsewhere could be indistinguishable from the intended take. Exact identity checks
do not eliminate this limitation. If that assumption is unacceptable, automatic
deletion needs an authoritative upstream identifier or a post-Stop confirmation
of a specific file; the one-checkbox interaction cannot promise stronger provenance.

Do not present directory correlation as an unconditional ownership guarantee.
The final UI help and user documentation must explain the supported configuration.

## Backend lifecycle and ordering

Keep the take/discard coordinator in Rust, shared by the single Engine and all
windows. Use a pure state machine for association and discard decisions, with
external effects performed by separate workers. Wire snapshots carry generation,
revision, take token, eligibility/reason, discard operation ID, and phase.
Frontend requests identify the take and generation; they do not choose a pathname.

The lifecycle is tracking, discard requested, waiting for a safe Stop opportunity,
Stop sent, waiting for finalization, deleting, then completed or retained. Duplicate
requests for the same take reuse the accepted operation and never enqueue a second
toggle or delete. A new token is never substituted into an old operation.

The [physical acceptance report](../../research/2026-10-19-recording-readiness-acceptance.md)
records Open Camera 1.56.2 ignoring Stop within its first 500 ms, and successful
explicit actions 600 ms after fresh confirmation. For this discard operation,
invalidate readiness immediately, then schedule its single initial Stop no earlier
than 600 ms after fresh confirmation of that Start. Waiting is asynchronous and
does not hold capture/session locks. A take already past that boundary, including
a paused take, can dispatch its Stop immediately when otherwise eligible.

This is a deferred first Stop belonging to an explicit discard request, not a retry
of an ignored toggle. Ordinary Stop retains its existing immediate dispatch behavior.
Do not infer Stop success from predicted idle, shell acknowledgement, or time expiry.
Require fresh, causally matching confirmation of the discard operation's own Stop.
If Start/Stop cannot be confirmed within the existing verification deadline, retain
the file and expose the existing uncertainty with a discard-specific explanation.

Another ordinary capture intent before the discard Stop is sent cancels that pending
Stop. A later capture intent, a new Start, connection change, observer failure,
foreground/video-mode loss, or shutdown cancels every uncommitted deletion effect.
No recovery observation, reconnect, application restart, or new window resumes it.
Once a delete has committed it cannot be undone; report that outcome accurately.

Before deletion, obtain a fresh finalized identity and require it to be unchanged
across two checks at least 250 ms apart. Bound waiting for finalization to 10 seconds
after confirmed Stop. These checks run independently of capture dispatch. Revalidate
generation, take token, cancellation, and exact identity immediately before the
existing helper's journaled delete claim. Perform effects outside session locks and
do not hold a capture lock during remote I/O. Record a delete's commit boundary so
a cancellation cannot falsely claim that an already committed deletion was prevented.

Use the existing no-symlink root validation, identity-checked journal/recovery path,
and media-index refresh. Do not add raw `rm`, arbitrary frontend filesystem access,
or a fallback that deletes a similarly named file. Serialize discard deletion with
existing file mutations. A changed source, unavailable file worker, failed helper,
disconnect, or finalization timeout retains the original and reports the reason.

Completed deletion refreshes Videos and removes the stale selection by identity.
Use the existing process-owned optional success/failure feedback, once per operation,
regardless of window count. Do not duplicate the discard Stop's success tone with
the deletion result, replay readiness, or produce repeated failure sounds while waiting.

## Interfaces and implementation boundaries

- Add a focused take/discard state-machine module rather than growing readiness
  policy into file management. Keep readiness invalidation at its existing boundary.
- Extend session/runtime integration with generation- and take-bound discard IPC
  and shared snapshots; ordinary recording IPC retains its current contract.
- Add an independent helper-backed baseline/file-observation transport; do not
  queue baseline preparation behind long copy/hash jobs or block Start on it.
- Reuse FileService and the Android helper for the final serialized delete, adding
  a cancellation/precommit guard where necessary.
- Extend frontend types, IPC/controller state, RecordingPanel, and both translation
  dictionaries. Frontend code renders backend phases and owns only the local checkbox.
- Extend the external ADB fixture, actual WebView2 desktop suite, and bilingual
  setup/keyboard guidance. No product test server or special testing IPC is added.

No new dependency, Observer permission, camera-app modification, setting migration,
or release publication is assumed by this design. Select any necessary dependency
only during planning with explicit compatibility evidence.

## Verification and acceptance

1. Pure deterministic tests cover take tokens, baseline ordering, eligible versus
   externally started recording, single-file association, ambiguity, split files,
   replacements, missed events, and session changes. Include negative tests proving
   an ambiguous candidate does not reach deletion; document that writer provenance
   is an environmental assumption rather than a property the fixture can prove.
2. Clock tests cover immediate readiness cancellation, the 600 ms initial-Stop
   boundary, missing Start confirmation, one Stop only, stale Stop confirmation,
   verification expiry, finalization stability/timeout, and cancellation before
   each commit boundary. Never use wall-clock sleeps in pure state-machine tests.
3. Real-process fixture tests verify that delete cannot precede confirmed Stop and
   finalization, exact identities survive cancellation/failure, and new sessions,
   new takes, duplicate window requests, and queued ordinary commands cannot
   redirect a discard operation. Hold transfers and prove Start/Stop dispatch
   remains independent; hold the delete before commit and test cancellation.
4. Component/controller tests cover unchecked defaults, checkbox/button names in
   both languages and RTL, keyboard behavior, ordinary Ctrl+Shift+R, per-take resets,
   window-local confirmation, shared backend progress, and focus continuity.
   Continue checking absence of live regions and inapplicable disabled controls.
5. Actual release EXE/WebView2 tests exercise the complete checkbox-to-IPC workflow,
   duplicate windows, retained-file errors, and real fixture destination/root files.
   Fixture success establishes integration, not actual Open Camera ownership evidence.
6. Real-phone acceptance uses dedicated, explicitly owned test recordings only.
   Establish their pre-Start manifests, preserve verified local copies before any
   test deletion, and exercise normal, paused, very early discard, uncertainty,
   cancellation, and unrelated fixtures. Do not delete personal recordings or
   infer test ownership from newest-file order. Report unperformed physical cases
   and actual platform coverage honestly.

## Subsequent requested components

Keep these visible without implementing them in this component: transient file
results and accessible rename updates; measured 500 MB/1 GB transfer improvements;
installer-language handoff; compatible bundled ADB and wireless pairing/connection;
and the remaining dependency, translation, and repository automation audit.
The repository already records desktop dependency and release-publication work;
audit that evidence before treating those broader items as entirely unfinished.

Automatic trimming of leading footage remains a separate preservation design.
