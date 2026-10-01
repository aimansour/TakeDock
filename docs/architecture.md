# Architecture

The Svelte 5 frontend uses native HTML controls and typed Tauri commands/events.
It predicts recording transitions synchronously before issuing asynchronous IPC.
Start/Stop share a DOM button; Pause/Resume share another. An inapplicable action
is removed. Results are ordinary text, never live regions. Navigation and editing
shortcuts respect composition, text/select inputs and key repeats.

The Rust session owns one authorized phone and a monotonically increasing generation.
It watches the ADB device stream, keeps a capture shell open and queues bounded
commands with sequence markers. It never automatically repeats a toggle. Every
request carries the intended generation; late results cannot act on a replacement
session. A failed shell latches capture unavailable until reconnect, while retaining
a valid file lease. File operations never require Open Camera foreground.

The Android Observer targets itself and uses
`getUiAutomation(FLAG_DONT_SUPPRESS_ACCESSIBILITY_SERVICES)` plus
`setOnAccessibilityEventListener`. It produces an initial snapshot and coalesced,
event-triggered snapshots. It does not poll the phone, dump XML, run `uiautomator
events`, or disable screen-reader services. Installed Open Camera resource IDs and
localized labels identify video mode/state. Unknown layouts fail closed.
Versioned messages carry session UUID, sequence and Android uptime.

Verification is an independent Rust state machine. Session/time/sequence gates
reject stale observations; coalesced newer evidence can supersede intermediate
states. Local deadlines report uncertainty without delaying dispatch. No guessed
state is restored after contradictory evidence. Teardown removes listeners and
finishes instrumentation through Android's framework-owned cleanup.

FileService has a separate worker/queue and cancellation. A small independently
authored Rust Android helper validates a fixed no-symlink directory and source
device/inode/size/mtime/ctime, conservatively checks finalized containers, and uses
atomic no-replace mutations and precommit recovery journals. Android private-storage
Bionic locks serialize helpers because shared-storage FUSE does not provide flock.
Copies stream to unique desktop partials, sync, revalidate source, hash both sides,
and publish without overwrite. Move commits deletion only afterwards. Identities
cross JavaScript as opaque text to preserve nanosecond precision.

Settings persist in the per-user profile. Tauri capabilities expose event subscription
and native dialogs; frontend filesystem/shell/general updater permissions are absent.
Narrow Rust update commands retain the checked offer and require explicit activation.
The official updater verifies signatures/version and shuts down workers before exit.

The official single-instance plugin activates the last focused window on relaunch.
Named windows share the same Runtime/Engine/FileService/settings. Narrow window
commands create/rename windows without broad frontend window permissions. Settings
broadcasts and bounded job snapshots initialize every window; backend result
feedback sounds once, independently of frontend window count. Startup update
checking has one process owner. Empty destinations migrate to Videos/TakeDock.

The bounded finalized-container reader also supplies milliseconds from MP4 mvhd
timescales/version 0/1 or WebM Info Duration/TimestampScale. Unavailable durations
remain explicit. Ordering uses full source modification nanoseconds in Rust.
Native labelled progress exposes transfer percentages to the reader; unknown
listing/verification durations use indeterminate progress, without live regions.

CI runs a separate fake ADB process against the actual release EXE, WebView2 and
Rust IPC. It does not inject test behavior into the product. Four mandatory cases
feed package hash verification. Publishing requires every job successful, matching
versions, persistent signing identities, package hashes and verified uploaded bytes.
