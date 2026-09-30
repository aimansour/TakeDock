# Session engine verification

The desktop engine has independent device-tracking, persistent shell writer,
shell acknowledgement reader, Observer reader, and verification workers.
Dispatch validates current eligibility, predicts state, and enqueues one key
event without waiting for shell acknowledgement or observation. A bounded
queue rejects overload rather than blocking the caller. Toggles are never
automatically retried.

Every connection and engine replacement obtains a monotonically increasing
generation. Desktop actions carry their expected generation; stale actions,
observations, acknowledgements, and file leases cannot target another session.
Observer setup failure leaves the authorized phone available to file jobs.
Shutdown cancels installation/setup and joins lifecycle workers before return.

Verification confirms observed states, marks coalesced older intent superseded,
and reports expired missing evidence as unconfirmed. The default background
deadline is 10 seconds, configurable from 2 to 120 seconds. This deadline never
delays later commands. Unconfirmed state must not be replaced by an old guessed
state: controls require fresh evidence or a user-requested session reconnect.

Run the core and real-process fixture tests:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The fixture is a separate executable using ADB's actual argv, length-framed
device stream, persistent stdin/stdout shell, and instrumentation status format.
It has no route to a real phone. Per-test directories and observation barriers
prove shell commands arrive before delayed verification is released. It also
tests multiple phones, stale UI intent, failed Observer installation, shutdown
during installation, and engine-generation replacement.

## Device evidence: 2026-10-01

The Rust engine performed start/pause/resume/stop with the real Android 16 phone
and enabled Samsung TalkBack. For one debug-build run, dispatch returned after
29, 73, 28, and 88 microseconds respectively. Send-to-observed-state times were
730, 126, 200, and 877 ms. Dispatch numbers measure Rust validation/enqueue with
a minimal test sink, not frontend IPC, shell delivery, or production UI latency.
Record production timing distributions separately during packaged acceptance.

The harness allowed 800 ms before stop because Open Camera ignores stop within
500 ms of starting/resuming. The shipping dispatcher has no such wait. Observer
shutdown left no Observer process. Twenty core/integration tests passed after
observed RED baselines; strict Clippy and formatting checks passed.

To repeat real-device acceptance, deliberately open Open Camera in video mode
and run the following. This creates a test video:

```powershell
cargo run -p takedock --example device-recording -- PATH_TO_OBSERVER_APK PATH_TO_ADB
```
