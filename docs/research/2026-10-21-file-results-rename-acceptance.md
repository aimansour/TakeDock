# Shared file results and verified rename acceptance

Status: implementation and the complete post-review hosted gate pass. The single
whole-branch review and its one fix pass are complete. Audible NVDA assessment was
explicitly deferred by the user and is not claimed here.

## Implemented behavior

Clean file success uses one process-owned ten-second deadline shared by every
window. Failures, cancellations, retained/unknown outcomes and warnings remain
until a revision-specific acknowledgement. Native history starts closed and
retains details after success expiry; records being read stay while it is open.
A removed result containing keyboard focus stays locally until focus leaves.
An explicit local hide can return focus once, unless it has already moved or a
newer warning remains. Capture/settings messages retain their separate owner.

A confirmed rename publishes the complete fresh Video before media indexing or
the next queued job. Exact source identity/name and worker execution sequence
prevent stale listings and reversed rename links from restoring older metadata.
The existing inode-keyed checkbox remains selected, with its native name updated;
the next operation uses the fresh full identity. Metadata/index problems after
acknowledgement remain warnings without retry or fabricated rollback.

The physical collision trial exposed a legacy ADB transport issue: `exec-out`
can return local status zero after a helper failure. A read-only remote `false`
probe produced `exec-out=0`, `shell -T=1`. The fixture was corrected to model that
behavior, its regression observed the wrong completed outcome, and job result
commands now use `shell -T` for remote exit status. Binary copy streaming keeps
its existing transport and verification; guarded discard keeps its handshake.

## Red and green evidence

| Check                                               | Evidence                                                                                                                                                                                                      |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native missing rename update, before either fix     | [39537593382](https://github.com/wip-bunker-42/TakeDock/actions/runs/39537593382), 3/4 release cases; case 3 failed its held-index native label assertion                                                     |
| Process missing rename fields/event                 | [39540989236](https://github.com/wip-bunker-42/TakeDock/actions/runs/39540989236), 8/11 file cases; intended missing-event/destination failures                                                               |
| Rename process/native milestone                     | [39542130212](https://github.com/wip-bunker-42/TakeDock/actions/runs/39542130212), full workflow successful and actual artifact lists all four passing release cases                                          |
| Native missing result/history view, before view fix | [39542242459](https://github.com/wip-bunker-42/TakeDock/actions/runs/39542242459), 3/4 release cases; case 3 failed because native history did not exist                                                      |
| Faithful legacy ADB collision regression            | [39545066628](https://github.com/wip-bunker-42/TakeDock/actions/runs/39545066628), 10/12 file cases; completed versus failed observed before transport fix; obsolete remainder cancelled after preserving RED |
| Corrected transport process suite                   | [39545216837](https://github.com/wip-bunker-42/TakeDock/actions/runs/39545216837), Rust workspace successful including all 12 file cases; superseded desktop run cancelled                                    |

The unchanged native Cancelled assertion in
[39544265608](https://github.com/wip-bunker-42/TakeDock/actions/runs/39544265608)
also exposed missing explicit cancellation text alongside a retained discard
outcome. Its focused component regression failed before the UI fix, then passed.
The subsequent release run
[39546564835](https://github.com/wip-bunker-42/TakeDock/actions/runs/39546564835)
passed Rust/UI/Observer but failed the remote-acknowledgement focused-card
assertion (3/4 native cases). Additional unchanged-production component,
Chromium and real-phone tests preserved focus through newer success/expiry.
Native checks now explicitly prove focus is established before newer results
and remains afterward, retaining every original assertion and timeout. Final
hosted acceptance remains mandatory; the earlier failed run is not a pass.

The diagnostic run
[39589254513](https://github.com/wip-bunker-42/TakeDock/actions/runs/39589254513)
passed both focus preconditions, then failed the original protected-card
assertion. Its additional retained discard result exposed the missing input in
the simpler phone/component checks. With two persistent notices, acknowledgement
kept the focused card but moved it after the other card; Chromium lost native
focus during that DOM move, and a later snapshot removed it. The component slot
regression failed before the fix. Reconciliation now preserves surviving card
positions and prepends only new results; real Chromium retained the focused node
through repeated acknowledgement snapshots. JSDOM alone did not expose native
focus loss on DOM movement, so real browser/release checks remain essential.

Late acceptance/progress/bootstrap regressions also reproduced completed jobs
returning as active after eviction of their detailed terminal records. Compact
generation-scoped terminal IDs now preserve terminal truth independently of the
twenty-record presentation cap. A terminal bootstrap replaces an earlier queued
event, while an older active bootstrap cannot revive completed work.

The fresh review found two Important lifecycle defects: an accepted committed
operation could lose its terminal outcome after disconnect, and history eviction
could strip per-file details from a still-visible success. The executor promoted
the backend queued-publication race to Important as well: enqueue sends work
before publishing queued, so an evicted terminal job could later appear active.
All three regressions were observed failing before the single fix pass.

Known accepted operations may now settle historical outcomes under their original
generation, without updating current active work or video rows. Eligible notices
keep complete per-file data independently of the twenty-record history cap; retired
caches are compacted lazily. Backend terminal IDs prevent late queued publication
from reviving completed work. The actual hosted FileService sink now feeds the
shared FileResultStore, and its post-acknowledgement disconnect case asserts the
durable original-generation destination and outcome rather than raw output alone.

Local final checks include 102 Rust library tests, 25 helper tests, strict desktop
Clippy, formatting, zero Svelte diagnostics, 87 UI tests, production frontend and
actual release desktop build. The real signed updater fixture rejects corrupt,
wrong-version, downgrade and malformed responses without installing anything.
All three Android helper ABIs build with the updated locked toolchain. Compatible
dependencies and provenance are documented in
[the maintenance record](2026-10-21-compatible-dependencies.md).

## Physical phone evidence

Tested source: `88ee8f181a80a41d122dbc2cbc42fda99bbe0698`.
Local release EXE SHA256:
`e56552a15bd900231fc8c6be99139d427afdd9e669ae181eabd65e794aa18cb6`.
Actual arm64 helper SHA256:
`3388eaf75eda20144f0268905707592b3aeb326dde888e868fb91868be4ad383`.
Observer APK SHA256:
`166ded3e31e88f6e9555a6246ac085e3f2b5aab99dc575d1e35a7ab1753bc874`.
This is the exact final hosted Observer artifact and matches the hosted desktop
acceptance resource. Preflight caught an obsolete local version-3 APK beside the
EXE; the ignored resource was replaced with this verified version-4 artifact.
Android rejected its different CI debug certificate as an in-place update, so
the previous stateless Observer APK was privately backed up before replacing only
that package. No personal-media package or ADB pairing was removed. A future
differently signed Observer may similarly require replacing this test package.

The actual WebDriver-controlled release application passed:

- Full fresh rename metadata, computed checkbox name, identical checkbox node
  and selected state, followed by verified copying with the new identity.
- Shared success deadline in two UI-opened windows, ordinary expiry without
  moving the selected checkbox focus, and initially collapsed history.
- A collision between two owned trial clips preserving both hashes, a failed
  result surviving a later clean success, and native acknowledgement in another
  window retaining its focused Hide button in its original slot beside another
  persistent failure until focus leaves. Actual focus and identical element ID,
  rather than card presence alone, are asserted.
- Arabic native history and computed Hide labels, with no live/alert/status
  regions. This proves computed semantics and DOM focus, not audible output or
  screen-reader virtual-cursor behavior.

Eight new clips were journaled; each has a matched-SHA256 local backup and a full
ffmpeg decode before any rename. Initial pending MediaStore names were allowed
to finalize before backup; no pending filename was guessed as a final path.
The original twelve unrelated file identities and hashes were unchanged. The first
post-review preservation check also detected twenty-six additional files beyond
that earlier baseline. They were left untouched, captured in a separate
immutable identity/hash preflight, and included in the final repeated run's
preservation check: thirty-eight unrelated files in total. The original baseline
was not overwritten. The repeat reused the already verified same-source expiry
clip, so no ninth recording was needed.
Backup hashes, current app-settings preflight bytes and Android accessibility
preflight were checked afterward; wireless ADB remained connected. Earlier trials
observed externally changed TalkBack settings. Each final run preserved its own
current preflight state rather than restoring an obsolete earlier state.
Ready sound/delay differed from the original trial after the installed application
had been used. The user confirmed they had not manually changed these settings;
the cause of the difference is not established. The final run used a separate
immutable current-settings baseline and preserved it byte-for-byte. Neither the
original profile nor file baseline was replaced.
The earlier trial also verified NVDA remained running;
audible assessment is deferred by the user. Private
filenames, device identifiers and raw reports are retained outside tracked docs.

WebDriver click-to-Stop-control observations were 1124.827 and 173.436 ms for the
initial ordinary capture controls at `ca6e781` with the unchanged capture module,
and 103.550 ms during result lifetime/expiry on the final `88ee8f1` EXE. The final
run also explicitly proved button focus before and after newer results and their
passive expiry. Selection is synchronized with the completed UI listing after
a rejected collision before submitting another rename.
These include control/browser/device round trips, are not raw camera-dispatch
latency, and establish no invented performance threshold. Deterministic held-file
worker capture independence is checked by the hosted external process/native
tests. Backup/hash/decode belong solely to trial tooling and add no production
capture/delete work.

## Final tree and review gate

[Validate 39597502081](https://github.com/wip-bunker-42/TakeDock/actions/runs/39597502081)
completed successfully on source `88ee8f181a80a41d122dbc2cbc42fda99bbe0698`:
all five required Rust/UI/Observer/desktop/quality-gate jobs passed. The downloaded
`windows-acceptance` artifact records exactly the four required titles, each
with `passed: true`, and `conclusion: passed`. Existing 90-second case / 15-second
shared wait bounds remain unchanged.

The artifact's synthetic merge commit is
`de4cda6aa0e7cad7f60e65364339d1c4f43005f3`. Its complete tree and the tested source
tree both equal `d1f7171f46dd19b694b11e806c622e7434cb074b`. Subsequent evidence-only
commits update this record, the maintenance record and the testing guide; they
leave the accepted application, tests and tooling unchanged.

Hosted Windows: OS `10.0.26100.0`, Node `26.12.0`, WebView2 and EdgeDriver
`154.0.4258.62`, tauri-driver `2.1.0`. Hosted tested EXE SHA256:
`6920c0fc3d3a573a15bd58fa9461f26b7a02b860dddfebdb3f06454683355554`.
This is distinct from the local phone-tested release hash above; both builds
identify the same source. The physical test used local WebView2/EdgeDriver
`155.0.4283.62`.

One fresh whole-branch review covered base `4dc44e7` through `3563716`; its two
Important findings and the executor-promoted race above were fixed in `88ee8f1`.
The complete post-review workflow, actual four-case artifact and exact source-tree
comparison passed. Native execution used one review and one fix pass, without a
second review. One Minor is deferred: the English/Arabic keyboard guides omit the
explicit twenty ordinary-history bound and unresolved/open-history exceptions.
