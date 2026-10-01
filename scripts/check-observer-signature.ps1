$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$signer=Join-Path $env:ANDROID_HOME ('build-tools/36.0.0/'+$(if($IsWindows){'apksigner.bat'}else{'apksigner'}))
$output=& $signer verify --verbose --print-certs (Join-Path $root 'src-tauri/resources/observer.apk')
if($LASTEXITCODE -ne 0){throw 'Observer APK signature failed'}
$actual=($output|Select-String '^Signer #1 certificate SHA-256 digest:').Line -replace '^.*digest: ',''
$expected=(Get-Content -LiteralPath (Join-Path $root 'observer/signing-certificate.sha256') -Raw).Trim()
if($actual.Trim() -ne $expected){throw 'Observer signing identity changed'}
Write-Output 'PASS: persistent Observer signing identity verified.'
