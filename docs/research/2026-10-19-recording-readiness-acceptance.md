# Automated release speech-readiness acceptance

Date: 2026-10-19. The 800 ms confirmation-relative guard is accepted for the
tested phone, internal microphone and Windows default output described below.
This is a device-specific result, not a universal microphone-readiness claim.

The user requested an automated experiment instead of ten spoken repetitions,
placed the phone beside the PC speaker, and explicitly authorized calibration
and the automated trials. This changes the physical reference procedure from
the originally planned headphone/hum/user-word experiment; it does not change
the product cue, guard or capture engine.

## Build and setup

- Product source: `4d50c5d358a20e71d3b7d4ac134bdd1501bdf850`.
- Release EXE SHA-256:
  `a73031c49e20e83c801345b8f03c9b3e5cdd9300ce4ad6297768862a4b40d3dc`.
- Windows build 10.0.26300.0; WebView2/EdgeDriver 155.0.4283.62;
  tauri-driver 2.1.0. Tests ran under the ordinary current user.
- Samsung SM-A155F, Android 16, Open Camera 1.56.2, internal microphone,
  H.264 1920×1080/30 fps and mono AAC/48 kHz.
- Default Realtek Windows output played through the physical PC speaker.
  NVDA remained running. Android TalkBack remained enabled; its exact enabled
  service setting was checked unchanged after each app run.

The unchanged signed Observer and helpers are the verified 0.1.2 resources.
No emulator, development APK replacement or production test-control endpoint
was used. Starts and Stops used the real release app's rendered controls.

## Reference and analysis

A throwaway external probe played a continuous coded acoustic signal before
Start. A stereo WASAPI loopback recorded the actual Windows output, including
reader speech and TakeDock's real cue. A qualified 1100 Hz tone triggered a known
SAPI-generated sentence after the tone ended. The phone recorded the physical
speaker/microphone path. These spoken words are test stimuli, not product TTS.

The expected sound order is coded PC reference, Open Camera's phone beep,
TakeDock's PC cue, then the test sentence. Only the TakeDock cue tells the user
to speak. The product requests 1100 Hz for 100 ms; 20 ms analysis blocks observed
40–60 ms of its pure, dominant tone body, excluding ramps and partial blocks.
The detector was calibrated to that measured body and waited an additional
40 ms quiet tail. Reader speech at that frequency had far lower tonality.

An initial small-buffer probe reported loopback discontinuities. A continuous
output stream and larger capture buffer removed active-trial discontinuities;
those diagnostic attempts are not counted as main acceptance trials. Every
accepted trial has zero active capture warnings. Full decoding covered both
video and audio; waveform analysis used 8 kHz PCM derived from the original
phone files.

Raw PC-to-phone sample correlation is reduced by the physical channel's phase
and frequency response. To judge startup clipping separately, three additional
takes recorded the exact sentence twice: once after the cue and again after
capture had continued. Alignment and gain estimation used the middle of the
sentence, leaving its opening out of the fitting region. The opening was then
compared against the later recording through the same physical path.

For those controls, three constant reference tones also made initial gain
measurable without confusing changing source frequency with microphone gain.
Each frequency was normalized to its final pre-cue level; their median was
used. A near-plateau means remaining within 80–120% of that level for at least
100 ms before the cue, with 40 ms windows and 5 ms hops. Baseline relative
standard deviations were 3.6–6.1%.

## Results

Eleven formal single-sentence takes were captured: the initial ten and one
additional take. All fully decoded. Ten have a resolved cue in both the actual
PC output and phone audio. The original trial 02 has an unresolved acoustic
cue and remains unclassified for cue timing; it was retained rather than
counted as a pass. The additional take supplies the tenth resolved main take.
Each main take emitted and detected exactly one PC cue. The defined audible
sentence attack began approximately 0.35 s after the resolved cue ended; the
stimulus has a quieter leading region. This is not a measurement of zero-delay
human reaction.

The paired controls independently establish that gain was already near its
plateau before the cue, and compare the tested opening with a stable later
reference:

| Control | Near-plateau in phone audio | Cue starts | Plateau leads cue | Opening envelope correlation with later repeat | Opening attack difference |
| --- | ---: | ---: | ---: | ---: | ---: |
| 01 | 555 ms | 791 ms | 236 ms | 0.9925 | −0.75 ms |
| 02 | 580 ms | 801 ms | 221 ms | 0.9861 | +0.50 ms |
| 03 | 575 ms | 775 ms | 200 ms | 0.9717 | −0.50 ms |

The first 80 ms RMS amplitude, normalized by sentence-body gain, was
1.24–1.28 times the later repeat: the opening was not selectively attenuated.
These are waveform observations supporting preservation of the tested
opening; they are not human linguistic judgments or a promise of bit-identical
audio. The sub-millisecond attack differences are computed relative comparisons
inside the same phone recording, not independently calibrated physical timing.

Approximately ±40 ms is a practical uncertainty estimate for the onset/plateau
relationship, accounting for analysis windows and observed cue-alignment
differences up to about 15 ms. It is not a hardware-clock calibration bound.
The measured 200–236 ms plateau lead exceeds that estimate on all three controls.

All six additional real-phone scenarios passed:

| Scenario | PC cue count | Additional evidence |
| --- | ---: | --- |
| Stop before readiness | 0 | Stop confirmed; readiness cancelled |
| Pause, then Resume | 0 | Both confirmed; Resume did not create a cue |
| Disable, then re-enable | 0 | Disabled while preparing; old candidate stayed silent |
| Open another window | 1 | Second window opened during preparation |
| Disconnect and reconnect | 0 | Reconnected to the existing recording; no new cue |
| Concurrent batch copy | 1 | Real three-video copy still running after capture confirmation |

Cancellation scenarios used a 2000 ms test guard so UI and native prerequisites
could be exercised before the deadline. The speech trials and concurrent-copy
trial used the 800 ms default. There was no product delay increase.

## Fast Stop finding and cleanup

An earlier cancellation attempt sent Stop about 116 ms after confirmation.
Readiness was cancelled and no cue played, but Stop remained unconfirmed.
Fresh observation after reconnect established that the test take was still
recording; one new, explicit cleanup Stop then confirmed idle.

The cause is an existing Open Camera policy, not an Observer-error regression:
[v1.56.2 Preview.java](https://sourceforge.net/p/opencamera/code/ci/v1.56.2/tree/app/src/main/java/net/sourceforge/opencamera/preview/Preview.java)
lines 5589–5600 ignore Stop when video-start time is unset or less than 500 ms
old. The official tag peels to `0dd4cbe78872df2c6e4eb6cee3fb0d5637b0f52e`.
The successful cancellation scenarios waited 600 ms after confirmation before
issuing one action, still before their 2000 ms cue guard. The earlier result
is retained and must inform the separate discard design: deletion requires
confirmed Stop and exact take ownership. No automatic toggle retry was added.
The Samsung `service.camera.rec.running` property was not reliable in this
experiment and was not used to decide cleanup actions.

That interrupted probe had not journaled its pre-Start file identities. Its
long clip was therefore treated only as a read-only candidate, not selected
for deletion by date or “latest file.” After copying it, 13.3 seconds of its
five-frequency code matched the captured probe reference at 99.9248%, confirming
test ownership. Deletion additionally required the copy's SHA-256, full decode,
and the unchanged exact remote video identity.

All 24 experiment clips, including diagnostic attempts and cancellation takes,
were copied and fully decoded before their exact phone identities were removed.
Verified local originals and private raw logs remain under ignored
`test-results/recording-readiness-2026-10-19/`. The final authoritative state was
connected, foreground video mode, observed/predicted idle, zero pending commands
and inactive readiness. App settings were restored byte for byte; Android
accessibility services were unchanged. Raw clips, screenshots, profile backups,
phone names and connection details are not publication artifacts.

## Software review and limits

The one fresh whole-branch review found one Important issue: a live Observer
read/protocol error could retain a confirmed readiness candidate. The own fix
pass first reproduced both the preparing candidate and an unclaimed queued
cue, then invalidated readiness under the owning-generation model lock and
broadcast outside it. Recovery observations cannot revive the cancelled cue;
file access retains its authorized lease. Both regressions passed and the
whole suite is green: 92 Rust tests, 43 UI tests and all four real release
desktop cases, with strict Clippy, formatting and Svelte/TypeScript checks.
There were no Critical findings or deferred Minor findings.

The final desktop run recovered two WebDriver stale-element warnings during
reactive UI updates; all four cases completed successfully. These are retained
in the private verification log, not counted as failed cases or hidden.
The updater validation example also reports a missing default-window-icon
resource while its signed-update checks pass and no installation runs. This
non-blocking diagnostic is a deferred Minor follow-up; it does not establish
an icon failure in the tested release application.

This acceptance covers this device/internal microphone and the tested default
output. Headphones, external microphones, different camera settings, other
phones and future OS/camera changes require their own checks. The preparation
preamble remains. The other requested features remain separate planned work.

Sanitized measurements are in
[the results JSON](2026-10-19-recording-readiness-results.json). No private file
identity, phone connection endpoint or credential is included there.
