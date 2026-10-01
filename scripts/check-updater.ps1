$ErrorActionPreference='Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    & cargo run --locked -p takedock --features desktop --example updater-validation -- tests/fixtures/updater/artifact.txt tests/fixtures/updater/artifact.txt.sig
    if($LASTEXITCODE -ne 0){throw 'Updater signature/response validation failed.'}
} finally {Pop-Location}
