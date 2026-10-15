# TakeDock recording startup measurements — 2026-10-15

Approved diagnostic experiment, not a product fix. Phone input: the existing internal microphone; no external phone microphone was attached. The user supplied an audible hum in trial 07 because PC playback was routed to headphones.

## Findings

Open Camera's Camera2 start sound is requested before MediaRecorder preparation and start. In the installed 1.56.2 release, this sequence was confirmed by inspecting upstream tag v1.56.2 (commit 0dd4cbe78872df2c6e4eb6cee3fb0d5637b0f52e) and by native Android logs. The existing TakeDock Observer also requires the pause control, which upstream shows after recorder start returns; changing the shutter label alone is not its complete recording test.

Ten trials have complete command/Observer timing metadata. Eleven test MP4s, including the manually hummed diagnostic clip, decoded without reported errors. No A/V synchronization defect is being alleged: these measurements concern startup and the silent beginning of the audio track.

| Metric | Samples | Minimum | Median | Maximum |
| --- | ---: | ---: | ---: | ---: |
| Phone shell marker before key dispatch → native recorder start | 10 | 537.47 ms | 624.72 ms | 706.17 ms |
| Start-sound playback proxy → native recorder start | 10 | 428.0 ms | 528.0 ms | 590.0 ms |
| MediaRecorder preparation alone | 10 | 141 ms | 154.0 ms | 277 ms |
| Recorder start → first encoded audio arriving at muxer | 9 | 216.0 ms | 224.0 ms | 235.0 ms |
| Recorder start → first encoded video arriving at muxer | 9 | 273.0 ms | 282.0 ms | 302.0 ms |
| Exact digital zeros at beginning of decoded audio | 11 | 320.0 ms | 320.0 ms | 320.0 ms |

## Per-recording evidence

| Trial | Command → start | Sound proxy → start | Prepare | Exact leading zeros | First audio above −80 dBFS |
| --- | ---: | ---: | ---: | ---: | ---: |
| trial-01 | 700.43 ms | 587.0 ms | 277 ms | 320.0 ms | 353.333 ms |
| trial-02 | 537.47 ms | 428.0 ms | 142 ms | 320.0 ms | 353.542 ms |
| trial-03 | 586.29 ms | 477.0 ms | 175 ms | 320.0 ms | 353.333 ms |
| trial-04 | 637.58 ms | 526.0 ms | 146 ms | 320.0 ms | 353.833 ms |
| trial-05 | 625.42 ms | 530.0 ms | 147 ms | 320.0 ms | 356.021 ms |
| trial-06 | 675.17 ms | 585.0 ms | 161 ms | 320.0 ms | 353.333 ms |
| trial-07 | Unavailable | 451.0 ms | 152 ms | 320.0 ms | 346.458 ms |
| trial-08 | 616.69 ms | 510.0 ms | 185 ms | 320.0 ms | 350.625 ms |
| trial-09 | 706.17 ms | 590.0 ms | 158 ms | 320.0 ms | 353.375 ms |
| trial-10 | 624.02 ms | 530.0 ms | 150 ms | 320.0 ms | 348.146 ms |
| trial-11 | 606.18 ms | 520.0 ms | 141 ms | 320.0 ms | 346.625 ms |

## Interpretation and limits

- Native MediaRecorder start is an API event, not the instant of the first physical exposure or microphone sample. Muxer arrival is another later event and includes encoding and buffering. These timestamps must not be presented as interchangeable.
- The sound timestamp is the app process's AudioTrack setVolume event associated with its start sound. It is a playback proxy, not an independent acoustic measurement of when the user heard the beep. The exact physical beep-to-capture interval remains unmeasured.
- All decoded tracks begin with 320 ms of exact floating-point PCM zeros. In the manually hummed trial, sound above −80 dBFS first appears at about 346 ms and grows over the next few hundred milliseconds. The hum has a fundamental near 160 Hz. The hum's physical onset was not independently timestamped.
- AOSP AudioSource contains a startup suppression/fade mechanism, but Samsung's exact framework behavior was not directly inspected. It is therefore a candidate explanation, not a proven attribution of the observed 320 ms zeros to a particular vendor implementation.
- Trial 01 used a newly opened persistent shell. The remaining fully timed trials warmed it before dispatch. Phone-side marker timing excludes most desktop transport overhead; host-to-Observer timing is not shipping frontend activation latency.
- Trial 07's phone clip, logs, and ffprobe data are usable. Its optional PC microphone reference did not produce usable audio and timed out during cleanup, preventing the final metadata write. It is excluded from complete timing distributions. The stalled process was explicitly stopped. PC tones reached headphones and are not used as an acoustic reference.
- Every fully timed trial confirmed idle before starting, issued one start and one stop with no automatic retry, and recorded unchanged enabled accessibility services during that trial. No claim is made that user settings remain identical after later user activity or interruptions.
- The test conditions were Android 16, Open Camera 1.56.2, Camera2, H.264 1920×1080 at 30 fps and mono AAC at 48 kHz. This does not certify external-microphone behavior or other devices.

## Sources and local evidence

- [Open Camera Preview.java at v1.56.2](https://sourceforge.net/p/opencamera/code/ci/v1.56.2/tree/app/src/main/java/net/sourceforge/opencamera/preview/Preview.java): pre-prepare hook at line 6093; prepare at 6167; start at 6187; started callback at 6302.
- [Open Camera CameraController2.java at v1.56.2](https://sourceforge.net/p/opencamera/code/ci/v1.56.2/tree/app/src/main/java/net/sourceforge/opencamera/cameracontroller/CameraController2.java): start-sound request at line 7584.
- [AOSP Android 16 AudioSource.cpp](https://android.googlesource.com/platform/frameworks/av/+/android16-release/media/libstagefright/AudioSource.cpp) and [AudioSource.h](https://android.googlesource.com/platform/frameworks/av/+/android16-release/media/libstagefright/include/media/stagefright/AudioSource.h).
- summary.json contains sanitized results and local copy SHA-256 hashes. Individual trial directories contain private raw logs and test clips; do not publish them.

## Recommendation

Do not treat Open Camera's existing start beep as a reliable speak-now cue. Design and validate a separate readiness cue against the recorded startup and initial audio suppression, with cancellation on stop, pause, disconnect, or replacement of the current take. Decide separately whether the desktop copy should omit the preparation preamble: keeping the first spoken syllable and eliminating leading silence are distinct acceptance conditions. Any trimming would need explicit design, sample-accurate validation and preservation of originals. No product implementation or dependency changes were made by this probe.

## Fixture cleanup and final verification

All eleven phone test recordings were matched to successfully decoded local copies by SHA-256 and unchanged file stat before removal. Their absence on the phone and retention of eleven local copies were verified after cleanup. The phone recording-running property was 0. Product Git files remain unchanged.
