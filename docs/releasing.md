# Releases and signing

TakeDock checks the public GitHub Releases `latest.json` at startup and from
Settings. Checking never downloads or installs. **Install update** is the only
installation trigger. The official Tauri updater validates the artifact and
its signed version before installation. Downgrades are disabled. On Windows,
the updater's `on_before_exit` callback closes the ADB session and cancels file
work before the installer runs. Finish recording on the phone before updating.

The checked-in public key is intentional. Keep the matching private key and
password outside Git, backed up in a restricted location. Generate a key once
with `npm exec tauri signer generate`; do not regenerate it between releases.
Set `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` as
GitHub Actions secrets. For local bundling set the same two process environment
variables, loading the key from restricted external storage. Never
place a password in a committed script or command log. The standalone
signer also supports `TAURI_SIGNING_PRIVATE_KEY_PATH`. Tauri signing is update
integrity verification; it is independent of Windows Authenticode.

Builds pin Node, Rust, Android tools and dependencies. The Observer uses its
own persistent Android signing key, also stored outside Git. CI secret and
publication setup is documented below.

## Required GitHub Actions secrets

| Name | Value |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | Exact encrypted Tauri private key content |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Tauri private key password |
| `OBSERVER_KEYSTORE_BASE64` | Base64 of the persistent Android PKCS12 keystore |
| `OBSERVER_STORE_PASSWORD` | Android keystore password |
| `OBSERVER_KEY_PASSWORD` | Password for alias `takedock-observer` |

Keep both signing identities across releases. The public Android certificate
SHA-256 fingerprint is `observer/signing-certificate.sha256`; release builds
must match it. The JVM reader tests use Debug, where current AGP enables unit
tests; release APKs separately run full release lint and signing verification.

## Publication gate

Pushes and pull requests run the same Rust, frontend, Observer and Windows
Desktop E2E gates. The reusable workflow rejects failed, cancelled, skipped
and missing jobs. GitHub Actions dependencies are pinned to verified commit
SHAs, with Dependabot checking compatible updates.

Dispatch **Release** manually for a signed validation-only candidate. This
builds exactly the publishable installer/assets and never publishes. For an
approved stable version, update package, Rust/Tauri and Observer versions and
increment the Observer versionCode; push tag `vX.Y.Z`. Only a tag with all
gates passing reaches publication. Tags must match the app metadata.

The desktop suite tests the release EXE/resources before packaging. Bundling
uses `--no-binary-patching` because TakeDock ships one Windows installer type;
updater metadata uses the generic `windows-x86_64` platform. This preserves the
tested EXE bytes. The package verifier extracts the actual NSIS installer with
pinned, checksum-verified portable 7-Zip 26.03 and compares every bundled hash,
verifies the updater signature/version and builds `SHA256SUMS.txt`/`latest.json`.

Releases are serialized. Publication rechecks versions/checksums/acceptance,
rejects stale versions, creates a draft and downloads all uploaded assets to
verify their bytes before making it public/latest. A failed upload stays a
draft. Installation remains a user's explicit Settings action.

## Verifying updater behavior without installation

Create a harmless test file outside the repository. Sign it with the Tauri CLI
using `signer sign --app-version 999.0.0` and the signing environment variables.
Run `cargo run -p takedock --features desktop --example updater-validation --
<artifact> <artifact.sig>`. This unbundled example uses a temporary loopback
server and the actual updater's check/download/signature code. It proves valid
download, corrupt rejection, signed-version binding, downgrade rejection and
malformed version rejection. It never calls install and opens no app window.
The production configuration permits HTTPS only. A checked-in harmless artifact
and its public signature let CI run `scripts/check-updater.ps1` without any
private key. They are not bundled in TakeDock or published as an update.

Frontend tests independently prove automatic/manual checks cannot install,
focus stays on the initiating control, and installation requires an available
offer and explicit activation. Desktop E2E covers the release executable and
real IPC; phone, NVDA and installer acceptance remain separate checks.
