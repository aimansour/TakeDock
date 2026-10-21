# Transient file results and accessible rename updates

Date: 2026-10-20
Status: written specification approved by the user on 2026-10-21; implementation plan review is next.
Base: merged `main` at `4dc44e72498c1f455b4e43b40f8088a040ceb45d`.

## Intent and agreed behavior

Continue the remaining TakeDock work after recording readiness and current-take
discard. This component makes completed file operations stop cluttering the main
interface and makes a confirmed rename reach the video's native accessible name.
The user needs to continue recording immediately, read failures deliberately, and
find the current filename without unsolicited speech or focus movement.

The user selected: successful results disappear after ten seconds; failures and
unknown outcomes remain until explicitly hidden. The user then approved a shared
result lifetime across windows, a collapsed operation history, protection for a
focused result, and updated screen-reader names while preserving row identity.
This conversational approval permits writing this specification, not product code.

Success means:

- Queued/running file work stays visible and cancellable. Completed success leaves
  the main result/activity presentation after 10,000 ms from its first authoritative
  terminal result. An unrelated operation does not postpone that deadline.
- Failed, cancelled/incomplete, retained and outcome-unknown results do not expire
  automatically. A later success does not replace or dismiss an unresolved result.
- Details remain available in an ordinary, initially collapsed operation history.
- A confirmed rename updates the same video's name, native checkbox label and
  actionable metadata in every window, without waiting behind another file job.
- Recording dispatch remains independent of file jobs, result expiry/dismissal,
  history rendering, media indexing and directory refresh.

## Existing behavior and component boundary

`session.svelte.ts` currently stores one untyped result string alongside up to
twenty jobs. `ActivityPanel` falls back to the latest terminal job indefinitely;
`App.svelte` renders terminal jobs as an always-open section. Backend `Events`
provides shared bootstrap jobs and feedback deduplication, but no shared result
lifetime or acknowledgement state. A new frontend-only timer would consequently
be insufficient for expired bootstrap results, multiple windows and late events.

`VideosPanel` already keys rows/selections by exact device/inode strings, renders
the current filename in its native checkbox label, and has a DOM-level rename
test. Preserve these useful guarantees. The existing file result records only the
source name, and the UI depends on a serialized listing after completion. That
does not establish a full IPC/native-accessibility rename update while a subsequent
file job holds that queue. Native and process tests must reproduce that gap before
changing behavior; do not claim the existing DOM test proves actual NVDA behavior.

This component owns presentation/result lifecycle, rename outcome data, narrow
result IPC, frontend reconciliation and bilingual guidance. It does not optimize
transfer/hash algorithms, change destructive-operation permits, trim media, change
installer language, bundle ADB, implement wireless pairing, upgrade dependencies,
or publish a release. Those remain later focused components. The existing wireless
phone connection must remain active at the user's request.
Apply the new lifetime policy only to file-operation results; capture/settings
messages keep their existing ownership and behavior.

## Shared result model

Introduce a focused Rust model for file-result presentation and history, separate
from capture prediction, file execution and audio ownership. Drive its time from
one process-monotonic clock and inject elapsed milliseconds into its pure methods
for deterministic tests. No wall-clock changes may extend a success lifetime.

Use stable logical keys containing the connection generation, owner kind
(`job` or `discard`) and operation ID.
Ordinary jobs use their job ID. A current-take discard uses its discard operation
ID: its generic delete-job events supply progress/details but never create a second
terminal notice or override the authoritative discard outcome.

The first accepted terminal outcome establishes a history entry and, for a clean
success, its expiry deadline. Duplicate terminal/progress events, reconnect views,
refreshes and new windows cannot rearm that deadline or revive dismissed notices.
Older generations cannot mutate a newer generation's active work. Preserve settled
history and unresolved notices separately when a connection changes; they are
descriptions of previous operations, never authority to act on another phone.

A notice has a stable key, its own outcome revision, kind, outcome/code, relevant
names/counts and an optional success deadline. Classify completed-with-warning separately from a
clean success. A media-index or post-rename-metadata warning remains visible until
hidden while the underlying committed mutation remains accurately completed.
An ordinary completed event must not erase a warning already accepted for that
operation, even when the warning arrives before the terminal job event.
Do not describe an acknowledged rename/deletion as undone because indexing or
presentation metadata failed afterward.

Keep the latest twenty ordinary terminal history entries, plus unresolved
non-success/warning entries pinned until acknowledged. Acknowledged entries become
eligible for the ordinary history bound. Pending notices must not be silently
evicted by later successes. History/acknowledgements live for this application
process, not as a new disk database; application restart clears them. This is
presentation retention, not cancellation or retry of remote work.

## Synchronization, expiry and acknowledgement

Provide a read-only result snapshot and an idempotent acknowledge-result command.
Snapshots include a monotonic projection time, model revision, terminal history and
currently eligible notices. Result changes publish a small revision notification;
windows reconcile from a fresh snapshot, coalescing overlapping reads and rejecting
older responses. Existing capture IPC remains unchanged.

Bootstrap obtains the same projection, so an already expired or acknowledged
success does not reappear in a newly opened window. The server owns the original
deadline; the frontend schedules display expiry from the remaining interval using
its monotonic elapsed time, conservatively accounting for snapshot request time.
Within a window, another snapshot/event can only preserve or shorten that notice's
local remaining lifetime, never restart it. Delayed timer callbacks recheck the
deadline. Expiry is a local rendering decision and must not require an ADB command,
scan, file-worker turn or capture-lock acquisition.

Acknowledgement identifies an existing logical result key and its outcome revision,
not the snapshot's global model revision. It cannot cancel a job, select a path,
authorize a deletion or dismiss a different result
through a stale ID. It applies to all windows and subsequent bootstrap snapshots.
Acknowledging a result from an earlier connection is allowed because it only
changes presentation. Duplicate acknowledgements are harmless. A newer warning or
outcome revision must not be hidden by an acknowledgement of an older revision.

Audio remains process-owned by the existing feedback rules. Snapshot replay,
expiry, history expansion, acknowledgement and rename metadata reconciliation
produce no tones. Exactly one discard terminal result continues to own its sound.

## Interface and accessibility behavior

Keep active operation progress/cancellation in Activity. When no operation is
active and no eligible terminal notice remains, show the ordinary Ready state
rather than a permanent completed progress bar. Show unresolved results as
ordinary navigable text with an explicitly named native hide button per result.
An acknowledgement hides a message, not a phone file or an unfinished operation.

Use a native disclosure for operation history, initially closed in each window.
It retains per-file outcomes and the source/destination names of renames. Do not
open it automatically or move focus when a result arrives. Keep already displayed
history records stable while the disclosure is open, deferring eviction of those
records until it closes; incoming results must not remove the record being read.

A success whose result card contains DOM keyboard focus remains rendered in that
window after expiry until focus leaves. Apply the same local protection if another
window acknowledges a focused card. An explicit local hide action may return focus
once to a predictable adjacent/stable control if it removes the focused button.
It must not move focus in another window. A later completion/refresh never moves
focus back to a row, result or rename form.

Do not add TTS, live regions, alert/status roles, automatic announcements or
disabled inapplicable controls. Preserve native control semantics, English/Arabic
labels, RTL and isolated filename direction. DOM focus protection is not a claim
to observe NVDA's separate browse/virtual cursor; stable open history and actual
user validation cover that distinct behavior.

## Authoritative rename update

Extend the existing file outcome path with an operation-bound confirmed rename
update containing generation, job ID, exact source identity/name and a complete,
verified destination `Video`. Reuse the existing helper's read-only inspection to
obtain the destination after successful rename acknowledgement; verify the expected
destination name and the same device/inode. Do not reuse the old opaque identity
after a rename, infer an identity from the filename, or optimistically paint a
requested name before the remote operation is acknowledged.

Publish this data update before best-effort media indexing and before the worker
begins a subsequent file job. It is not an extra success notice or audio event.
Keep the helper's atomic no-overwrite rename, extension preservation and source
identity checks. No new file-helper CLI protocol or dependency is required.

If rename is acknowledged but the new metadata cannot be verified, retain the
committed outcome and expose a persistent refresh warning. Do not fabricate
destination metadata, report that the acknowledged mutation was rolled back, or
retry the mutation. A later ordinary fresh listing reconciles the actual files.
Until then, distinguish the acknowledged destination in history from list metadata
awaiting refresh, and preserve exact source-identity validation for any later job.
An unacknowledged rename failure cannot emit a confirmed rename data update.

Each window applies a confirmed update only to the matching generation and exact
source object represented in its current listing. Update name, opaque identity,
duration and readiness together. Preserve the row/checkbox DOM nodes and selected
device/inode key. Invalidate directory responses started before that update so a
delayed listing cannot restore an old name/identity. Ignore already-applied or
stale rename updates rather than overwriting a subsequent rename or replacement.

Show successful rename history as source -> destination, with the actual new
filename available to native accessibility. Failed rename history keeps its
requested/source context without pretending the destination was committed.
Closing/submitting the user-operated rename form may use its established immediate
focus return; the asynchronous completion cannot steal focus afterward.

## Verification and acceptance

1. Pure Rust result-model tests with an injected clock: before/at 10,000 ms,
   duplicate terminal events, progress after terminal, unchanged deadlines across
   projection, stale generations/revisions, pinned failures, history bounds,
   acknowledgement races, later warnings and discard/generic-job deduplication.
2. Svelte/controller tests with fake timers and deferred IPC: success expiry from
   all main presentation surfaces, failed/retained/unknown persistence, failure
   surviving a later success, delayed/bootstrap responses, no expired replay in a
   second window, stale timer not clearing a newer result, and timer cleanup on
   disposal. Check focus-protected removal and stable open history separately.
3. Rename process/controller tests: exact destination metadata after acknowledged
   rename, unchanged inode, no overwrite, postcommit inspection/index warning,
   cancellation/disconnection around acknowledgement, reversed directory/update
   delivery, two successive renames and replacement/generation mismatch. Prove
   current capture dispatch remains independent while a file job is held.
4. Preserve exactly four mandatory actual release desktop test titles/case count.
   Extend their existing flows with a real full rename/IPC/native-computed-label
   check, stable selected DOM node, shared-window result lifetime/acknowledgement,
   no live announcements and an old listing/held-worker scenario. Do not add
   product test hooks, substitute direct IPC mutations for native UI actions, or
   relax existing assertions/timeouts merely to obtain a pass.
5. Use the pinned tools, locked dependencies, required formatting/Clippy/UI/build,
   updater and full CI gates. The local external ADB fixture remains quarantined;
   do not restore, rebuild/rename to evade, exclude or execute it locally. Use the
   user-authorized fresh GitHub build for fixture integration, retaining local
   pure/helper/UI checks where applicable. Verify the tested tree matches the
   final implementation, including any fixes.
6. Within the existing owned-fixture phone-test authorization, journal new short
   dedicated clips, make verified local backups before mutating them, and test
   rename plus the actual native filename with NVDA running. Confirm ordinary
   recording continues during held file work/result expiry. Preserve unrelated
   file identities/bytes, restore temporary settings/accessibility state, and keep
   wireless pairing/connection active. Raw clips/serials/manifests stay private in
   this component's ignored test-results directory. Existing phone clips and other
   plans' private workspaces are not this component's fixtures or scratch space.

Use an observed failing regression before each behavior change. A DOM-only label
test is not proof of native accessibility; a computed-label check is not proof of
audible NVDA behavior. Record those evidence levels separately. If the reported
row-label issue is already correct at one level, preserve it and identify the
remaining full-flow/result-display gap rather than introducing unnecessary DOM
replacement or unsolicited speech.

## Review focus

- One authoritative success lifetime across windows, delayed messages and bootstrap;
  no stale response/timer can erase a newer unresolved result or rearm a notice.
- Accurate mutation acknowledgement: rename metadata/index failure must not invent
  rollback, destination identity, repeated mutation or duplicate sound.
- Fresh row identity/name together, rejection of old listing and successive-rename
  races, native accessible names and stable focus/selection under real IPC.
- Focus-safe passive expiry, explicit acknowledgement across windows, stable open
  history and the stated distinction between DOM focus and virtual-cursor evidence.
- Capture/file-worker independence, preservation of guarded discard semantics,
  bilingual behavior and the unchanged four-case release quality gate.

## Approval and next stage

Self-review this written specification for placeholders, contradictory lifetime
or ownership rules, excessive scope and ambiguous outcomes. Present the committed
file to the user. Only written-spec approval permits `superpowers:writing-plans`;
the resulting implementation plan still needs its own review and execution choice
before product changes begin.
