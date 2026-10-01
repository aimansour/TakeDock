param([string]$Directory=(Join-Path (Split-Path $PSScriptRoot -Parent) 'test-results/drivers'))
$ErrorActionPreference='Stop'
$Directory=[IO.Path]::GetFullPath($Directory)
New-Item -ItemType Directory -Path $Directory -Force|Out-Null
$runtimeVersions=@(Get-Item 'C:/Program Files (x86)/Microsoft/EdgeWebView/Application/*/msedgewebview2.exe' -ErrorAction SilentlyContinue|ForEach-Object {$_.VersionInfo.ProductVersion})
if(-not $runtimeVersions.Count){throw 'Install the WebView2 Evergreen x64 runtime before setting up desktop drivers.'}
$runtimeVersion=$runtimeVersions|Sort-Object {[version]$_} -Descending|Select-Object -First 1
if($runtimeVersion -notmatch '^\d+\.\d+\.\d+\.\d+$'){throw 'Invalid WebView2 runtime version'}
$cargoRoot=Join-Path $Directory 'tauri-driver-2.1.0'
$driver=Join-Path $cargoRoot 'bin/tauri-driver.exe'
if(-not(Test-Path -LiteralPath $driver)){
    & cargo install tauri-driver --version 2.1.0 --locked --root $cargoRoot
    if($LASTEXITCODE -ne 0){throw 'Pinned tauri-driver installation failed'}
}
$edgeRoot=Join-Path $Directory "edge-$runtimeVersion"
$edge=Join-Path $edgeRoot 'msedgedriver.exe'
if(-not(Test-Path -LiteralPath $edge)){
    $zip=Join-Path $Directory "edge-$runtimeVersion.zip"
    Invoke-WebRequest "https://msedgedriver.microsoft.com/$runtimeVersion/edgedriver_win64.zip" -OutFile $zip
    Expand-Archive -LiteralPath $zip -DestinationPath $edgeRoot
}
$env:TAKEDOCK_TAURI_DRIVER=$driver
$env:TAKEDOCK_EDGE_DRIVER=$edge
Write-Output "Desktop drivers ready: tauri-driver 2.1.0, EdgeDriver $runtimeVersion."
