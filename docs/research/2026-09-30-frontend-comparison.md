# Frontend investigation: plain TypeScript versus Svelte

Date: 2026-09-30. Scope: pre-implementation research for TakeDock.

Recommendation: use Svelte 5 with TypeScript and Vite for the interface,
while keeping recording dispatch, verification, and file operations in the
independent Rust engine already described by the design. This is an
engineering recommendation based on requirements and primary-source review;
it is not a measured performance ranking.

## What was investigated

- The current TakeDock design and implementation plan, including optimistic
  recording controls, silent success, batch selection, updates, and RTL.
- Official Svelte documentation and the event implementation at release
  `svelte@5.57.1`.
- Official Tauri frontend and IPC documentation.
- Official SvelteKit accessibility behavior, to distinguish Svelte from its
  separate application framework.
- Current package metadata from npm, including supported peer versions.

No product code was scaffolded or dependencies installed. Neither frontend
was benchmarked. The user subsequently selected Svelte; the design and plan
now use plain Svelte 5 with TypeScript/Vite and focused Windows Desktop E2E.

## Comparison specific to TakeDock

| Criterion | Plain TypeScript and HTML | Svelte 5 and TypeScript |
| --- | --- | --- |
| Immediate command dispatch | Direct event handlers, no framework scheduler | A handler can call Tauri immediately, independently of reactive rendering |
| Runtime and dependencies | Fewer framework-specific dependencies | Adds a compiler, runtime, Vite integration, and component checking |
| Predicted and observed state | Explicit state model plus manual DOM synchronization | Explicit state model plus reactive presentation |
| Video selection and progress | Must implement selective node updates and lifecycle cleanup | Keyed lists and component lifecycle provide reusable mechanisms |
| Screen-reader semantics | Native HTML works if implemented correctly | Native HTML works; compile-time warnings catch some markup mistakes |
| Focus after control changes | Must explicitly preserve or restore it | Must explicitly preserve or restore it; no automatic guarantee |
| Silence on results | Application controls announcements and sounds | Application controls announcements and sounds |
| Long-term changes | Fewer tool upgrades, more presentation bookkeeping | More tooling compatibility work, less presentation bookkeeping |

The original choice favored minimal dependencies and direct DOM control.
TakeDock's combined recording states, pending verification, file selections,
progress, localization, Settings, and updates make presentation consistency
more significant than the number of top-level screens suggests. Svelte is
therefore the stronger overall choice for this project. Plain TypeScript
remains technically capable of meeting the requirements.

## Latency and command correctness

[Svelte compiles components into JavaScript](https://svelte.dev/docs/svelte/overview),
but it also has runtime event handling and reactive scheduling. It does not
have zero runtime overhead. Its
[event implementation](https://github.com/sveltejs/svelte/blob/svelte%405.57.1/packages/svelte/src/internal/client/dom/elements/events.js)
calls the relevant delegated handler during event propagation; that handler
does not inherently await component rendering.

[Reactive effects are batched](https://svelte.dev/docs/svelte/$effect), while
[Tauri invokes the backend through asynchronous IPC](https://v2.tauri.app/reference/javascript/api/namespacecore/#invoke).
Consequently the chosen frontend must call dispatch directly from the user
event, update predicted state without waiting for confirmation, and consume
verification separately. Do not send toggle commands from reactive effects
or make dispatch await `tick`, animation, transfer completion, or observation.

Native TypeScript permits fewer framework instructions in this path. That
fact alone does not quantify the end-to-end difference or establish that a
user would notice it. The earlier ADB shell measurements compare shell
lifetimes, not frontend frameworks, and must not be reused as a Svelte result.

## Accessibility and silent behavior

[Svelte's accessibility warnings](https://svelte.dev/docs/svelte/compiler-warnings)
help identify missing labels and inappropriate interactive markup. They are
developer diagnostics, not screen-reader announcements. They do not prove
runtime accessibility or screen-reader compatibility.

[Keyed lists](https://svelte.dev/docs/svelte/each#Keyed-each-blocks) track item
identity when rows change. Use stable identities rather than array positions;
still handle rename/delete identity changes and removal of a focused node.
Recording controls can reuse the primary button while changing its action
and accessible name, so the obsolete Start action is no longer exposed.

Use native buttons, labeled inputs, and a native table. Render only applicable
actions, without removal animations that leave old controls accessible.
Preserve logical focus deliberately. Do not add live regions or automatic
result speech. Success remains silent unless enabled in Settings.

[SvelteKit adds route announcements and navigation focus management](https://svelte.dev/docs/kit/accessibility).
These features are unsuitable defaults for the requested interaction model.
TakeDock needs plain Svelte with Vite and an application-local view switcher,
not SvelteKit's router or server features.

## Current tool compatibility

The registry reported these stable versions during the investigation:

| Package | Version | Relevant compatibility |
| --- | --- | --- |
| Svelte | 5.57.1 | Supported by the selected Vite plugin |
| Vite | 8.3.1 | Supported by the selected Vite plugin |
| `@sveltejs/vite-plugin-svelte` | 7.3.1 | Vite 8, Svelte >=5.46.4 within major 5 |
| `svelte-check` | 4.7.6 | TypeScript 5 or 6, not declared support for 7 |
| TypeScript for this combination | 6.0.3 | Latest stable major-6 version returned by npm |
| Latest TypeScript overall | 7.0.2 | Outside the checker's declared peer range |

The installed Node 24.21.0 satisfies the selected Vite/plugin engine ranges.
The [svelte-check package definition](https://github.com/sveltejs/language-tools/blob/master/packages/svelte-check/package.json)
confirms the TypeScript peer range. Do not force TypeScript 7 through an
unsupported range or disable component checking just to use the newest
version number. Re-evaluate this matrix when implementation starts.

Package unpacked size is not application bundle size. No installer-size,
startup-time, memory, or per-command timing claim follows from this metadata.

## Changes incorporated into the design and plan

Replace manual UI modules with small Recording, Videos, and Settings Svelte
components. Keep IPC wrappers and testable transition logic independent of
components, and retain all Rust/Observer separation. Use one owned subscription
per event stream with explicit cleanup. Limit display progress updates so
background file work does not flood the main thread.

Add `svelte-check` to CI alongside Rust checks and behavior tests. Test logical
focus, disappearance of obsolete controls, delayed verification, rapid inputs,
batch selection, language changes, no live announcements, and optional success
sound. Use actual WebView2/NVDA tests; compiler warnings are an additional check.

The updated plan also includes focused Windows Desktop E2E using
[Tauri's WebDriver support](https://v2.tauri.app/develop/tests/webdriver/)
and [documented Windows CI integration](https://v2.tauri.app/develop/tests/webdriver/ci/).
These tests can detect executable startup, Rust IPC, command ordering, focus,
batch-copy, and Settings-persistence failures that isolated component tests
cannot establish. Use a standard WebDriver client with external `tauri-driver`
and Edge WebDriver against the release app, plus a deterministic external ADB
fixture. The selected path requires no embedded test server or test-only IPC
in the shipping app. Keep four critical flows as required publication checks;
retain real-phone and NVDA acceptance because the fixture cannot certify them.

Measure user-event-to-IPC dispatch separately from control-update time and
phone confirmation, including under file-transfer load. Compare median and
tail timings in a production build. Those measurements can justify targeted
hot-path changes if needed; current research does not establish numeric
performance equivalence between the two choices.
