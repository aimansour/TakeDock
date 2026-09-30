param([ValidateSet('Debug', 'Release')][string]$Configuration = 'Debug')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($Configuration -eq 'Release' -and -not $env:OBSERVER_KEYSTORE) {
    throw 'Release builds require OBSERVER_KEYSTORE and the signing environment variables documented in docs/releasing.md.'
}
$projectRoot = Split-Path $PSScriptRoot -Parent
Push-Location (Join-Path $projectRoot 'observer')
try {
    & ./gradlew.bat "test${Configuration}UnitTest" "lint${Configuration}" "assemble${Configuration}"
    if ($LASTEXITCODE -ne 0) { throw 'Observer checks/build failed.' }
    $variant = $Configuration.ToLowerInvariant()
    $apk = Join-Path $PWD "app/build/outputs/apk/$variant/app-$variant.apk"
    if (-not (Test-Path -LiteralPath $apk)) { throw 'Signed Observer APK missing.' }
    $resources = Join-Path $projectRoot 'src-tauri/resources'
    New-Item -ItemType Directory -Path $resources -Force | Out-Null
    Copy-Item -LiteralPath $apk -Destination (Join-Path $resources 'observer.apk')
    Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $resources 'observer.apk') | Select-Object Algorithm, Hash
} finally { Pop-Location }
