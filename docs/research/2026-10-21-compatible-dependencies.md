# Compatible dependency maintenance during file-result work

On 2026-10-21 the user expanded the approved work to include compatible project
updates and small required adjustments. This supersedes the initial component's
no-upgrade constraint. The feature's full acceptance remains in progress.

The unchanged hosted `npm audit --audit-level=moderate` gate failed on Vite 8.3.1
in [Validate 39537593382](https://github.com/wip-bunker-42/TakeDock/actions/runs/39537593382).
Local audit independently reproduced the high-severity affected dependency.
[The upstream advisory](https://github.com/vitejs/launch-editor/security/advisories/GHSA-587h-7wmj-h5pm)
lists Vite 8.3.3 as patched; the user then authorized all compatible updates, and
the current stable Vite 8.3.4 was selected without audit bypass or force flags.

## Selected versions

| Component                           | Previous        | Updated                                 |
| ----------------------------------- | --------------- | --------------------------------------- |
| Vite                                | 8.3.1           | 8.3.4                                   |
| Svelte                              | 5.57.1          | 5.57.2                                  |
| Tauri JS API / CLI                  | 2.12.1 / 2.12.1 | 2.12.4 / 2.12.3                         |
| Tauri Rust / build                  | 2.12.1 / 2.7.1  | 2.12.4 / 2.7.3                          |
| Dialog JS/Rust                      | 2.8.0           | 2.8.3                                   |
| Rust updater / single-instance      | 2.13.1 / 2.5.2  | 2.14.0 / 2.5.4                          |
| Vitest / jsdom                      | 5.0.3 / 30.1.1  | 5.0.5 / 30.1.3                          |
| svelte-check / Prettier             | 4.7.6 / 3.9.9   | 4.7.7 / 3.9.10                          |
| user-event / Node type declarations | 14.6.7 / 26.6.3 | 14.6.8 / 26.6.5                         |
| Rust build toolchain                | 1.98.1          | 1.99.1, including three Android targets |
| CI Node                             | 24.21.0         | 26.12.0                                 |
| Gradle wrapper                      | 9.6.0           | 9.8.2                                   |
| Android Build-Tools                 | 36.0.0          | 37.0.0                                  |
| setup-node / setup-java actions     | 7.0.0 / 6.0.1   | 7.1.0 / 6.1.0                           |
| upload / download artifact actions  | 7.0.1 / 8.0.1   | 7.0.2 / 8.0.2                           |

Compatible transitive npm/Cargo dependencies were resolved into their lockfiles.
The direct test dependencies minisign-verify and base64 were also updated to
0.3.0 and 0.23.1; real updater signature fixtures and verification gates remain.
Actions retain exact release-commit SHAs. Gradle distribution and wrapper bytes
were checked against the official release metadata; the wrapper SHA-256 is
`3b8a25775a69158b5ad2b1d17a80a88dc7b40a352a73eb27d06c612a7ce68e98`.

## Compatibility boundaries and evidence

TypeScript stays at 6.0.3, the latest stable 6: current svelte-check 4.7.7 declares
`^5.0.0 || ^6.0.0`, excluding latest TypeScript 7.0.2. No unsupported peer override
was introduced. AGP 9.4.1, NDK 30.0.16248370, tauri-driver 2.1.0 and the existing
checkout/setup-android action releases were already current stable. Android API
target/minimum policy is retained; a platform API target is an application
compatibility requirement rather than a package patch version.

After updating, local npm audit reports zero vulnerabilities, Svelte reports zero
diagnostics and Vite production build passes. The current reconciler milestone
passes 69 UI tests and 98 Rust library tests with strict desktop Clippy. All three
Android helper ABIs build on Rust 1.99.1, and all 25 helper cases pass (16 unit
and nine integration cases). The actual updater signature example passes its signed fixture,
corrupt rejection, version binding, downgrade and malformed-manifest checks.

The local Java runtime is absent. Hosted Observer tests/lint/build and Node 26 UI
gates passed in [Validate 39542130212](https://github.com/wip-bunker-42/TakeDock/actions/runs/39542130212),
whose Rust job also passed the actual post-acknowledgement rename process tests.
These are milestone checks: the final result/history views and final source tree
must still pass every existing gate before acceptance.

The inventory generator verified 321 redistributed dependency notices. The new
tauri-plugin 2.7.2 crate omits its license text, so its supplemental text was fetched
from the exact published source commit, not assumed from an older package version.
