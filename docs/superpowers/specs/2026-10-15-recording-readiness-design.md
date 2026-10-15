# Recording speech-readiness cue

Date: 2026-10-15. Status: approved by the user; implementation-plan review is next.
Implementation has not started. The user approved a separate TakeDock cue through
their PC headphones.

## Intended outcome

The user speaks immediately after Open Camera's phone beep and loses the beginning
of their hook. Audio and video remain synchronized in the resulting recording;
the reported problem concerns capture startup. A dependable cue must tell the
user when to begin speaking without making capture controls wait for verification.

TakeDock will play a distinct, short cue through the Windows session's default
audio output, which the user currently routes to headphones. The cue belongs to
one newly requested take and must never replay because another window opens or
old verification arrives. Ordinary status text remains accessible without live
announcements or focus changes.

## Evidence and its limits

The approved probe used Android 16, Open Camera 1.56.2, Camera2, H.264 1080p/30,
and mono AAC/48 kHz without attaching the external phone microphone. Ten trials
have complete timing data; eleven MP4s passed full decode verification.

- Phone command marker to native recorder-start invocation: 537–706 ms,
  median 625 ms.
- Recorder preparation alone: 141–277 ms.
- Every decoded audio track began with 320 ms of exact digital zeros. The user's
  manually hummed reference became detectable above −80 dBFS at about 346 ms,
  then increased in level over the following few hundred milliseconds.
- Upstream requests the start sound before recorder preparation and start. The
  installed Observer requires the pause control as well as the changed shutter
  description; that pause control appears after recorder start returns.

The native start call, first physical sample, and encoded data arriving at the
muxer are different events. The probe did not independently record the physical
onset of the phone beep. It also did not establish external-microphone behavior
or a universal readiness interval for other phones. See the
[measurement report](../../research/2026-10-15-recording-startup-measurements.md).

## Approaches considered

1. **Confirmed recording plus a measured guard interval — selected.** Keep Open
   Camera and the existing Observer. Start the readiness interval only after a
   fresh confirmation of the current Start command. This incorporates variable
   camera preparation time and adds a guard for the observed audio startup.
   The interval requires real-device acceptance; it is not direct sample-level
   knowledge from Open Camera.
2. **Delay measured from button activation.** Simpler, but camera preparation can
   consume a variable portion of the delay. A value sufficient for one trial
   can expire too early on a slower start. This does not meet the cue's purpose.
3. **Own or modify the phone capture engine.** Direct control could expose capture
   buffers and prewarming. It expands the work into maintaining phone recording
   behavior and Open Camera integration. It is outside this component's scope.

## User-visible behavior

After Start, a short ordinary text line says **Preparing to speak** /
**جارٍ التجهيز للكلام**. Recording controls remain immediately usable. Once the
current Start is confirmed and its guard expires, the line becomes
**Ready to speak** / **جاهز للكلام**, and TakeDock plays one 1100 Hz, 100 ms tone.
The user begins speaking after this tone finishes. There is no TTS, live region,
automatic focus move, or repeated ready announcement.

The cue is for a newly requested Start only. Resume does not replay the Start
cue; pause/resume startup has not been measured in this probe. Connecting to an
already recording phone also does not produce a fresh cue.

Settings add two persistently saved bilingual controls:

- `ready_sound`: **Play the ready-to-speak cue** /
  **تشغيل صفارة الجاهزية للكلام**, enabled by default and independent of
  `success_sound`.
- `ready_delay_ms`: **Speech-readiness delay after confirmation (milliseconds)** /
  **مهلة الجاهزية للكلام بعد التأكيد (مللي ثانية)**, initially 800 ms, integer
  multiples of 50 in the range 500–5000. The help explains that this is measured
  after camera confirmation and reducing it requires checking the first spoken
  words on that phone.

The 800 ms default is a proposed guard for the measured startup and gradual
audio onset, not a claim of already-verified playback timing. Shipping this
default requires the physical acceptance below; a failed acceptance returns to
diagnosis and design review rather than silently adding repeated delays.

Saving a new delay affects the next Start. Disabling the cue invalidates any cue
not yet started. Re-enabling it takes effect on the next Start and does not
replay a cue for the current take. The visual readiness state still advances
when the tone is disabled. These changes do not require reconnecting the phone.

When the readiness cue is enabled for a Start, generic success feedback for that
Start is suppressed, so an earlier confirmation tone cannot be mistaken for the
speak-now cue. Other success and failure feedback retain their existing meaning.

## State, interfaces, and cancellation

The Rust session engine owns authoritative speech-readiness state. Add
`speech_readiness` to `SessionState` with `inactive`, `preparing`, and `ready`.
Existing recording prediction and independent verification keep their meaning.
The frontend renders readiness state but never schedules or plays the cue.

A readiness candidate contains the device generation, Start sequence, a unique
readiness token, a snapshot of the selected guard, and cue enablement. It exists
only for a successfully queued Start. A matching fresh confirmation changes it
from waiting-for-confirmation to warming, using a monotonic desktop deadline.
Repeated observations cannot move that deadline or schedule another tone.

The existing background timer can advance readiness without sleeping in IPC or
the command writer. The candidate commits to ready only when the same generation
and Start token remain current, camera eligibility is intact, and both observed
and predicted states are recording without a later capture command.

A later Stop, Pause, new Start intent, discard intent, loss of foreground/video
mode, unknown observation after confirmation, verification failure/timeout, delivery failure,
disconnect, engine replacement, or shutdown invalidates a pending candidate.
Failed command delivery cannot resurrect an invalidated readiness token. Resume
cannot revive a cancelled Start cue. Stale and superseded confirmations do not
create candidates. The later discard component will use the same invalidation
boundary before it requests Stop or performs any file operation.

A transient unknown recording state while Start is still awaiting confirmation
does not arm a timer or play a cue. It may continue waiting for fresh eligible
confirmation until the existing verification deadline. An unknown state after
confirmation cancels the candidate; later recovery alone does not revive it.

Readiness commit, command intent invalidation, and generation replacement must
be ordered against the same authoritative state. Event emission happens outside
the session lock. An internal ready event carries the generation, Start sequence,
and token; a new window's bootstrap only obtains the current state, not an
auditory replay event.

## Audio dispatch

Use one process-owned audio dispatcher, not one sound player per window. It
serializes tones and validates a readiness token immediately before starting
its playback. Pending readiness takes priority over queued optional success
tones; a failure already playing may finish first, after which the readiness
token is checked again. Stale queued cues are discarded.

The current Windows `Beep` backend can produce this tone through the default
audio device. It must run exclusively on the audio worker because the API is
synchronous. No capture/session/settings lock is held while a tone plays, and
capture commands never wait for the audio queue. Do not change system volume,
switch the user's output device, or require installing an audio package.

A cancellation ordered before the playback commit prevents the cue. A 100 ms
tone already committed may finish after a subsequent command; capture dispatch
still proceeds immediately. Audio failure produces a deduplicated ordinary
bilingual error and never marks playback as successful or retries a stale cue.

## Settings compatibility and integration

Missing new fields in old settings JSON receive the defaults above; an existing
language, destination, ADB path, and sound preference are preserved. Validation
rejects invalid readiness delay values before persistence. Windows receive the
saved settings through the current authoritative broadcast.

Keep readiness policy separate from file transfer, listing, updater work, and
generic result history. The backend lifecycle and audio behavior require no
new third-party dependency. Existing supported Windows/Android targets remain
the baseline; compilation alone does not expand physical compatibility claims.

## Acceptance and verification

1. Deterministic Rust clock tests prove: no cue before confirmation or deadline;
   one cue per eligible Start; no replay across windows or bootstrap; correct
   suppression after cancellation, stale generations, superseded results,
   disconnect, unknown state, queue/delivery failure, and rapid command sequences.
2. An audio-worker test holds playback before its commit, invalidates the token,
   and proves the cue is skipped. Another holds audio output while capture is
   dispatched and proves command delivery remains independent of audio.
3. Settings tests load pre-feature JSON, preserve existing values, persist both
   new fields, reject invalid delays, and check disable/re-enable behavior.
4. Svelte/controller tests cover the preparing/ready text, keyboard operation,
   both languages and RTL, independent sound settings, stable focus, and the
   continued absence of live speech regions.
5. Real EXE/WebView2 fixture acceptance covers delayed confirmation, cancellation
   before readiness, shared windows, restart persistence, and a held file job
   concurrent with recording. Fixture cue events do not certify audible routing
   or phone microphone readiness.
6. Run at least ten release-build Starts on the current phone with its internal
   microphone and the PC output routed to the user's headphones. Use a continuous
   audible reference reaching the phone and user-spoken marked words after the
   new cue. Preserve logs and inspect the decoded recordings. Accept only if the
   cue occurs once per eligible Start, no words spoken after it are truncated,
   and the cue does not precede the measured audio onset/ramp. A tone routed only
   to PC headphones must not be treated as a reference recorded by the phone.
7. Verify rapid Stop/Pause, connection loss, and opening another window do not
   cause a late cue. Verify phone accessibility coexistence in its actual
   pre-test state; do not enable or disable TalkBack merely to match older data.

## Remaining requested work

This cue makes the intended speak-now signal explicit. It does not reconstruct
speech that was never captured, alter Open Camera's phone beep, or remove the
pre-cue portion of the file. Eliminating unwanted leading silence is a separate
design decision requiring accurate audio/video trimming and preservation of
originals; it is not silently added to this change.

The user's broader request remains active and will receive subsequent focused
designs: checkbox-confirmed stop-and-delete of the current take; transient file
results and updated screen-reader names after rename; measured 500 MB/1 GB file
transfer improvements; installer-language handoff; bundled compatible ADB with
in-app wireless pairing/connection; compatible dependency/toolchain upgrades,
translation updates, and completing repository update/PR automation.

## References

- [Local measured evidence](../../research/2026-10-15-recording-startup-measurements.md).
- [Microsoft Beep behavior and synchronous API](https://learn.microsoft.com/en-us/windows/win32/api/utilapiset/nf-utilapiset-beep).
- Existing capture contracts: `docs/architecture.md`, `docs/testing/session.md`,
  and `docs/testing/observer.md`.
