# Changelog

## 0.1.2 — 2026-10-01

First public release, including named windows sharing one phone/settings/folder,
single-process shortcut activation, Ctrl+Shift+N, default Videos/TakeDock saving,
newest-first ordering, millisecond container duration and native activity progress.
Backend result sounds occur once regardless of the number of open windows.
Delayed repeated camera states cannot confirm newer intent prematurely; expired
connections invalidate file selections and confirmations. Rejected capture requests
recover without reconnecting. Shared saves retain authoritative settings, update
installation binds to the displayed offer, interrupted journal preparation preserves
file access, and lost camera eligibility preserves keyboard focus.
Monotonic session snapshot revisions reject out-of-order desktop events even
when the accepted command number is unchanged.

## 0.1.1 — local acceptance candidate

Named-window and progress acceptance candidate, superseded by the final snapshot
ordering repair. Never published as a GitHub release.

## 0.1.0 — development baseline

Initial TakeDock implementation: Tauri/Svelte Windows app, English/Arabic dark interface,
event-driven Open Camera Observer, persistent ADB capture shell, independent
verification, protected batch video management, silent success default, explicit
signed in-app updates and gated GitHub publication with actual Desktop E2E.

Targeted Windows 10/11 x64 and Android 16+. Initial hardware evidence is Windows
10/Android 16/Open Camera 1.56.2; newer-platform hardware acceptance remains open.
