# One-way phone-folder handoff verification

Status on 2026-10-27 (Africa/Cairo): implementation and the initial full hosted
acceptance passed. The fresh Native whole-branch review found two material issues;
the pending-control focus regression and restarted-session trace allocation now
have RED-to-GREEN fixes. Final hosted and physical acceptance remain pending. This
document does not claim measured phone timings yet.

## Behavior under test

Mark moves a finalized source video from `/sdcard/DCIM/OpenCamera` to
`/sdcard/DCIM/TakeDock` on the same phone filesystem, without overwrite. A confirmed
receipt removes the source row and retires its exact current-take discard target.
The application does not list, watch, rename, delete, restore or otherwise manage
the destination collection. The user copies files to Windows through Phone Link.

Desktop Copy/Move jobs, destination settings and video payload read/hash commands
were removed. Helper-delivery checksums and updater verification remain separate
from video operations. Source Delete/Rename and recording/readiness/discard behavior
remain covered by their existing suites.

## Hosted evidence

[Validate run 40339613815](https://github.com/wip-bunker-42/TakeDock/actions/runs/40339613815)
passed all five jobs: Rust, UI, Observer, Windows desktop and the strict quality
gate. Its source head is `a2c8e394254711d1280619a35561ac9333b2e9d0`; the actual
native checkout is `2bfb1e557b614b76d83aba5983eceec283914ef9`. The downloaded
`windows-acceptance` artifact confirms four distinct release WebView2 cases passed,
without skips:

- Offline keyboard controls and silent defaults.
- Recording dispatch and focus while verification/file work is held.
- Source handoff, batch selection, source rename/delete, current-take retirement,
  shared results and destination-sentinel preservation through restart.
- Arabic and unrelated preferences, including obsolete destination migration.

The hosted process suites additionally cover last-moment collisions, exact
identity reservation, unique per-file receipts, acknowledgement loss, cancellation
and disconnect boundaries, metadata-index warnings, and absence of production
video payload requests. Local checks use only the real helper filesystem tests,
backend library tests, UI tests and scoped production builds. The external fake ADB
is built and executed only on GitHub.

The full wire round-trip test caught serde's internally tagged enum handling of
nested `u128` timestamps. Receipts now carry the exact identity as opaque JSON text
while retaining the Rust `Identity` type. The regression proves those integers
survive the same decoder used by the host. Cancellation classification also has a
regression: ordinary source-command cancellation remains cancelled, while a
confirmed commit or unknown outcome keeps its own status.

## Physical trial and timing method

The controller is `scripts/test-phone-handoff.ps1`, with explicit executable, SDK,
serial, manifest, trial root and native-driver paths. Each trial has new owned
`TDHandoff-<trial-id>-…` names and an independent manifest/journal. Previously owned
test assets may supply read-only bytes; their old allowlists/journals do not grant
mutation authority to the new trial. Reference copies are hashed and fully decoded
before any destructive action. The controller snapshots the real Windows Known
Folder settings file, writes its temporary preferences only after checking for
concurrent changes, and restores the exact original bytes after closing the owned
app. Unexpected values are preserved for reconciliation instead of overwritten.

The ordinary release build exercises single/batch Mark, source Rename/Delete,
collision retention, and preservation of an already handed-off sentinel across
later source operations and restart. Phone Link readback is checked against the
preserved reference. A separately identified `desktop,transfer-diagnostics` build
provides backend phase observations without changing the handoff algorithm.

The native interval starts immediately before the real control click and ends
after the matching terminal notice and source-row removal are observed. It uses
the host's high-resolution clock and reports observation uncertainty. Reference
hashing, decoding, fixture staging and Phone Link readback stay outside that
interval. Diagnostic acceptance/handoff-acknowledgement/terminal observations are
reported separately from the ordinary release interval; they are not the phone's
filesystem clock.

No native Mark/Delete/Rename mutation has run for this change yet. After Wi-Fi
and wireless debugging were enabled, the saved SDK pairing reconnected without a
new code. Owned source fixtures were staged. A startup-only probe exposed that
Tauri uses Windows Known Folders despite APPDATA overrides. Startup normalized
the actual settings before the probe failed navigation; pre-probe raw bytes were
not captured, so exact raw restoration of that first attempt is not claimed. No
file action was dispatched. The controller now backs up the actual profile and
uses the existing tested guarded restore. Diagnostic sessions each receive a new
trace path and require complete loss-free matching QPC evidence. The controller
returns Home and sleeps the screen in
cleanup, checks that Open Camera is not resumed, and preserves the shared ADB server.

## Acceptance coverage and limits

| Requirement | Evidence |
| --- | --- |
| No-overwrite move, changed source/parent protection and source-only recovery | Helper filesystem and guarded-protocol suites; hosted process tests |
| Exact opaque identities, late rename/list fencing, pending reservations | Backend and UI controller suites |
| Per-row/batch Mark, source removal and keyboard context | DOM component tests and native acceptance; pending-disable focus transition has an additional native assertion |
| Unknown versus retained/committed, no automatic retry | Host protocol and process tests with a dropped result |
| Unrelated destination files survive source operations and restart | Real-helper recovery cases and native sentinel assertions |
| Delete/Rename, recording/readiness/current-take discard regressions | Backend/process suites and all four native cases |
| Legacy destination ignored without a Windows video folder | Rust settings tests, UI migration tests and native restart |
| Actual A15/Phone Link integrity and small/500 MB timings | Pending physical trial |
| Fresh whole-branch Native review | Completed; two Important findings fixed with regression tests; final hosted evidence pending |

DOM/native assertions do not constitute a listening NVDA audit. No new listening
screen-reader claim is made. Earlier desktop-transfer throughput reports remain
historical and do not measure this source-only workflow.
