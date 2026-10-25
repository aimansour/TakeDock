# Transfer speed: current repository survey and physical experiments

Research began 2026-10-24; follow-up continues 2026-10-25. Scope: reduce complete large-file transfer time on
the existing Windows/Android wireless path. Repository popularity is a selection
aid; only source inspection and measurements determine applicability.

The survey found useful mechanisms and tested the applicable candidates, but
**no new production change demonstrated a large, repeatable complete-file
speedup**. The original transfer worker remains unchanged. Burst Mode showed
promising native 500 MB observations followed by a slow ordinary 1 GB copy;
the later alternatives below did not establish a consistent improvement.

## Source selection and reproducibility

GitHub metadata showed all eight repositories active and unarchived. Seven have
over 40,000 stars; KDE Connect contributes an established Android/LAN design.
Counts below are rounded snapshots, not performance evidence. The eight repository
snapshots link to exact inspected revisions. The first nine pinned source bodies were fetched
again and checked equal to the bodies initially reviewed on their default branch.
The private source manifest records exact revisions and retrieval metadata.

| Repository          | Stars, approximately | Latest repository push at retrieval | Inspected revision                         |
| ------------------- | -------------------: | ----------------------------------- | ------------------------------------------ |
| LocalSend           |               95,000 | 2026-10-21                          | `6e37efd5fe750299758b6778d2b150b31af3b95d` |
| Syncthing           |               90,000 | 2026-10-24                          | `a4a2da477f32d29eda98cc6eb08554dfe5078875` |
| rclone              |               61,000 | 2026-10-24                          | `81e7372f6fbedfa80592e9a564593d63a8dc32f3` |
| scrcpy              |              153,000 | 2026-10-24                          | `a60891aea193d92e7e5c3942700eca63f9d19a5f` |
| croc                |               41,000 | 2026-10-22                          | `9c28c3bd3ddaf106ea32192ef9bc807e999b1367` |
| copyparty           |               47,000 | 2026-10-21                          | `1af7efeca858dcfa29f797d7ae93f22eb2fdecf0` |
| RustDesk            |              127,000 | 2026-10-24                          | `1297da6d186e58d9096be24072ae3998cdbbf8fe` |
| KDE Connect Android |                1,500 | 2026-10-24                          | `353361ed55ab5ccb444c9d00a74b9bab97eff43e` |

Push timestamps describe repository activity; they are not asserted to be the
last modification date of an inspected file or a stable release date.

## Findings and applicability

### LocalSend: flow control and bounded pipelines

The inspected HTTP client explicitly chooses HTTP/1.1 because HTTP/2 flow control
can constrain bulk upload throughput. It bypasses system proxies for local peers
and retains its certificate checks. TakeDock currently has no HTTP transfer path,
so its HTTP version setting cannot be copied literally. The transferable lesson
is to examine the actual transport window before adding application threads.
[Client source](https://github.com/localsend/localsend/blob/6e37efd5fe750299758b6778d2b150b31af3b95d/packages/core/src/http/client/mod.rs)

Its file producer uses 512 KiB reads and a four-item channel, bounding queued
read-ahead to 2 MiB per stream. Its receiving writer coalesces writes into a
512 KiB buffer and separates socket draining from file writing through a bounded
channel. These are useful architectural references, but the earlier TakeDock
256 KiB write-buffer candidate did not pass its physical acceptance gate.
Changing a number to match another application does not establish a gain here.
[Read pipeline](https://github.com/localsend/localsend/blob/6e37efd5fe750299758b6778d2b150b31af3b95d/packages/core/src/model/transfer.rs),
[receive pipeline](https://github.com/localsend/localsend/blob/6e37efd5fe750299758b6778d2b150b31af3b95d/packages/core/src/http/server/common/save.rs)

### Syncthing: concurrent block requests with a byte budget

Syncthing launches block requests concurrently while a semaphore limits the
aggregate pending bytes. Each received block is checked before offset-based
writing, and write concurrency is bounded separately. Its tuning guide also
distinguishes outstanding request bytes from the number of files and connections.
The applicable idea is bounded concurrency inside one large file, with exact
coverage and validation. Increasing TakeDock's file queue concurrency alone
would not implement this design.
[Block puller source](https://github.com/syncthing/syncthing/blob/a4a2da477f32d29eda98cc6eb08554dfe5078875/lib/model/folder_sendrecv.go),
[official tuning guide](https://docs.syncthing.net/users/tuning.html)

### rclone: parallel ranges require source and destination support

rclone's multithreaded copy opens bounded source ranges and writes them at the
matching destination offsets. It caps concurrent work and checks each chunk's
exact length. Its documented defaults include four streams for eligible large
files, a 256 MiB cutoff and a 64 MiB chunk size where the backend has no natural
chunk size. It deliberately avoids this path for unsupported backends and some
local copies. These numbers are workload choices, not universal optimums.
[Range-copy implementation](https://github.com/rclone/rclone/blob/81e7372f6fbedfa80592e9a564593d63a8dc32f3/fs/operations/multithread.go),
[configuration documentation](https://github.com/rclone/rclone/blob/81e7372f6fbedfa80592e9a564593d63a8dc32f3/docs/content/docs.md)

TakeDock's existing helper holds an exclusive root lock around a complete read.
Launching four unchanged helper reads would serialize them. A production range
protocol would require its own reviewed amendment covering lock ownership,
source identity, exact non-overlapping coverage, cancellation, readback and
publication. The preliminary channel experiment below tests whether that extra
complexity has a useful transport opportunity before designing it.

### croc: bounded chunk ownership and concurrent offset I/O

croc assigns file offsets to workers through a bounded range queue. A claimed
chunk belongs to one worker even when connections join later. It reads and writes
by offset; the receiving file write occurs outside the shared state mutex.
This is a useful example for preventing a nominally parallel implementation from
serializing all disk I/O under one lock. It does not establish that four logical
streams sharing one ADB/TLS connection will outperform one stream.
[Chunk scheduler](https://github.com/schollz/croc/blob/9c28c3bd3ddaf106ea32192ef9bc807e999b1367/src/croc/chunk_queue.go),
[data path and chunk bounds](https://github.com/schollz/croc/blob/9c28c3bd3ddaf106ea32192ef9bc807e999b1367/src/croc/croc.go)

### scrcpy: separate bulk and control sockets

scrcpy uses separate ADB-tunnel sockets for media and control. It applies
TCP_NODELAY to the control socket and explicitly explains that setting it on its
receive-only media sockets is ineffective. Therefore enabling this option on a
TakeDock receiving socket is not a supported bulk-throughput fix. Its socket
architecture remains a reference if a different data path is justified by a
measurement; no scrcpy binary or phone server was installed for this research.
[Socket setup](https://github.com/Genymobile/scrcpy/blob/a60891aea193d92e7e5c3942700eca63f9d19a5f/app/src/server.c)

### copyparty: parallel chunks and kernel-assisted sending

copyparty's resumable upload design can send multiple chunks concurrently. Its
download utility also has separate buffered and kernel sendfile implementations.
The source illustrates eliminating unnecessary data copies when the OS and
socket path allow it. TakeDock currently crosses an Android helper pipe, ADB's
encrypted transport and a Windows child stdout pipe. A kernel sendfile call is
not a replacement for that complete path. Testing ADB's native file-sync receive
path is a smaller way to investigate avoidable helper/pipe overhead.
[Upload architecture and performance documentation](https://github.com/9001/copyparty/blob/1af7efeca858dcfa29f797d7ae93f22eb2fdecf0/README.md),
[sendfile implementations](https://github.com/9001/copyparty/blob/1af7efeca858dcfa29f797d7ae93f22eb2fdecf0/copyparty/util.py)

### RustDesk: sufficiently filled blocks and selective compression

RustDesk fills a 128 KiB transfer block across short reads. It uses a compressed
block only when the result is smaller, and skips compression for some already
compressed extensions. This avoids assuming every input becomes faster when
compressed. TakeDock's test video has random padding, and actual encoded video
also needs measurement before compression is called beneficial. No compression
of a zero source is accepted as evidence for video speed.
[File-transfer reader](https://github.com/rustdesk/rustdesk/blob/1297da6d186e58d9096be24072ae3998cdbbf8fe/libs/base/src/fs.rs)

### KDE Connect: a dedicated trusted payload connection

KDE Connect sends control metadata on one connection and establishes a separate
trusted TLS socket for the payload. The sender copies with a 256 KiB buffer and
throttles progress notifications. This demonstrates that direct LAN transfer can
retain trust and encryption; bypassing ADB would nevertheless require a new
authenticated phone-side service and lifecycle, rather than only a desktop
buffer setting. No such service has been added or exposed.
[LAN payload implementation](https://github.com/KDE/kdeconnect-android/blob/353361ed55ab5ccb444c9d00a74b9bab97eff43e/src/main/java/org/kde/kdeconnect/backends/lan/LanLink.kt)

## Physical experiments

All new probes use the actual paired phone and real SDK. They are exploratory
measurements, not the original plan's complete release acceptance matrix.
Channel tests use uncompressed `/dev/zero`; file tests use the owned video master
and its full decoded backup. No personal media or phone source is moved/deleted.

### ADB send window

The official experimental Burst setting addresses ADB's per-stream packet
acknowledgement limit. Actual native 500 MB Copy was faster in two temporary
server comparisons, but an ordinary default-server 1 GB follow-up still took
280.38 seconds. Its streaming phase alone took 271.00 seconds. A control that
changed only client flags on the same running server was inconsistent.
The [separate measurement report](2026-10-24-wireless-burst-mode.md) retains all
these observations, including the disappointing follow-up.
[AOSP mechanism](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/docs/dev/delayed_ack.md)

### Four logical streams on the same existing ADB connection

The fixed aggregate payload was 128 MiB for both methods. One stream read all
bytes; four streams each read 32 MiB concurrently. The same C# drain/hash loop,
running default server and client Burst setting were used. Each stream's exact
byte count and full digest passed. An 8 MiB warm-up per method was excluded.

| Pair | Order      | One stream, seconds | Four streams, seconds |
| ---: | ---------- | ------------------: | --------------------: |
|    1 | One → Four |               36.35 |                 33.46 |
|    2 | Four → One |                6.99 |                 18.49 |
|    3 | One → Four |                6.98 |                 34.78 |

Four logical streams failed to improve two pairs and cannot justify a production
parallel-range change. The 6.98–36.35 second single-stream range also shows why
one favorable wireless observation cannot establish a reliable gain. These
synthetic observations are not native application file times. Logical streams
still share a physical ADB/TLS transport; this probe does not measure four
independent physical network connections.

The original default server PID, pairing-key digest and authorized phone
connection were preserved. The phone returned Home with its screen asleep.

### Native SDK file-sync receive path

Two alternating comparisons completed: the unchanged identity-bound helper read
versus uncompressed SDK `pull -Z` of the same owned 500,000,000-byte source. Both
used the same already-running default Burst-enabled server and a fresh local
output directory per operation. Complete time includes durable flushing, exact
stored size, full stored-file readback, post-copy source identity and full phone
digest. These are exploratory backend totals, not native UI measurements; the
helper-pipe reader is a probe and not a replacement frozen application build.

| Pair | Order             | Helper stream, seconds | SDK sync stream, seconds | Helper complete, seconds | SDK sync complete, seconds |
| ---: | ----------------- | ---------------------: | -----------------------: | -----------------------: | -------------------------: |
|    1 | Helper → SDK sync |                 114.66 |                   127.16 |                   118.53 |                     134.12 |
|    2 | SDK sync → Helper |                  68.83 |                    85.05 |                    72.69 |                      88.80 |

All four outputs and original source hashes passed. SDK sync was slower in both
observed comparisons, so it is not selected as a speed candidate. The original
profile bytes, unrelated phone files, accessibility state, shared server process,
pairing key and authorized connection were preserved; the phone returned
Home/sleep. One preparation attempt stopped before timing because a private
benchmark helper was not exported; its invalid zero-run report was retained.
[Official pull options](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/docs/user/adb.1.md),
[Android file-sync sender](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/daemon/file_sync_service.cpp)

### Direct LAN headroom diagnostic

A separate diagnostic compared the existing encrypted ADB channel with a
one-client direct TCP listener. The latter sent only a bounded zero generator;
it had no file source, executable command input or persistent service. It bound
the already-authorized phone's own Wi-Fi address, used a fresh high port and
closed after one connection. This direct diagnostic was **unencrypted** and is
not a proposed production transfer protocol.

Both methods transferred exactly 64 MiB per measured observation, with full
byte-count and SHA-256 checks. One 8 MiB warm-up per method was excluded.

| Pair | Order        | Existing ADB, seconds | Direct zero-only TCP, seconds |
| ---: | ------------ | --------------------: | ----------------------------: |
|    1 | ADB → Direct |                 17.81 |                          4.76 |
|    2 | Direct → ADB |                 16.23 |                          5.43 |

The repeatable difference identifies an opportunity outside the current ADB data
path on this network. It does not isolate whether encryption, framing, pipe
behavior or another part of that path causes the difference. A separate
authenticated TLS experiment on an actual owned video is required before
selecting a new product transport. All one-shot listeners closed; the existing
paired connection and original server/key were retained, and the phone returned
Home/sleep. One earlier preparation attempt stopped with no transfers because
Toybox's timeout help command exits 125 on this build; a bounded no-op then
verified the command correctly.

### Authenticated direct TLS spike

The first complete spike used a disposable Java/Android helper, compiled outside the application
with portable official JDK and D8 artifacts. It accepts one TLS client, requires
a fresh one-use token sent through the existing authenticated ADB control channel,
and serves only the bounded zero source or an owned benchmark video. The desktop
pins the temporary certificate. Negative credential checks precede real copies.
Actual file comparisons include durable output, full stored-file readback and
post-copy identity/full digest checks through the unchanged file helper. The
temporary phone helper and private key were removed after the session. This
research spike changed no application transport or installed app.
[Official D8 distribution and invocation](https://r8.googlesource.com/r8/+/refs/heads/main/README.md)

Wrong certificate and wrong one-use token checks both rejected the client with
zero file payload. The positive 64 MiB zero copy took 10.10 seconds and passed
full stored-file hashing. The connection negotiated TLS 1.3 with
TLS_AES_256_GCM_SHA384. The four owned 500 MB file observations were:

| Pair | Order               | Helper complete, seconds | Direct TLS complete, seconds |
| ---: | ------------------- | -----------------------: | ---------------------------: |
|    1 | Helper → Direct TLS |                   122.11 |                       119.62 |
|    2 | Direct TLS → Helper |                    75.32 |                       139.40 |

Every output and source digest/identity check passed. The unrelated-file
preservation assertion also passed. However, the final accessibility snapshot
changed from one enabled phone service to none, and the stage was marked
**invalid**. The script did not restore or overwrite those settings, and the
cause of that change is not established. The final desktop-profile equality
assertion was not reached. The observations remain available but do not support
a controlled speed claim. Even their unqualified raw totals show no consistent
speedup. Existing server, pairing and connection were preserved at cleanup;
the owned helper/key were removed and the phone returned Home/sleep.

### Sender socket configuration follow-up

The first disposable Java sender used the default Nagle setting. The official
reqwest client, used by LocalSend's sending path, enables TCP_NODELAY by default.
This is a sender-side configuration opportunity for the disposable direct path.
AOSP's Wi-Fi ADB listener already disables Nagle on accepted TLS sockets, so it
is not a missing setting to add to the existing ADB transport.
[Reqwest 0.13.4 socket configuration](https://github.com/seanmonstar/reqwest/blob/11489b34eda6d32b15ad4033e62beba2ee401350/src/async_impl/client.rs),
[AOSP Wi-Fi listener](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/daemon/adb_wifi.cpp)

A fresh probe alternated only the phone sender's TCP_NODELAY value while keeping
the TLS implementation, owned file, receiver, byte checks and complete readback
the same. The server reported and verified the effective socket value through its
authenticated ADB control channel. Both credential rejection checks passed before
file payload. The stage completed with all four file outputs verified and current
profile, unrelated files, accessibility, shared server, pairing and connection
preserved. Its temporary helper/key were removed and the phone returned Home/sleep.

| Pair | Order    | Sender TCP_NODELAY off, seconds | Sender TCP_NODELAY on, seconds |
| ---: | -------- | ------------------------------: | -----------------------------: |
|    1 | Off → On |                           84.80 |                          67.11 |
|    2 | On → Off |                           74.90 |                          75.45 |

The second pair did not improve. The first favorable observation therefore does
not establish a repeatable gain, and this configuration is not selected as a
production speed improvement. Requested flag metadata uses PowerShell's
`IsPresent` representation in the raw JSON; the effective Java socket receipts
independently verify which setting each operation used. This completed follow-up
does not replace or reclassify the earlier invalid stage.

When the phone became unavailable between
sessions, enabling its Wi-Fi/wireless debugging allowed the existing trusted
identity to reconnect through discovery with no new pairing code.

### Short screen-state control

Three alternating 8 MiB pairs used the same existing encrypted ADB channel and
reader, changing only the requested interactive screen state. Raw wakefulness
was checked before and after every observation. Noninteractive observations
reported Samsung's normal Dozing state; interactive observations reported Awake.
No screen timeout, accessibility setting, Wi-Fi setting or camera application
was changed. Each exact length and full digest passed.

| Pair | Order                  | Noninteractive, seconds | Awake, seconds |
| ---: | ---------------------- | ----------------------: | -------------: |
|    1 | Noninteractive → Awake |                   0.972 |          1.482 |
|    2 | Awake → Noninteractive |                   1.131 |          1.376 |
|    3 | Noninteractive → Awake |                   1.379 |          1.669 |

Waking the screen did not improve any pair. These are short channel observations,
not sustained full-file or thermal measurements, so they do not establish all
power-management behavior. They provide no reason to keep the screen awake for
speed. Accessibility, existing server, pairing and connection remained unchanged,
and the phone returned Home/sleep. An initial zero-run preparation expected only
Asleep and rejected the phone's actual Dozing state; its invalid record was kept
before defining the correct noninteractive states for the fresh comparison.

### Existing encrypted ADB through a direct host socket

This probe retained the same SDK server, paired transport, phone command and TLS
connection. It compared the SDK CLI/stdout bridge with reading the official ADB
smart-socket service directly on loopback. The same host drain/hash logic checked
64 MiB per measured operation; one 8 MiB warm-up per method was excluded.

| Pair | Order        | SDK CLI/pipe, seconds | Direct host socket, seconds |
| ---: | ------------ | --------------------: | --------------------------: |
|    1 | CLI → Socket |                 15.32 |                       15.20 |
|    2 | Socket → CLI |                 12.75 |                       17.12 |
|    3 | CLI → Socket |                 11.09 |                        9.16 |

The direct socket did not improve every pair and did not establish a material,
repeatable gain. It is not selected as a new embedded ADB client. Every exact byte
count and full digest passed; the original server, pairing and phone connection
were preserved. No phone file or setting was changed, and the phone returned
Home/sleep.
[Official ADB host/device services](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/SERVICES.TXT)

The official SDK release notes list 37.0.1 as the current revision at retrieval,
matching the tested installation. They also document that this revision uses
libadbmdns and removes the old openscreen backend. Old openscreen or USB-only
environment suggestions are not applicable wireless speed fixes here.
[SDK release notes](https://developer.android.com/tools/releases/platform-tools)

## Decision boundary

The private evidence index preserves completed and invalid reports separately.
One initial zero-run failed preparation's source snapshot was overwritten by a
later source-copy step that selected a directory using its changed modification
time. A provenance correction identifies the misplaced files; that original
failed-attempt source/build snapshot is unavailable. The completed sender-study
source was then stored in the correct directory selected by its report identity.
No timing, byte/digest result or stage verdict was rewritten or reclassified.

Keep the validated original streaming/verification/publication safeguards. The
survey does not authorize a speed claim, weakened readback, unchecked deletion,
global power-saving changes or a new permanent network service. Further product
work follows measured opportunity and a concrete reviewed amendment where the
transport or helper protocol changes. Wireless discovery/recovery remains in
its existing queued component and is not duplicated by this investigation.
