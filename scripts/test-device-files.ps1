param([Parameter(Mandatory)][string]$TestManifest,[Parameter(Mandatory)][string]$Destination,[string]$AdbPath='adb')
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
Push-Location $projectRoot
try {
    $manifest=(Resolve-Path -LiteralPath $TestManifest).Path
    $folder=(Resolve-Path -LiteralPath $Destination).Path
    $adbExecutable=(Get-Command $AdbPath -ErrorAction Stop).Source
    & cargo run --locked -p takedock --example device-files -- (Join-Path $projectRoot 'src-tauri/resources') $adbExecutable $manifest $folder
    if ($LASTEXITCODE -ne 0) {throw 'Real-device file acceptance failed.'}
} finally {Pop-Location}
