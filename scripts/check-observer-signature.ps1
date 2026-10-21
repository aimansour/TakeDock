$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'observer-signing-identity.ps1')
$root=Split-Path $PSScriptRoot -Parent
$signer=Join-Path $env:ANDROID_HOME ('build-tools/37.0.0/'+$(if($IsWindows){'apksigner.bat'}else{'apksigner'}))
$output=& $signer verify --verbose --print-certs (Join-Path $root 'src-tauri/resources/observer.apk')
if($LASTEXITCODE -ne 0){throw 'Observer APK signature failed'}
$expected=(Get-Content -LiteralPath (Join-Path $root 'observer/signing-certificate.sha256') -Raw).Trim()
try {Assert-ObserverSigningIdentity -Output $output -Expected $expected}
catch {
    Write-Output "Expected public certificate SHA-256: $expected"
    $output | Select-String 'certificate SHA-256 digest:' | ForEach-Object { Write-Output $_.Line }
    throw
}
Write-Output 'PASS: persistent Observer signing identity verified.'
