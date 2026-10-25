# One-way phone-folder handoff; remove desktop transfers

Date: 2026-10-25, Africa/Cairo.
Status: approved by the user on 2026-10-25; implementation planning is next.
Working base: `1dbb32e25e2f22ee151c2ed1a2eb414d660d5610`.

## Intent and user decisions

TakeDock will prepare recordings on the phone. The user will copy them to Windows
manually through Phone Link/File Explorer. Remove TakeDock's desktop Copy and Move
features completely from the application.

The user explicitly clarified the meaning of marking a video: move it into the
phone's TakeDock folder and remove it from the application's list after success.
TakeDock then stops managing that file. There is no marked-files collection,
unmark action, automatic return, or destination-folder management in the app.

Keep ordinary recording control, readiness, current-take discard, and source-folder
Delete/Rename. Their existing behavior and accessibility contracts remain relevant.
The recording scope is an implementation assumption based on the request removing
desktop file transport, rather than a request to remove recording controls.

The proposed destination is `/sdcard/DCIM/TakeDock`, a sibling of the existing source
`/sdcard/DCIM/OpenCamera`. In Phone Link this is Internal storage > DCIM > TakeDock.
Its exact name/location is the designer's default, not a user-chosen custom path.

## Approaches and selected boundary

The earlier proposal kept handed-off videos visible and managed both phone folders,
with a possible return action. The user rejected that ownership model and selected
one-way handoff: the source list ends when the file leaves OpenCamera successfully.

Use a same-filesystem, no-replace directory-entry move on Android for handoff. It
changes the phone file's location without streaming its payload to Windows or
copying it to another phone file. A copy-and-delete fallback would reintroduce
payload-sized waiting and broader failure states, so it is excluded. A mere saved
selection would not put the video where the user expects to find it in Phone Link.

On approval, this design supersedes desktop-transfer requirements in the original TakeDock
design and the active large-transfer optimization specification/plan. Retain their
research and test evidence as historical records. Installer language and the
existing queued wireless connection component keep their separate scope. Neither
implement recovery nor continue transfer optimization as part of this change.
Do not merge the earlier transfer PR as a prerequisite for this new behavior.

## User-facing behavior

- List and manage videos only in OpenCamera. Do not enumerate, watch, index into an
  app-owned collection, display, rename, delete, restore, or clean up TakeDock's
  destination files after handoff. No destination tab or managed-file database.
- Provide a native action labeled Mark / تعليم that immediately queues a one-way
  handoff. The designer's UI default is an action button, since this is a command
  whose successful row disappears, rather than a persistent checked state.
- Existing selection checkboxes continue to select rows for batch actions. Add
  Mark selected / تعليم المحدد alongside Delete and the single-file Rename action.
  Selection alone must never unexpectedly move a file while the user is selecting
  it for Delete or Rename. A per-row Mark action supports the direct single-file
  operation the user requested without a separate confirmation dialog.
- Only ready/finalized videos can be marked. Protect recording/finalizing files
  and stale/replaced identities using the existing backend protections.
- A pending row stays visible and cannot receive competing local file actions.
  Hide the exact source row in every window only after confirmed handoff. Preserve
  remaining selection and keyboard context; focus the next actionable row, then
  the previous row, then the stable toolbar if the focused row was removed.
- On a definite pre-commit failure, retain the original row and report the actual
  reason. A destination collision is an error: keep the source and existing target
  unchanged. Do not silently replace or invent another destination filename.
- On a lost acknowledgement, show an unknown outcome rather than success or a
  claim that the source was retained. Do not automatically retry a mutation.
  A fresh source listing remains the authority for what the app can manage.
- Keep the shared ten-second success lifetime, persistent errors/uncertainty,
  collapsed result history, bilingual names and silent screen-reader behavior.
  History describes the completed command; it offers no destination-file actions.
- No desktop destination selector, transfer progress UI, Copy/Move-to-computer
  controls, or request to install/open/configure Phone Link. The user owns that
  external copy workflow.

## Mutation and ownership contract

The handoff helper is permitted to create and open the fixed destination directory
and inspect the single exact operation target to validate and finish that command.
That short operation-level access does not grant ongoing destination ownership.
After the operation settles, source enumeration remains confined to OpenCamera.

The backend owns one bounded file-operation queue, device lease/generation checks,
per-file cancellation and authoritative results. Capture dispatch never waits for
that queue, directory refresh, media indexing, results, or helper recovery.

For each selected source:

1. Validate its name, expected identity and current device generation. Acquire the
   existing shared helper mutation lock. Check it is a finalized regular video.
2. Lazily create/validate the fixed TakeDock directory safely. Reject a file or
   symlink substituted for that directory, unsafe names, an existing target, a
   different filesystem, or a changed source. Do not accept arbitrary destination
   paths from the frontend or silently fall back to copying across filesystems.
3. Use directory handles, no-follow source access, a protected source claim and
   recoverable intent following the existing mutation pattern. Verify that the
   claimed file is the expected object before publishing it to the destination.
   Use a no-overwrite rename across the two anchored directory handles.
4. Commit by confirming the moved object retains the expected device/inode,
   size and content modification time. Its metadata change time may legitimately
   change. No payload hash, full video read/decode, network download or desktop
   staging file belongs on this production handoff path.
5. Publish an operation-correlated result and source-row removal. Complete any
   media-index notification outside recording dispatch; an index warning cannot
   turn a committed move into a claimed retention or trigger a move back.

Keep pre-commit recovery and post-commit cleanup distinct. A crash while a source
claim exists must be recoverable to an intact source without overwriting another
file. Recovery must never pull a successfully published destination file back into
OpenCamera. Clean only owned intent/claim metadata; never sweep destination files.
Persist enough intent/identity to distinguish a committed handoff from an
uncommitted source claim, using a format compatible with legacy mutation recovery.
The builder chooses the smallest safe journal extension and tests its crash points.

Cancellation before the commit boundary leaves/restores the source. Once the
same-filesystem move commits, cancellation cannot undo it or relabel it cancelled.
Batch results remain per-file; a conflict in one item does not undo earlier handoffs
or block unrelated later items. Do not overlap two mutations of the same identity.

Own temporary source-claim watch events must not make a row look successfully
handed off before commit. Correlate pending operations, terminal outcomes and
source watch updates without waiting for a later full listing. Real external
source removals through Phone Link still reconcile truthfully. Reconnect or a new
window cannot replay handoffs, adopt another phone, or resurrect stale commands.
Handoff must retire a matching ended current-take discard target, so a later
discard action can never reach the handed-off video. It does not stop recording.

## Removing desktop-transfer behavior

Remove Copy/Move job kinds and their release IPC acceptance, desktop destination
creation/picker and setting usage, payload transfer/desktop publication and
transfer-only runtime paths. Remove the file helper's transfer-only read/hash CLI
commands from the shipped interface when no remaining production behavior needs
them. Do not keep a hidden runnable transfer feature in the release binary.
Review dependencies/usages before removing transfer-only code or libraries; keep
anything still required for updates, helper delivery, or other existing features.

Legacy settings containing a Windows destination must still load. Ignore the old
field and omit it from new settings rather than requiring the old directory to
exist or creating one. Preserve all unrelated settings. Never delete previously
copied desktop files, phone files, existing folders, pairing keys or user settings
as a migration step. Existing files in TakeDock remain outside app ownership.

Keep ordinary source Delete/Rename guards and current-take discard authorization.
Their production paths must not acquire the removed transfer verifier. Device-side
fixture and preservation tooling remains separate from the shipped feature.

## Verification and acceptance

Use meaningful pure helper/Rust/UI tests and the real native application. Preserve
the restriction against building/restoring/running fake ADB locally, including
through workspace tests, desktop fixtures or WSL. Hosted GitHub validation owns
those fixture/native acceptance paths. Update transfer-dependent acceptance cases
to test the actual handoff contract without weakening unrelated gates.

Required coverage:

- A ready video moves to TakeDock exactly once, retains identity/content and
  disappears from every source view. Unrelated source/destination files remain
  intact. Restart/refresh does not create a managed destination collection.
- Arabic, Unicode, spaces and quotes, multiple selected files, duplicate basenames,
  destination collisions, symlinks, unavailable or different-filesystem targets,
  source replacement races and stale phone generations are covered.
- Test both sides of cancellation/commit, disconnect/lost acknowledgement, helper
  crash and each recovery boundary. A successful destination is never restored,
  deleted, renamed, retried, watched or swept by later app maintenance.
- Recording/finalizing videos cannot be marked. Source Rename/Delete and ended
  current-take discard continue to work; handed-off files cannot become discard
  targets. Held file work does not delay recording command dispatch.
- Verify the per-row Mark action versus ordinary batch selection, remaining
  selection, row removal/focus continuity, multiple windows, result expiry/history,
  Arabic/English labels, keyboard controls and absence of unsolicited speech.
- The release UI/IPC has no desktop Copy/Move feature or destination selector.
  Old settings load without creating a Windows destination or changing unrelated
  preferences. The source list does not traverse TakeDock.
- In the authorized phone test stage, use dedicated test clips with locally
  preserved, size/hash-checked and fully decoded backups before mutation. Verify
  the handed-off file is discoverable through Phone Link and byte-identical;
  then prove the app's normal listing and source operations leave it alone.
- Time marking on small and large owned files, including a cold helper start.
  Record native user-visible completion and backend time separately. Verify the
  path performs metadata relocation without payload transfer or full-file hashing;
  do not impose the old lengthy desktop-transfer benchmark matrix or claim a
  specific latency before measurement. Full decode/hash belongs outside timing.

Use the current trusted phone and existing authorization, preserve the shared ADB
service/pairing and current settings, and leave the phone Home/asleep after every
phone test session. Do not leave Open Camera foreground between stages.

## Decisions for review and builder discretion

The user's explicit decision is one-way removal from app ownership after marking.
The designer's defaults to review are the fixed `DCIM/TakeDock` location, a native
Mark action distinct from batch-selection checkboxes, and collision failure rather
than automatic filename changes. No extra confirmation is proposed for marking.

The builder owns the narrow journal/protocol representation, safe shared-directory
opening details, event correlation and exact test layout within these contracts.
Keep this on the existing Tauri/Rust/Svelte stack and the user's Native execution
workflow. Design approval leads to an implementation plan via writing-plans before
product code is changed. No merge or release is implied by this document.
