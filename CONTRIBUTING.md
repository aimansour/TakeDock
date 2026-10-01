# Contributing

Use English for code, issues, pull requests and development documentation.
Keep essential user documentation synchronized with its Arabic counterpart.
Do not add live regions, router announcements, unsolicited speech, automatic
recording retries or verification waits to the capture dispatch path.

## Windows build

Install Node 24.21.0, Rust 1.98.1 (including rustfmt/clippy), Visual Studio C++
Build Tools/Windows SDK, PowerShell 7, JDK 17 and the Android SDK. Install SDK
`platforms;android-37.0`, `build-tools;36.0.0` and NDK `30.0.16248370`.
Set `JAVA_HOME`, `ANDROID_HOME` and `ANDROID_NDK_HOME` appropriately.

```powershell
npm ci
rustup target add aarch64-linux-android x86_64-linux-android armv7-linux-androideabi
./scripts/build-observer.ps1
./scripts/build-file-helper.ps1
npm run desktop:dev
```

Debug Observer signing is for development only. Remove that package from a
test phone before installing a release with the persistent release certificate.
Release credentials stay outside Git; see [signing](docs/releasing.md).
Build a release EXE with `npm run desktop:build -- --no-bundle` after building
resources. The release workflow produces the signed installer.

## Checks

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --features desktop --locked -- -D warnings
cargo test --workspace --locked
npm run format:check
npm run check
npm run test:ui
npm run build
node --test scripts/release-gate.test.mjs scripts/prepare-publication.test.mjs tests/desktop/tooling.test.mjs
./scripts/check-updater.ps1
./scripts/setup-desktop-drivers.ps1
npm run test:desktop
```

Observer builds also run seven reader unit tests and Android lint. Linux CI
covers platform-specific filesystem cases. [Desktop E2E](docs/testing/desktop-e2e.md)
requires actual release resources, WebView2 and a matching external EdgeDriver;
there is no embedded test server or mocked IPC. GitHub's elevated Windows runner
runs it through `run-desktop-unprivileged.ps1` without changing host policies.

Real-phone examples create recordings and can mutate files: use only dedicated
fixtures and the documented private manifest. Never commit phone identifiers,
pairing codes, personal filenames, credentials, videos or raw accessibility logs.
Add a meaningful regression test for behavior/safety changes. Describe behavior,
verification and compatibility evidence in pull requests. Report vulnerabilities
privately using [SECURITY](SECURITY.md).
