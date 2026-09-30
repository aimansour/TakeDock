# Video operation evidence

The Rust file worker has its own bounded queue. Capture dispatch never waits
for a listing, transfer, hash, cancellation, or file result. Every request binds
to an explicitly selected device lease and generation.

`cargo test --workspace` exercises quoted Unicode/apostrophe names, binary NUL
listing, opaque identities (avoiding JavaScript timestamp precision loss),
Windows path/device-name rejection, changed sources, atomic rename collisions,
journal recovery, incomplete MP4/WebM containers, batch results, transfer/hash
failures, cancellation during streaming and held hashing, and concurrent capture.
The external executable fixture shares the MIT native helper's filesystem code;
it never forwards to real ADB. Linux-only symlink/newline cases run in Linux CI.

On 2026-10-01, the Android 16 phone passed the native stat/hash/rename/delete
probe and the `device-files` acceptance helper with Open Camera force-stopped.
Exactly two recordings previously created for this project's tests were selected
through a private manifest. Batch copies were size/identity/SHA-256 verified;
an existing destination was retained; one recording was renamed to a Unicode
name and moved; the other was explicitly deleted. Personal videos were excluded.
The copied test recordings remain in a task-owned acceptance directory.

The real phone exposed two portability failures during development: Rust's
standard file lock does not implement Android, and Android shared-storage FUSE
does not implement `flock`. The helper now uses Bionic `flock` on a shell-owned,
no-symlink private-storage lock; recovery journals remain beside the videos.
Atomic no-replace rename (`renameat2`) passed on this phone.

Supported containers are MP4/3GP/M4V/MOV and WebM/Matroska. A positive finalized
duration and bounded container metadata are required; unfinished or damaged
containers remain visible but protected. This is a conservative safeguard for
Android MediaRecorder outputs, not a full codec/playability validator. No remote
polling is involved. AOSP MPEG4Writer creates `moov` on finalization; WebmWriter
fills the final Duration on stop:
[MPEG4Writer](https://android.googlesource.com/platform/frameworks/av/+/refs/heads/main/media/libstagefright/MPEG4Writer.cpp),
[WebmWriter](https://android.googlesource.com/platform/frameworks/av/+/refs/heads/main/media/libstagefright/webm/WebmWriter.cpp).

Copies use unique `.partial` files, fsync, source identity checks, SHA-256, and
atomic no-replace publication. A move reaches its destructive commit only after
publication. Cancellation/disconnection before that commit retains the original;
once deletion has actually committed it cannot be undone by a later cancellation
or lost acknowledgement. The verified desktop copy remains available. A killed
helper's precommit journal restores a claimed original on its next invocation.
Batch results name each completed, failed, cancelled, or skipped entry; independent
failures do not prevent subsequent entries. Media scan broadcasts follow mutations;
an indexing warning does not misreport an already completed mutation as undone.

Use `scripts/test-device-files.ps1` only with a manifest of dedicated test
recordings and an empty task-owned destination. It deliberately renames, moves,
and deletes those selected phone fixtures.
