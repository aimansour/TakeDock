param([string]$Executable=$env:TAKEDOCK_APP_EXE,[string]$TauriDriver=$env:TAKEDOCK_TAURI_DRIVER,[string]$EdgeDriver=$env:TAKEDOCK_EDGE_DRIVER)
$ErrorActionPreference='Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
$driverProcess=$null
$settingsFile=Join-Path $env:APPDATA 'app.takedock/settings.json'
$originalSettings=$null
$profilePrepared=$false
try {
    if(Get-Process takedock -ErrorAction SilentlyContinue){throw 'Close TakeDock before running desktop acceptance.'}
    if($env:TAKEDOCK_PREBUILT_FIXTURE_DIR){$metadata=@{target_directory=[IO.Path]::GetFullPath($env:TAKEDOCK_PREBUILT_FIXTURE_DIR)}}
    else {
        $metadata=& cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
        if($LASTEXITCODE -ne 0){throw 'Cargo metadata failed'}
    }
    if(-not $Executable){$Executable=Join-Path $metadata.target_directory 'release/takedock.exe'}
    if(-not $TauriDriver){$TauriDriver=(Get-Command tauri-driver -ErrorAction Stop).Source}
    if(-not $EdgeDriver){$EdgeDriver=(Get-Command msedgedriver -ErrorAction Stop).Source}
    foreach($path in @($Executable,$TauriDriver,$EdgeDriver)){if(-not(Test-Path -LiteralPath $path -PathType Leaf)){throw "Required executable missing: $path"}}
    $installedDrivers=Get-Content -LiteralPath (Join-Path (Split-Path (Split-Path $TauriDriver -Parent) -Parent) '.crates2.json') -Raw|ConvertFrom-Json -AsHashtable
    $driverVersion=@($installedDrivers.installs.Keys|Where-Object {$_ -match '^tauri-driver 2\.1\.0 '})
    if($driverVersion.Count -ne 1){throw 'Install the pinned tauri-driver 2.1.0 with Cargo.'}
    & node --test tests/desktop/tooling.test.mjs
    if($LASTEXITCODE -ne 0){throw 'External driver guard failed'}
    $resourceRoot=Split-Path $Executable -Parent
    $resources=@((Join-Path $resourceRoot 'observer.apk'))
    foreach($abi in @('arm64-v8a','x86_64','armeabi-v7a')){$resources+=Join-Path $resourceRoot "file-helper/$abi/takedock-files"}
    foreach($notice in @('LICENSE','THIRD_PARTY_NOTICES.md','third-party/inventory.json','third-party/licenses.txt','third-party/supplemental.json','third-party/rust-standard-library.txt')){$resources+=Join-Path $resourceRoot $notice}
    foreach($resource in $resources){if(-not(Test-Path -LiteralPath $resource -PathType Leaf)){throw 'Bundled resource missing'}}
    $runtimeVersions=@(Get-Item 'C:/Program Files (x86)/Microsoft/EdgeWebView/Application/*/msedgewebview2.exe' -ErrorAction SilentlyContinue | ForEach-Object {$_.VersionInfo.ProductVersion})
    if($runtimeVersions.Count -eq 0){throw 'WebView2 runtime missing'}
    $runtimeVersion=$runtimeVersions|Sort-Object {[version]$_} -Descending | Select-Object -First 1
    $edgeVersion=(& $EdgeDriver --version | Select-Object -First 1) -replace '^.*?([0-9]+\.[0-9]+\.[0-9]+\.[0-9]+).*$','$1'
    if($edgeVersion -ne $runtimeVersion){throw "EdgeDriver $edgeVersion must match WebView2 $runtimeVersion"}
    if(-not $env:TAKEDOCK_PREBUILT_FIXTURE_DIR){
        & cargo build --locked -p takedock-adb-fixture
        if($LASTEXITCODE -ne 0){throw 'External ADB fixture build failed'}
    }
    if(-not(Test-Path -LiteralPath (Join-Path $metadata.target_directory 'debug/takedock-adb-fixture.exe') -PathType Leaf)){throw 'Built external ADB fixture missing'}
    $testBase=if($env:TAKEDOCK_TEST_TMP){$env:TAKEDOCK_TEST_TMP}else{[IO.Path]::GetTempPath()}
    $testRoot=Join-Path $testBase ('takedock-desktop-'+[guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
    if(Test-Path -LiteralPath $settingsFile){$originalSettings=[IO.File]::ReadAllBytes($settingsFile)}
    New-Item -ItemType Directory -Path (Split-Path $settingsFile -Parent) -Force | Out-Null
    $profilePrepared=$true
    $env:TAKEDOCK_TEST_ROOT=$testRoot
    $env:TAKEDOCK_SETTINGS_FILE=$settingsFile
    $env:TAKEDOCK_FIXTURE_EXE=Join-Path $metadata.target_directory 'debug/takedock-adb-fixture.exe'
    $env:TAKEDOCK_APP_EXE=[IO.Path]::GetFullPath($Executable)
    $driverSocket=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$driverSocket.Start();$driverPort=$driverSocket.LocalEndpoint.Port;$driverSocket.Stop()
    $nativeSocket=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0);$nativeSocket.Start();$nativePort=$nativeSocket.LocalEndpoint.Port;$nativeSocket.Stop()
    $env:TAKEDOCK_DRIVER_PORT="$driverPort"
    $driverProcess=Start-Process -FilePath $TauriDriver -ArgumentList @('--port',"$driverPort",'--native-port',"$nativePort",'--native-driver',('"'+$EdgeDriver+'"')) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $testRoot 'driver.log') -RedirectStandardError (Join-Path $testRoot 'driver-error.log')
    $deadline=(Get-Date).AddSeconds(15)
    do {
        try{$ready=Invoke-RestMethod "http://127.0.0.1:$driverPort/status" -TimeoutSec 1}catch{$ready=$null}
        if($driverProcess.HasExited){throw 'Tauri driver exited during startup'}
        if((Get-Date) -gt $deadline){throw 'Tauri driver did not become ready'}
        if(-not $ready){Start-Sleep -Milliseconds 100}
    }while(-not $ready)
    $commit=if($env:TAKEDOCK_ACCEPTANCE_COMMIT){$env:TAKEDOCK_ACCEPTANCE_COMMIT}else{& git rev-parse HEAD}
    $report=[ordered]@{commit=$commit;os=[Environment]::OSVersion.Version.ToString();node=(& node --version);webview2=$runtimeVersion;edgeDriver=$edgeVersion;tauriDriver='2.1.0';resources=@(@($Executable)+$resources|ForEach-Object {@{name=[IO.Path]::GetRelativePath($resourceRoot,$_).Replace('\','/');sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash}})}
    $report|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $testRoot 'preflight.json')
    & node node_modules/@wdio/cli/bin/wdio.js run tests/desktop/wdio.conf.ts
    if($LASTEXITCODE -ne 0){throw "Desktop E2E failed; fixture diagnostics: $testRoot"}
    $tests=@(Get-Content -LiteralPath (Join-Path $testRoot 'results.jsonl')|ForEach-Object {$_|ConvertFrom-Json})
    if($tests.Count -ne 4 -or @($tests|Where-Object {-not $_.passed}).Count){throw 'Four passing cases required; skips fail'}
    $report.tests=$tests;$report.conclusion='passed'
    New-Item -ItemType Directory -Path 'test-results/desktop' -Force|Out-Null
    $report|ConvertTo-Json -Depth 10|Set-Content -LiteralPath 'test-results/desktop/acceptance.json'
    Write-Output 'PASS: all four release Desktop E2E cases.'
} finally {
    if($driverProcess -and -not $driverProcess.HasExited){Stop-Process -Id $driverProcess.Id -ErrorAction SilentlyContinue}
    if($profilePrepared){
        Get-CimInstance Win32_Process -Filter "Name = 'takedock.exe'" | Where-Object {$_.ExecutablePath -eq $env:TAKEDOCK_APP_EXE} | ForEach-Object {Stop-Process -Id $_.ProcessId -ErrorAction SilentlyContinue}
        Get-CimInstance Win32_Process -Filter "Name = 'adb.exe'" | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.StartsWith($testRoot+[IO.Path]::DirectorySeparatorChar)} | ForEach-Object {Stop-Process -Id $_.ProcessId -ErrorAction SilentlyContinue}
        if($null -ne $originalSettings){[IO.File]::WriteAllBytes($settingsFile,$originalSettings)}elseif(Test-Path -LiteralPath $settingsFile){Remove-Item -LiteralPath $settingsFile}
    }
    Pop-Location
}
