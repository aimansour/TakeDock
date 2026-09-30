# Observer verification

The instrumentation targets `app.takedock.observer`, never Open Camera.
It obtains UiAutomation with `FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES` only.
State reading runs on a dedicated HandlerThread after accessibility events;
the callback coalesces event bursts. There is one initial snapshot and no
periodic query, XML dump, or accessibility-service suppression.

Open Camera resource identifiers identify the two relevant controls. Their
expected descriptions are read from the installed app's localized resources.
Unknown or incomplete evidence remains unknown. No camera source code or
translation tables are bundled.

Build/check with Java 17, SDK 37.0, and the checksum-pinned Gradle wrapper:

```powershell
./scripts/build-observer.ps1
```

Protocol lines are instrumentation status entries beginning
`INSTRUMENTATION_STATUS: takedock=` followed by single-line JSON. Version 1
contains a session token and `ready`, `state`, or `error` type. State includes
sequence, Android uptime event timestamp, foreground/video-mode eligibility,
and `unknown`, `idle`, `recording`, or `paused`.

The session is stopped through a package-scoped STOP broadcast with its token;
the receiver requires Android's DUMP permission, held by the ADB shell.
The runner unregisters callbacks/receiver and ends instrumentation so Android
releases its connection. The desktop also force-stops only this Observer
package if graceful shutdown does not complete.

Acceptance evidence must include actual start/pause/resume/stop events,
foreground loss, unchanged enabled accessibility services, the screen reader
remaining bound, and no Observer process after shutdown. Unit tests alone do
not establish these properties.

## Device evidence: 2026-10-01

- Windows 10 host, Android 16/API 36 phone, Open Camera 1.56.2, Samsung TalkBack
  already enabled before testing. Built against SDK 37.0 using AGP 9.4.1.
- Seven state-reader tests passed after an observed failing baseline.
- The event stream observed idle → recording → paused → recording → idle.
  One persistent ADB shell delivered all four key events. Approximate desktop
  send-to-observed-state timings for one run: 763, 114, 145, and 990 ms.
  These include application/device/event processing and local log detection;
  they are not TakeDock UI dispatch benchmarks or timing distributions.
- Foreground loss emitted an ineligible unknown state. TalkBack's enabled
  setting and bound service remained present throughout. Touch-exploration
  focus was visible during camera use. This is coexistence evidence, not a
  complete audit of every TalkBack feature.
- A token-authenticated STOP broadcast ended instrumentation with exit code
  zero and no remaining Observer process. Accessibility settings were not
  changed. Test-created videos are tracked privately for later file tests.
- Open Camera ignores stop requests within 500 ms of starting/resuming its
  recorder (`Preview.takePicturePressed` at the tested upstream tag). A very
  rapid test reproduced this. TakeDock must still dispatch immediately,
  never retry a toggle automatically, and report unconfirmed execution through
  its independent verifier. The normal-transition test allowed camera time
  before stop; that test allowance is not a desktop dispatch delay.
