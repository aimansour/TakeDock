# Security policy

[العربية](SECURITY.ar.md)

Report vulnerabilities through [GitHub private vulnerability reporting](https://github.com/aimansour/TakeDock/security/advisories/new).
Do not publish credentials, phone identifiers or personal videos in public issues.
Include the version, expected/actual behavior, a minimal reproduction with synthetic
files, and the affected trust boundary. Only the latest stable release is supported.

TakeDock trusts your already authorized ADB connection and the selected desktop
destination. ADB shell access is powerful: authorize only your own trusted computers
and use a trusted network. Pairing/authentication is external. No server is opened
by the shipped desktop app or Observer. Test servers/drivers are development tools.

The Observer targets its own instrumentation and preserves existing accessibility
services. It reads the foreground package/camera controls to verify recording;
it does not collect or upload accessibility text. File management is restricted
to the default Open Camera directory, validates identities, rejects symlinks and
never overwrites a destination. A completed destructive commit cannot be rolled
back by losing its acknowledgement or cancelling afterwards.

Updates require explicit activation, HTTPS, the checked-in public key and a
signature bound to the offered version. Signing credentials are held in restricted
external storage and GitHub Actions secrets. Updater signing does not substitute
for Windows Authenticode; current installers have no Authenticode certificate.

There is no telemetry/video upload. Startup update checks contact GitHub, whose
network/privacy policies apply. Settings remain in the user's Windows profile.
Dependency updates must pass the mandatory gate before publication.
