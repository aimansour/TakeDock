# File results and confirmed rename regression checks

The approved scope is recorded in
[the spec](../superpowers/specs/2026-10-20-file-results-rename-design.md) and
[implementation plan](../superpowers/plans/2026-10-21-file-results-rename.md).
Compatible dependency maintenance is authorized by the user's later amendment.

## Local deterministic checks

Run the locked Rust library and helper tests, strict desktop Clippy, Svelte
diagnostics, UI tests, formatting and production frontend build:

```powershell
cargo test --locked -p takedock --lib
cargo test --locked -p takedock-files
cargo clippy --locked -p takedock --all-targets --features desktop -- -D warnings
cargo fmt --all --check
npm audit --audit-level=moderate
npm run format:check
npm run check
npm run test:ui
npm run build
./scripts/check-updater.ps1
```

The result model/controller suites use injected monotonic clocks and deferred
responses for the exact 10,000 ms boundary, unchanged shared deadlines,
acknowledgement revisions, later warnings, duplicate terminals, disposal and
retention. Session regressions also deliver acceptance replies, progress and
bootstrap out of order after more than twenty terminal jobs, ensuring that
evicted work cannot reappear as active. Backend tests also delay queued publication
past terminal eviction, preserve known accepted outcomes across generation changes,
and keep complete eligible notice details after more than twenty newer results.
A focused rename-destination component regression covers history eviction and
passive retirement. The reconciler tests deliver A→B→C in both orders, exercise
same-identity A→B→A cycles and preserve exact integer digits above JavaScript's
safe range. Component tests retain actual checkbox/button objects and distinguish
passive focus protection from explicit local focus return. Open history retains
displayed keys through retention churn until closure.

## Hosted external process and release checks

The local external fake ADB remains quarantined. Do not restore it, rebuild it
locally, change exclusions or use another subsystem to run it. The unchanged
Validate workflow runs full workspace tests on fresh hosted runners, including:

- Rename update before held indexing/next job, full destination metadata,
  post-acknowledgement metadata warnings and cancellation boundaries. The actual
  FileService event sink includes FileResultStore: post-acknowledgement disconnect
  must preserve the original-generation historical destination and outcome.
- Faithful legacy `exec-out` behavior: remote helper failure can accompany local
  exit zero. File mutation acknowledgements therefore use `shell -T` and its
  remote exit status. The real phone's read-only `false` probe returned zero
  through `exec-out` and one through `shell -T`; the explicit process regression
  reproduced a protected collision being misclassified as completed before the
  transport fix. The distinction is described by the
  [ADB service protocol](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/docs/dev/services.md).
- Existing guarded discard and held-hash capture regressions.
- All three Android helper ABIs, Observer tests/lint/build, updater signatures,
  release executable, unprivileged WebView2 automation and final quality gate.

Exactly four release cases remain mandatory:

1. launches offline with named keyboard controls and silent success defaults
2. dispatches all four actions while verification is held and retains logical focus
3. selects two videos, hides batch rename and copies verified bytes through Rust IPC
4. persists Arabic, destination and optional success sound across application restart

The third case retains original batch/hash/capture/cancellation checks. It reads
bootstrap/result/listing projections only for synchronization; rename, copy,
acknowledgement, capture and new-window actions use actual native controls. It
checks the renamed checkbox's computed label within 1,000 ms while indexing is
held, retains its WebDriver element ID, queues copying through its refreshed
identity and proves capture reaches the persistent command process during held
hashing. Window scenarios compare the same backend expiry, check late-window
bootstrap/history, preserve a failure through later success and verify focus
protection after another window acknowledges it. Existing 90-second cases and
15-second shared waits remain unchanged.

The missing native rename label was reproduced before the rename fix in
[Validate 39537593382](https://github.com/wip-bunker-42/TakeDock/actions/runs/39537593382).
The absent native history/result presentation was reproduced before those view
changes in [Validate 39542242459](https://github.com/wip-bunker-42/TakeDock/actions/runs/39542242459).
Acceptance of the final tree additionally requires all workflow jobs successful
and the downloaded artifact listing all four actual passing cases.

## Physical phone and audible observations

Use [the physical procedure](../research/2026-10-21-file-results-physical-procedure.md)
with new journaled clips, verified backups/full decoding before mutations, a
private unrelated-file baseline and settings/accessibility preservation. Keep
wireless debugging paired and connected afterward. Neither fixture preservation
nor decoding belongs in the production capture/delete path. Record native names,
focus and timing separately from the user's audible NVDA/virtual-cursor report.
