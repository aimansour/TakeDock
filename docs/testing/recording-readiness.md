# Recording speech-readiness acceptance

The [approved design](../superpowers/specs/2026-10-15-recording-readiness-design.md)
uses fresh confirmation of a newly requested Start plus a configurable guard.
It emits one 1100 Hz, 100 ms PC cue. Speak after this cue finishes. Preparing and
ready are ordinary text, with no live region, TTS or focus move. The Windows
session's default audio output and volume remain under the user's control.

The readiness setting defaults on independently of optional success feedback.
The proposed initial guard is 800 ms, with accepted integer multiples of 50
between 500 and 5000. A new delay or re-enabled sound applies to the next Start;
disabling invalidates pending playback immediately. Visual readiness still
advances with sound off. Stop, Pause, later capture intent, loss of eligibility,
unknown after confirmation, failure, replacement and shutdown cancel pending
playback. Resume and connecting to an existing recording never create a new cue.
The preparation preamble remains in the recording.

## Automated evidence — 2026-10-19

- Rust 1.98.1: 92 workspace/model/process/helper tests passed. Deterministic clock
  cases cover confirmation-relative deadlines, one claim, cancellation,
  unknown-before/after-confirmation, settings changes and saturating arithmetic.
- Eight injected-player audio tests use explicit barriers and a later success
  sentinel. They cover queued cancellation, disable/re-enable, stale token
  ordering across sequences/generations, priority, bounded feedback, shutdown,
  deduplication and ordinary errors without an audio retry. They play no sound.
- Real-process ADB tests prove all four capture commands arrive while observation
  is held and while an actual test audio player is blocked. They also cover
  queue-failure rollback, unclaimed retired tokens and live policy/reconnect.
- All 43 UI tests passed, including English/Arabic readiness, stable focus,
  independent keyboard settings, invalid numeric input and shared windows.
  Svelte/TypeScript reported zero errors/warnings; formatting and production
  frontend build passed.
- Strict Clippy passed for the workspace, all targets and desktop feature.
  Four release/publication/tooling script tests passed.
- Release EXE/WebView2 acceptance: all four extended cases passed on
  Windows build 10.0.26300.0, WebView2/EdgeDriver 155.0.4283.62 and tauri-driver
  2.1.0, under the ordinary current user. The report identifies product commit
  `4d50c5d358a20e71d3b7d4ac134bdd1501bdf850` and EXE SHA-256
  `a73031c49e20e83c801345b8f03c9b3e5cdd9300ce4ad6297768862a4b40d3dc`.
  Complete executable/resource hashes are in `test-results/desktop/acceptance.json`.
- The updater fixture passed signed download, corruption rejection, signed
  version binding, downgrade and malformed-manifest checks; no update was installed.

The unchanged signed Observer and three ABI helpers were reused from installed
0.1.2 only after SHA-256 matched its published `acceptance.json`, and the Observer
and helper sources had no diff from tag `v0.1.2`. A development APK was not used
to replace the phone's signed Observer.

## Physical acceptance procedure

The user approved an automated external-speaker reference on 2026-10-19 in
place of the manual headphone/hum/word procedure below. The actual protocol,
controls, uncertainties and results are recorded in
[the automated acceptance report](../research/2026-10-19-recording-readiness-acceptance.md).

1. Identify the release EXE by SHA-256 and record current Windows, phone and Open
   Camera versions. Save existing app settings and the phone's actual enabled
   accessibility services. Keep those services in their existing state.
2. Use exactly one authorized real phone, its internal microphone, Open Camera
   idle in video mode, PC headphones, readiness enabled and an 800 ms guard.
3. For at least ten new Starts through the real EXE controls, supply a continuous
   audible hum reaching the phone before Start. After the PC cue finishes, speak
   marked initial words identifying the trial. Stop each take through the app.
4. Save only the newly created test clips and their native timing logs under
   ignored `test-results/`. Fully decode both tracks, inspect the opening
   waveform/ramp and listen to the marked words. Record cue count and whether
   each beginning is complete. State clock/reference uncertainty explicitly.
   PC tones confined to headphones are not an acoustic reference in the phone
   recording.
5. Exercise Stop/Pause and disconnection before readiness, another window during
   preparation, disable/re-enable while pending, and concurrent file work. Check
   that cancelled cues stay silent and a normal new Start yields one heard cue.
6. Restore saved settings only if no intervening user change supersedes them.
   Remove only test-created phone files after stat/hash verification against
   successfully decoded local copies. Leave capture stopped.

## Physical evidence status

Accepted for the tested setup on 2026-10-19: eleven formal single-sentence takes
provided ten resolved acoustic cue references and one explicitly unclassified
take; three paired repeat controls supported preservation of the tested opening.
All six real-phone cancellation/window/concurrent-copy scenarios passed. The
phone's measured near-plateau preceded the cue by 200–236 ms, with approximately
±40 ms timing uncertainty. NVDA and TalkBack remained active. All 24 test-created
phone clips were removed only after verified local copies and full decoding;
capture ended idle and app settings/accessibility services were restored.

The [2026-10-15 diagnostic probe](../research/2026-10-15-recording-startup-measurements.md)
predates this cue and cannot certify its audible routing or release behavior.
The 800 ms guard is accepted for the current phone/internal microphone and
tested default speaker output. External microphones and other configurations
remain unverified. Open Camera's existing 500 ms fast-Stop exclusion is recorded
in the acceptance report and must be handled by the later discard design.
If real acceptance fails, return to diagnosis and design review rather than
silently increasing delays or claiming universal microphone readiness.
