# Large-file transfer measurements

Date: 2026-10-22. Status: baseline calibration and candidate safety checks complete;
the bounded write-buffering candidate awaits physical comparison. No performance
improvement has been accepted.

## Method and artifact provenance

The approved measurement contract is in
[the specification](../superpowers/specs/2026-10-22-large-file-transfer-design.md).
Native Copy/Move actions and matching result notices define UI elapsed time.
Opt-in, bounded QPC diagnostics define backend accepted-to-terminal time and
stage durations. Both clocks use the same Windows host frequency. Warm-ups,
invalid attempts and measured pairs are retained separately. No incoming-stream
digest replaces stored-file readback.

| Artifact                                      | Source revision                            | Executable SHA-256                                                 |
| --------------------------------------------- | ------------------------------------------ | ------------------------------------------------------------------ |
| Public v0.1.5                                 | `75476b3c65a0c6719c5ac65c52490c5f9bef308d` | `4a4863d1a6cc1aa72f49ba24c37abb29283cd62f2202b644a0b564388c8ffaef` |
| Serial baseline, diagnostics enabled          | `74e1d556fc5a32f9beea42bbb1d094f48c95726a` | `0d6224f97abd49a0dec876ec7276e14bb53913853c20a270f9bb36ed4b28bc3b` |
| Serial baseline, default build (preserved)    | `74e1d556fc5a32f9beea42bbb1d094f48c95726a` | `76f469abbbda2debb92c1198fefb77445610517c627c5689faca2c921832d524` |
| Buffered candidate, diagnostics enabled       | `50c4969e769d660fdfda8792c4ebe8acd3d3b260` | `234d67a527de0503a2d6d11ba12818c18ac75a92f375307ee485786e487f541b` |
| Buffered candidate, default build (preserved) | `50c4969e769d660fdfda8792c4ebe8acd3d3b260` | `1dc32ed4673ce2fe11055c788a65c79e273cf37c8726f15068b3f323ec7a795e` |

Host executables were frozen before subsequent harness-only fixes. Local ignored
helper resource bytes differed from the public release despite matching tracked
helper source, lockfile, toolchain and build script. Original bundles were
preserved. Separate measured runtime bundles use the verified public helper on
every ABI on both sides; the phone's arm64 helper SHA-256 is
`5f24c6cdae8677bbe16c871003c1ddd0f337269201792ccb8b293ca127b93014`.
All eleven extracted release resources matched the public acceptance artifact;
downloaded release assets matched their published checksums. The installer was
extracted without execution.

The fixtures contain exactly 500,000,000 and 1,000,000,000 bytes. They are valid
short MP4 containers padded with a valid `free` atom of nonsparse random bytes,
modelling bulk I/O rather than that many bytes of distinct decoded frames. Full
size, SHA-256 and video decode checks precede phone mutations outside the timed
interval. Immutable local backups are retained. Each Move receives a new owned
duplicate; reusable masters and unrelated media are never moved. Raw filenames,
addresses, identities, settings, journals and traces remain ignored/private.

The phone uses wireless ADB. A read-only observation showed 5 GHz (5180 MHz),
390 Mbps reported link rate and RSSI -55 dBm; advertised PHY rates are not file
throughput. The host has a four-core/eight-thread Intel Xeon E5-1620 v3. OS and
phone caches are uncontrolled. No root, cache flushing, antivirus exclusions,
transport changes or accessibility changes are used. Phone cleanup returns Home
and sleeps the screen while preserving wireless pairing and the ADB server.

## Complete baseline calibration

The native stage completed all sixteen cells: eight warm-ups and eight measured
runs (one released/instrumented pair per size/kind). Each accepted run has its
independent output size/SHA receipt, appropriate Copy-preserved/Move-absent
source check and successful cleanup receipt. All eight instrumented traces have
complete matching boundaries and loss-free footers. Twenty-four pure harness
cases and the preserved matrix/trace/receipt verification passed. Unrelated
phone identities and hashes were reverified; phone sleep, owned-process closure
and per-run profile restoration completed. No speed optimization is represented
by this comparison: both implementations still use the serial pipeline.

All durations below are milliseconds, rounded to 0.1 ms. The private records
retain their full tick-based precision. Uninstrumented released backend time is
unavailable, rather than inferred from UI or file existence.

| Payload bytes | Kind | Observation     | Released UI | Instrumented UI | Instrumented backend |
| ------------: | ---- | --------------- | ----------: | --------------: | -------------------: |
|     500000000 | Copy | warm-up         |    106742.3 |        127785.8 |             127656.4 |
|     500000000 | Copy | measured pair 1 |    123284.2 |        111893.1 |             111754.7 |
|     500000000 | Move | warm-up         |    170294.4 |        125129.9 |             125029.6 |
|     500000000 | Move | measured pair 1 |    130029.7 |        150698.2 |             150577.7 |
|    1000000000 | Copy | warm-up         |    323077.3 |        362226.4 |             362111.8 |
|    1000000000 | Copy | measured pair 1 |    283775.2 |        327149.4 |             327063.7 |
|    1000000000 | Move | warm-up         |    365068.6 |        500057.9 |             499980.1 |
|    1000000000 | Move | measured pair 1 |    287202.8 |        314518.3 |             314319.8 |

The conservative calibration uncertainty is **43374.1807 ms**, the largest
absolute measured paired UI difference. This includes variation and possible
instrumentation perturbation; it does not attribute all difference to tracing.
The largest observed native polling/request uncertainty was 507.3346 ms.
Warm-ups are excluded from those measured calibration pairs. With only one
measured pair per group, this calibration is not a repeatability/median-gain
acceptance test. The three-pair performance gate has not run yet.

## Measured stage costs and candidate decision

These are the instrumented measured runs, not warm-ups. Hashes are serial;
the enclosing verification interval already includes both and must not be added
to them again when computing total time.

| Payload bytes / kind | Stream ms | Sync ms | Verification ms | Phone hash ms | Desktop readback ms | Publish ms | Move stat / delete / index ms |
| -------------------- | --------: | ------: | --------------: | ------------: | ------------------: | ---------: | ----------------------------- |
| 500000000 Copy       |  107738.5 |     6.2 |          3488.6 |         981.2 |              2507.3 |        1.6 | —                             |
| 500000000 Move       |  146001.5 |     2.2 |          3515.1 |         969.4 |              2545.7 |        1.9 | 169.0 / 304.9 / 182.4         |
| 1000000000 Copy      |  319420.3 |     1.0 |          7414.4 |        2336.0 |              5078.4 |        3.0 | —                             |
| 1000000000 Move      |  305947.3 |     1.0 |          7162.6 |        2113.9 |              5048.7 |        2.0 | 152.1 / 277.5 / 150.6         |

Streaming accounts for approximately 96.4%, 97.0%, 97.7% and 97.3% of backend
elapsed time respectively. Initial/post-transfer stats and delivery overhead
account for the remaining small differences; full spans remain private.
Perfect overlap of the two measured hashes could remove at most their shorter
duration: about 969–2336 ms, before coordination cost. That cannot clear the
43374 ms conservative calibration budget in this declared test. The paired-hash
candidate is therefore rejected **before implementation**; neither more worker
coordination nor faster hash-only timing would establish the required total gain.

The user explicitly approved the
[bounded write-buffering amendment](../superpowers/plans/2026-10-22-transfer-buffering-amendment.md)
as Native. It evaluates one fixed 256 KiB desktop write buffer, retaining exact
size, explicit flush before `sync_all`, real stored-file readback, source identity
checks and guarded publication/deletion. Its gain is a hypothesis, not a result.
The baseline remains immutable and separate. Candidate acceptance still requires
all four groups and three matched pairs with both total metrics faster in every
pair and median gains exceeding ranges and uncertainty. A predeclared failure
stop may reject a partial comparison after a non-improving valid pair; an
incomplete matrix can never be accepted. No broader transport is implemented.

A later separate read-only `adb pull -Z` diagnostic copied the owned 500 MB
master in 95463.0 ms (about 5.24 decimal MB/s), with independent size/SHA,
unchanged source identity/hash and unchanged current desktop profile verified
outside that interval. Its fresh local output was removed; its backup and
receipt remain. It ran after calibration, with uncontrolled warm caches, and
does not include the product's verification or native settled-result interval.
It is not a matched product comparison or an acceptance sample and does not
justify a claimed transport speedup.

## Invalid observations and harness corrections

An earlier instrumented Copy warm-up completed its backend but was invalidated
because a native notice appeared after the harness's authoritative snapshot.
The harness now confirms from a fresh snapshot after the observed notice and
outside the UI interval; an injected-driver regression reproduced the fault
before the fix. The original invalid observation is retained and excluded.

A formatting-only profile restoration mismatch was diagnosed: all seven typed
settings were unchanged after application startup serialization. Original
profile bytes were restored; subsequent restore guards still reject changed
keys, types or values. Completed-operation receipts prevent repeated mutations.

Two released 500 MB Move warm-up attempts were rejected before publication.
Their sources remained at their exact original identities; their destinations
were empty. A preserved terminal snapshot identified Windows publication
failure at a 270-character final path. The harness uses shorter exclusively
created evidence directories and validates the complete prospective path before
creating a phone duplicate. A regression observed RED then GREEN. Product long-
path behavior is unchanged; these attempts are not performance samples.

Raw invalid attempts, decoded immutable backups, original and aligned runtime
bundles, exact hashes, conditions, settings and journals are retained privately.
The baseline report does not establish cold-cache performance or a universal
wireless speed. No production deletion step gained video decoding or test backups.
