# Desktop interface evidence

Svelte controller/component checks cover immediate start/pause/resume/stop
dispatch while previous promises remain pending, shared literal Rust protocol
cases, stale-state rejection, native control eligibility, retained focus,
single-only rename, batch confirmation, refresh/rename selection continuity,
protected files, Arabic RTL, optional success sound, subscription cleanup,
queued job visibility, and typing/repeat-safe shortcuts.

`scripts/check-ui.ps1` runs Svelte checking with warnings treated as failures,
Vitest, and a production Vite build. On 2026-10-01: zero errors/warnings and ten
UI tests passed. The first real Windows release executable built with Rust
1.98.1 and Tauri 2.12.1; its bundled Observer/helper resources resolved and it
recognized the connected Android 16 phone without a manually entered ADB path.

The real WebView2 Windows accessibility tree exposed named native navigation
buttons, headings, and ordinary connection/camera text. Recording controls were
absent while Open Camera was closed. NVDA was running. The Computer Use helper's
window screenshot capture timed out and indexed input reported unavailable
geometry; text-only accessibility inspection succeeded. Consequently this probe
does not certify actual NVDA speech, keyboard traversal, or native folder selection.
Those remain packaged acceptance checks alongside the separate Desktop E2E suite.

The user later reported successful installed-app screen-reader, capture and transfer
acceptance and took over further physical testing. Native `<progress>` exposes the
actual changing transfer percentage; reader speech/beeps follow reader preferences.
TakeDock supplies no live-region or forced percentage announcements.

Final review regression coverage now includes rejected capture IPC recovery without
an invented backend sequence, delayed start/pause/resume evidence, expired-session
selections/delete confirmations, cleared retired-session activity, reversed shared
save responses, immutable approved update offers, and focus retained on the recording
heading when capture eligibility disappears. Monotonic snapshot revisions reject
reordered broadcasts and bootstrap responses independently of accepted command
numbers. All 33 frontend and 59 Windows Rust/
process/helper tests pass, with zero Svelte diagnostics and strict desktop Clippy.

Sound uses a background Windows audio thread: optional rising two-note success,
falling two-note failure, with success preference enforced in Rust as well as the
frontend. It does not create live regions, alert/status roles, or focus changes.
Audio-device output remains part of packaged acceptance.

An additional negative transport integration test observed a closed shell
incorrectly leaving capture controls eligible. The repair latches the broken
capture channel until reconnect, clears its pending guesses, and retains the
authorized phone lease for file operations. Further Observer events cannot
reenable a dead command channel. The failure was observed before the repair,
and the full 43-test Rust/process suite and strict desktop Clippy passed.
