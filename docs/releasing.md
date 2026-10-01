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
GitHub Actions secrets. For local builds use `TAURI_SIGNING_PRIVATE_KEY_PATH`
and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` in the process environment. Never
place a password in a committed script or command log. Tauri signing is update
integrity verification; it is independent of Windows Authenticode.

Builds pin Node, Rust, Android tools and dependencies. The Observer uses its
own persistent Android signing key, also stored outside Git. CI secret and
publication setup is documented with the release workflow.

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
