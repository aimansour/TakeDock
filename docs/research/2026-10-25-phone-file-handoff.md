# One-way phone-folder handoff verification

Status on 2026-10-27 (Africa/Cairo): implementation, fresh Native whole-branch
review, its material fixes, final hosted acceptance and ordinary real-phone
acceptance passed. The separate diagnostic timing trial also passed. All five
implementation tasks are complete; no merge or release was performed.

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

[Validate run 40444035881](https://github.com/wip-bunker-42/TakeDock/actions/runs/40444035881)
passed all five jobs: Rust, UI, Observer, Windows desktop and the strict quality
gate. Its source head is `f4b5667400e43960bb3ce0b494916e7392042169`; the actual
native checkout is `66ae63045cd7ae75010bea6a702c51cf83cc8481`. Both trees are
`68e412587a3fe9367541648ec7cbe1697b359d78`. The downloaded
`windows-acceptance` artifact confirms four distinct release WebView2 cases passed,
without skips, including the pending-disable/source-removal focus assertion.
The acceptance JSON SHA-256 is
`24e71e02d4a9307dea3b4f0c32cf79b839130e91dff82acc9cf08b1916546de8`.

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

After Wi-Fi and wireless debugging were enabled, the saved SDK pairing reconnected
without a new code. A startup-only probe exposed that
Tauri uses Windows Known Folders despite APPDATA overrides. Startup normalized
the actual settings before the probe failed navigation; pre-probe raw bytes were
not captured, so exact raw restoration of that first attempt is not claimed. No
file action was dispatched. The controller now backs up the actual profile and
uses the existing tested guarded restore. Diagnostic sessions each receive a new
trace path and require complete loss-free matching QPC evidence. The controller
returns Home and sleeps the screen in
cleanup, checks that Open Camera is not resumed, and preserves the shared ADB server.

The final ordinary trial is `fb0c876dff0e48d6b218f29721130798`. It uses the
runtime source head above and ordinary `desktop` features, with executable SHA-256
`ea653f7adbc35d12761480a47110a2a328deb165e7ab56f6296e8f619ef6f241`.
The diagnostic trial is separately frozen as `6a1ec251b26b4ac2b3df0c9ea306e6c4`,
executable SHA-256
`b67805aab0f9772ea5587048e7e05c07ce02a70d1d7df7942e7bb7759c006d00`.
Both have matching helper/Observer resources and the same runtime source.
Hardware: Samsung A15 `SM-A155F`, Android 16/API 36, authorized SDK 37.0.1 over
wireless ADB. Host: Windows build `26300.9711`, 26H2, AMD64; native WebView2 and
EdgeDriver `155.0.4283.62`, tauri-driver `2.1.0`. The registered CrossDevice root
was used, rather than creating another Phone Link registration.

### Ordinary native release results

| Operation | Test bytes | Click to visible settlement | Observation bound |
| --- | ---: | ---: | ---: |
| Small, first job in app session | 23,819 | 690.7 ms | 207.3 ms |
| Small, warm | 23,819 | 601.3 ms | 195.0 ms |
| Large, first job after app restart | 500,000,000 | 879.9 ms | 163.9 ms |
| Large, warm | 500,000,000 | 700.6 ms | 167.8 ms |
| Batch Mark, two small clips | 47,638 total | 1,035.4 ms | 166.6 ms |
| Source Rename | 23,819 | 763.6 ms | 188.7 ms |
| Source Delete | 23,819 | 581.2 ms | 161.8 ms |
| Name collision, both files retained | 23,819 source | 410.7 ms | 155.9 ms |

These are individual observations, not a throughput measurement or a latency
guarantee. First jobs mean a fresh app/worker session, not a new SDK installation.
The reported interval includes WebDriver dispatch and observation overhead; the
bound accounts for polling/roundtrip observation. User-controlled Phone Link
transfer to Windows is a later separate operation. The small-versus-large results
show that this metadata move does not wait for payload size to cross the network.

All eight operations and nine exact owned source fixtures passed. All six handed-
off videos were read through Phone Link, SHA-256 matched and fully decoded, outside
the measured interval. The collision source and existing target both retained
their exact bytes. The handed-off sentinel and other handed-off clips survived
later source operations and app restart. All 60 unrelated source identities were
unchanged; final focus was on a button for every operation. Cleanup restored the
actual profile's exact bytes, confirmed Home/sleep with no resumed Open Camera,
and preserved the shared authorized ADB connection.

Raw evidence remains under `test-results/phone-handoff/<trial-id>/`: frozen
preparation/resource digests, manifest, immutable references, preparation and
operation journals, native log, readbacks and `results.json`. No destructive action
from a failed attempt is replayed. The initial zero-mutation probe is preserved
separately under trial `806a4e8d3dbc4725a867759f9c67d7da`.

### Separate diagnostic timing results

| Mark observation | Native visible interval | Backend accepted→terminal | Accepted→handoff acknowledgement | Initial source stat | Handoff protocol |
| --- | ---: | ---: | ---: | ---: | ---: |
| Small, first in session | 581.9 ms | 394.9 ms | 394.8 ms | 132.3 ms | 262.2 ms |
| Small, warm | 1,065.9 ms | 759.4 ms | 758.9 ms | 390.4 ms | 368.3 ms |
| 500 MB, first after restart | 546.4 ms | 402.9 ms | 402.8 ms | 173.0 ms | 229.5 ms |
| 500 MB, warm | 707.6 ms | 571.3 ms | 571.1 ms | 337.4 ms | 233.6 ms |

The diagnostic build is a separate observation, not an A/B speed comparison with
the ordinary build. Each matching job has a QPC header at 10,000,000 ticks/s,
ordered complete phase spans, and a zero-drop `job_complete` footer. Backend
timestamps lie inside the matching native interval. Native observation allowances
were respectively 152.9, 332.1, 144.2 and 149.8 ms. Handoff acknowledgement to terminal
took 0.13–0.46 ms; job preparation was cached at 0.001–0.009 ms. This does not measure
a first helper installation: the existing helper was already deployed, while each
stat/handoff subprocess and its metadata protocol are included in their spans.
Host acknowledgement is not a direct phone filesystem timestamp.

All four diagnostic operations passed byte-exact Phone Link readback, full decode,
sentinel/restart preservation, and cleanup. All 62 unrelated source identities
were preserved. Three app sessions allocated three distinct trace files, including
the final read-only restart, proving that `create_new` no longer suppresses a
restarted session. Matching complete traces exist for all four measured jobs.
The actual profile SHA-256 before and after each final trial was
`7edb4a1ea0210fc108f75dd8c82033c64ea0cdd32170207757681244975045c7`.

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
| Actual A15/Phone Link integrity and small/500 MB timings | Ordinary eight-operation trial and separate four-operation diagnostic trial passed |
| Fresh whole-branch Native review | Completed; both Important findings fixed RED-to-GREEN; final hosted evidence passed |

DOM/native assertions do not constitute a listening NVDA audit. No new listening
screen-reader claim is made. Earlier desktop-transfer throughput reports remain
historical and do not measure this source-only workflow.

## Native review and implementation decisions

The single fresh whole-branch review used `gpt-6-astra` at xhigh effort, from
`75476b3c65a0c6719c5ac65c52490c5f9bef308d` through
`382d902e77cefcfb29c2e3422468df99f795c452`. It found no Critical defect. Its two
Important findings received the inline RED-to-GREEN fix pass:

- Chromium blurs a newly disabled Mark control before its row disappears. The
  component now remembers the initiating context before disable, restores the
  next/previous source action or the toolbar after settlement, and abandons that
  context when the user deliberately navigates elsewhere. The full UI suite
  passes 105 tests. Actual release WebView2 acceptance and real-phone focus checks
  also passed.
- A fixed diagnostic filename conflicted with the trace writer's `create_new`
  protection on restart. Each session now gets a unique path. Timing acceptance
  requires the matching job/generation, QPC frequency, unique ordered boundaries,
  exact per-file spans and a complete zero-drop footer.

The Native execution ledger made these decisions, in order:

1. Best-effort media-index notifications use one bounded owned queue, keeping
   confirmed settlement and capture dispatch responsive. Cost: delayed indexing
   can leave a persistent warning and require a manual Phone Link refresh. It
   never triggers a repeated file mutation.
2. Real subprocess batch/unique-receipt/cancellation/disconnect proofs run on
   GitHub, respecting the ban on local fake ADB. Cost: transport defects can be
   found later in CI; completion is gated on those tests.
3. Prepare the owned-clip harness while hosted compilation runs, keeping physical
   mutations gated on all five jobs and four native cases. Cost: a later CI fix
   can require revising an unused harness.
4. Run the one fresh code review while the phone is unavailable. Cost: that
   review cannot certify hardware timings or behavior; the physical trial remains
   mandatory, and review does not waive the added native focus assertion.
5. Retain old transfer benchmarks as historical research. Cost: invoking old
   Copy/Move routines against the new runtime can fail; they are not acceptance
   tools for the source-only feature.

Deferred Minor: `scripts/test-device-files.ps1` still has the retired destination
argument/launcher contract, and `docs/testing/files.md` still describes the old
transfer acceptance flow. Use `scripts/test-phone-handoff.ps1` for the current
guarded native hardware trial. The legacy-tool/documentation repair was excluded
from the material-fix pass, as the Native workflow requires.
