# Actual wireless Copy measurements with ADB Burst Mode

Date: 2026-10-24. Two alternating comparisons of an actual 500,000,000-byte
native Copy completed successfully. Burst Mode was faster in both comparisons,
with complete stored-file verification and preservation checks. A subsequent
1,000,000,000-byte native Copy on the ordinary default connection took 280.38
seconds despite the enabled setting. The large speedup was therefore not
reproduced consistently; the overall speed problem remains open.

## What changed

The existing ADB 37.0.1 server reported `burst_mode: false`. The phone advertised
`delayed_ack` support. AOSP documents the experimental `ADB_BURST_MODE=1` setting:
it permits multiple data packets in flight instead of waiting for an acknowledgement
after each packet. See the [official Burst Mode explanation](https://android.googlesource.com/platform/packages/modules/adb/+/refs/heads/main/docs/dev/delayed_ack.md).

The comparison used two temporary, owned ADB servers on separate loopback ports,
one with the setting disabled and one enabled. Both connected to the same trusted
wireless phone using the existing pairing. The user's original ADB server,
connection, pairing key and startup shortcut were preserved; only the temporary
servers were closed afterward. No router, firewall or phone accessibility setting
was changed. No compression, proxy ADB, transport downgrade or file-queue change
was introduced.

The host used a 1 Gbps Ethernet connection. The Samsung SM-A155F ran Android 16
on 5 GHz Wi-Fi. The tested SDK was 37.0.1; WebView2 and EdgeDriver were both
155.0.4283.62. Radio conditions and caches were uncontrolled, so the substantial
variation between observations must remain visible.

## Real application results

Both sides used the same frozen serial diagnostic executable from
`74e1d556fc5a32f9beea42bbb1d094f48c95726a`, SHA-256
`0d6224f97abd49a0dec876ec7276e14bb53913853c20a270f9bb36ed4b28bc3b`.
Its application/helper runtime source matches the current product. Exact resource
hashes were checked before launch. The application and its real ADB children
inherited only the selected temporary server environment. No production code was
patched to perform the copy or create the result.

Native controls selected one owned test video, then invoked **Copy to computer**.
UI time runs from the native action to the matching visible completed result;
backend time runs from accepted operation to authoritative terminal outcome.
It includes streaming, synchronization, actual stored-file readback and serial
verification. Both clocks and the complete matching diagnostic footer were checked.

| Pair | Execution order | Normal UI seconds | Burst UI seconds | Normal backend seconds | Burst backend seconds | UI time ratio |
| ---: | --------------- | ----------------: | ---------------: | ---------------------: | --------------------: | ------------: |
|    1 | Normal → Burst  |            242.25 |            24.81 |                 242.11 |                 24.67 |         9.76× |
|    2 | Burst → Normal  |            130.16 |            62.62 |                 130.01 |                 62.46 |         2.08× |

All four operations transferred exactly 500,000,000 bytes. Each published copy's
full SHA-256 independently matched the preserved, fully decoded local backup.
Fresh source identities and full hashes were checked before and after Copy;
originals remained on the phone. Native observation bounds were at most 740.2 ms
and 531.8 ms for the respective pairs. The measured differences are much larger
than those observation bounds, but two pairs do not characterize all network
conditions. Do not promise that every 500 MB transfer will finish in 25 seconds.

The two-pair configuration comparison contains no Move samples and does not
satisfy the earlier buffering candidate's full acceptance matrix. It does not
revive that rejected candidate or claim a new production transfer algorithm.

## Ordinary default connection follow-up

After persistent activation and restoration of the existing trusted connection,
one additional native Copy used the ordinary default SDK server, without a custom
server socket or an extra client environment override. The same frozen serial
application copied 1,000,000,000 bytes. Its UI total was **280.38 seconds** and its
backend total **280.27 seconds**. Streaming took **271.00 seconds**; serial phone
and actual stored-file hashing took **8.86 seconds**. Size, full output/source
digests, current profile, unrelated files, accessibility state, existing server,
pairing and Home/sleep cleanup all passed.

This single follow-up is not a matched 1 GB comparison, so it neither establishes
a 1 GB speedup nor isolates the reason for the variation. It does establish that
successful temporary-server observations cannot be promoted into a reliable
ordinary-connection speed guarantee. Further investigation targets streaming.

A subsequent channel-only control held the running default server constant and
alternated only the client Burst environment. Three 64 MiB pairs took
17.44/11.57, 17.93/13.29 and 7.18/16.20 seconds for client-off/client-on. The last
pair reversed the apparent advantage. Client flags alone did not produce a
consistent improvement. All exact bytes and full digests passed; the shared
server and pairing were retained. These are not application transfer totals.

## Preparation, failed setup and preservation

An earlier channel-only probe transferred a known zero source: 8 MiB warm-ups
followed by three alternating 64 MiB pairs. The middle observed times were
16.39 seconds with Burst disabled and 8.68 seconds enabled. Those observations
isolated the channel and are not application Copy timings.

The application comparison read an already owned test master and its immutable
backup without moving or deleting the phone file. A metadata-only source change
since the historical trial first stopped preparation. Current full bytes, size
and backup decode were then verified before creating a fresh **Copy-only** identity
reference in memory. The original manifest, identities, old timings and journals
were not rewritten. This revalidation never authorized Move or Delete.

Three native setup attempts stopped before a timed file operation: process
environment restoration, a stale source identity and a refresh-locator mismatch.
Their raw failed records remain private. The environment wrapper now distinguishes
absent, empty and populated variables. The refresh locator now matches the real
English label, **Refresh videos**. A rendered-Svelte contract test first reproduced
the mismatch, then passed after the fix; it also verifies the scoped loading
locator against a second, unrelated panel. All 88 UI tests, 50 pure benchmark
cases and Svelte checks passed after this harness fix.

The final comparison rechecked unrelated phone identities/hashes and unchanged
current accessibility services. Every native process/driver was closed and the
current pre-test desktop profile was restored byte-for-byte. The original ADB
server process, authorized connection and pairing key were confirmed unchanged
at cleanup. The phone returned Home and its screen was put to sleep. Raw media,
addresses, serials, profiles, diagnostic traces and receipts remain private.

## Applying the setting and remaining scope

Persistent activation was applied on this workstation with the user's explicit
authorization for a reasoned temporary interruption. The existing pairing key,
startup shortcut and current application profile were preserved. The default
server was launched through the actual desktop Explorer automation object;
the launch receipt verifies Explorer as the short launcher's parent. The trusted
phone connection was restored and the server reports Burst enabled. The per-user
environment value is saved; a complete Windows reboot was not tested.

For other installations, activation is an operator step: the ADB server must start with
`ADB_BURST_MODE=1`. Setting it only on a client does not change an already running
server. A one-time shared-server restart requires coordination; preserve pairing,
restore the authorized phone connection and verify `adb server-status` afterwards.
The existing Windows startup shortcut invokes `adb start-server`; a per-user
environment setting is inherited at the next login. No hard-coded phone address
or pairing secret belongs in the shortcut or repository.

Automatic paired-phone discovery/recovery remains in the existing queued wireless
component, now [explicitly specified](../superpowers/specs/2026-10-22-large-file-transfer-design.md#existing-wireless-component-automatic-recovery-queued-2026-10-24).
Neither successful temporary ADB configuration nor the phone's native settings
tile implements that future application feature. Audible NVDA/virtual-cursor
evaluation remains deferred; this experiment establishes native actions, file
results and byte preservation, not an audible screen-reader audit.
