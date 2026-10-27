param(
    [Parameter(Mandatory)][string]$TrialRoot,
    [Parameter(Mandatory)][string]$Builds,
    [Parameter(Mandatory)][ValidateSet('Baseline','Compare')][string]$Stage,
    [Parameter(Mandatory)][string]$AdbPath,
    [Parameter(Mandatory)][string]$TauriDriver,
    [Parameter(Mandatory)][string]$EdgeDriver,
    [Parameter(Mandatory)][string]$FfmpegPath,
    [Parameter(Mandatory)][string]$FfprobePath,
    [string]$Manifest,
    [string]$Serial
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
Import-Module (Join-Path $PSScriptRoot 'transfer-benchmark/Core.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'transfer-benchmark/Device.psm1') -Force -DisableNameChecking
Import-Module (Join-Path $PSScriptRoot 'transfer-benchmark/Ui.psm1') -Force -DisableNameChecking
Import-Module (Join-Path $PSScriptRoot 'transfer-benchmark/Stage.psm1') -Force -DisableNameChecking

function Assert-BenchmarkBuild([hashtable]$Build) {
    if($Build.revision -notmatch '^[a-f0-9]{40}$' -or $Build.sha256 -notmatch '^[a-f0-9]{64}$'){throw 'An exact measured build revision/digest is required.'}
    if(-not [IO.Path]::IsPathFullyQualified($Build.exe) -or (Get-FileHash -LiteralPath $Build.exe -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Build.sha256){throw 'Measured executable differs from frozen artifact.'}
    $root=Split-Path $Build.exe -Parent
    foreach($resource in $Build.resources) {
        $path=Assert-TransferLocalPath -Root $root -Path (Join-Path $root $resource.path)
        if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $resource.sha256){throw 'Measured resource differs from frozen artifact.'}
    }
    foreach($required in @('observer.apk','file-helper/arm64-v8a/takedock-files','file-helper/armeabi-v7a/takedock-files','file-helper/x86_64/takedock-files')) {
        if(@($Build.resources | Where-Object path -CEQ $required).Count -ne 1){throw 'Incomplete frozen resource set.'}
    }
}
function New-DriverPort {
    $socket=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0)
    try {$socket.Start();return $socket.LocalEndpoint.Port} finally {$socket.Stop()}
}
function Start-BenchmarkDriver([string]$RunRoot,[AllowNull()][string]$TracePath) {
    $port=New-DriverPort;$nativePort=New-DriverPort
    $info=[Diagnostics.ProcessStartInfo]::new($TauriDriver)
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    foreach($arg in @('--port',[string]$port,'--native-port',[string]$nativePort,'--native-driver',$EdgeDriver)){$info.ArgumentList.Add($arg)}
    if($TracePath){$info.Environment['TAKEDOCK_TRANSFER_TRACE']=$TracePath}
    else {$null=$info.Environment.Remove('TAKEDOCK_TRANSFER_TRACE')}
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    $process=[Diagnostics.Process]::new();$process.StartInfo=$info
    if(-not $process.Start()){throw 'Native driver failed to start.'}
    $logs=@($process.StandardOutput.ReadToEndAsync(),$process.StandardError.ReadToEndAsync())
    $watch=[Diagnostics.Stopwatch]::StartNew()
    $uri="http://127.0.0.1:$port"
    try {
        do {
            $ready=$false
            try {$ready=(Invoke-RestMethod -Uri "$uri/status" -TimeoutSec 1).value.ready} catch {}
            if($ready){return @{process=$process;uri=[uri]$uri;logs=$logs;root=$RunRoot}}
            if($process.HasExited -or $watch.Elapsed.TotalSeconds -gt 15){throw 'Native driver did not become ready.'}
            Start-Sleep -Milliseconds 100
        }while($true)
    } catch {
        if(-not $process.HasExited){$process.Kill();$process.WaitForExit()}
        $process.Dispose();throw
    }
}
function Stop-BenchmarkDriver([hashtable]$Driver) {
    if(-not $Driver.process.HasExited){$Driver.process.Kill();$Driver.process.WaitForExit()}
    try {
        for($i=0;$i -lt 2;$i++) {
            $path=Join-Path $Driver.root "driver-$i.log"
            [IO.File]::WriteAllText($path,$Driver.logs[$i].GetAwaiter().GetResult())
        }
    } finally {$Driver.process.Dispose()}
}
function Read-CompletedTrace([string]$Path,[string]$JobId) {
    $watch=[Diagnostics.Stopwatch]::StartNew()
    do {
        try {return Read-TransferTrace -Path $Path -JobId $JobId}
        catch {if($watch.Elapsed.TotalSeconds -gt 10){throw};Start-Sleep -Milliseconds 100}
    }while($true)
}
function Invoke-BenchmarkRun([string]$BuildName,[long]$Bytes,[string]$Kind,[int]$Pair,[int]$Order,[bool]$Warmup) {
    $build=$script:BuildInfo[$BuildName]
    Assert-BenchmarkBuild $build
    $events=@(Get-Content -LiteralPath $script:Trial.journal | ForEach-Object {$_ | ConvertFrom-Json -AsHashtable -DateKind String})
    $receiptStage=if($Stage -eq 'Compare'){'Compare'}else{''}
    $saved=Get-TransferCompletedRun -Events $events -Build $BuildName -Sha256 $build.sha256 -Bytes $Bytes -Kind $Kind -Pair $Pair -Order $Order -Warmup $Warmup -Stage $receiptStage
    if($null -ne $saved){Register-TransferStageRun -Attempt $script:StageAttempt -RunId $saved.run_id;Write-Host "Reusing journal-verified $BuildName $Kind $Bytes-byte result; no operation repeated.";return $saved}
    Assert-TransferTrialSpace -Trial $script:Trial
    $runId=[guid]::NewGuid().ToString('N')
    Register-TransferStageRun -Attempt $script:StageAttempt -RunId $runId
    $master=@($script:Trial.fixtures | Where-Object {$_.role -eq 'master' -and [long]$_.bytes -eq $Bytes})
    if($master.Count -ne 1){throw 'Owned master payload is missing/ambiguous.'}
    $fixture=$master[0]
    $remoteName=if($Kind -eq 'move'){"TDPerf-$($script:Trial.trial_id)-move-$runId.mp4"}else{$fixture.remote_name}
    $paths=Get-TransferRunPaths -TrialRoot $TrialRoot -RunId $runId -RemoteName $remoteName
    $runRoot=$paths.root;$destination=$paths.destination
    $null=New-Item -ItemType Directory -Path $runRoot
    $null=New-Item -ItemType Directory -Path $destination
    if($Kind -eq 'move') {
        $fixture=Add-TransferPhoneFixture -Trial $script:Trial -Id "move-$runId" -Backup $fixture.backup -Role move_duplicate -FfmpegPath $FfmpegPath -SourceFixtureId $master[0].id
        $script:Trial | ConvertTo-Json -Depth 25 | Set-Content -LiteralPath $Manifest
    }
    $run=@{run_id=$runId;stage=$Stage;stage_attempt_id=$script:StageAttempt.id;build=$BuildName;bytes=[string]$Bytes;kind=$Kind;pair=$Pair;order=$Order;warmup=$Warmup;fixture_id=$fixture.id;destination=$destination}
    Get-TransferConditions -Trial $script:Trial | ConvertTo-Json -Depth 15 | Set-Content -LiteralPath (Join-Path $runRoot 'conditions-before.json')
    Write-TransferJournal -Trial $script:Trial -Entry @{event='run_planned';run=$run;build_revision=$build.revision;build_sha256=$build.sha256}
    $settingsPath=Join-Path $env:APPDATA 'app.takedock/settings.json'
    $before=if(Test-Path -LiteralPath $settingsPath){[IO.File]::ReadAllBytes($settingsPath)}else{$null}
    if($null -eq $before){throw 'Existing TakeDock profile required; preserve initial setup separately.'}
    [IO.File]::WriteAllBytes((Join-Path $runRoot 'profile-before.bin'),$before)
    $settings=[Text.Encoding]::UTF8.GetString($before) | ConvertFrom-Json -AsHashtable
    $settings.language='en';$settings.destination=$destination;$settings.adb_path=$AdbPath
    $last=[Text.Encoding]::UTF8.GetBytes(($settings | ConvertTo-Json -Depth 20))
    $tracePath=if($BuildName -eq 'released'){$null}else{Join-Path $runRoot 'trace.jsonl'}
    $driver=$null;$session=$null;$prepared=$false
    try {
        if(Get-Process takedock -ErrorAction SilentlyContinue){throw 'An existing TakeDock session must be closed before this trial.'}
        New-Item -ItemType Directory -Path (Split-Path $settingsPath -Parent) -Force | Out-Null
        [IO.File]::WriteAllBytes($settingsPath,$last);$prepared=$true
        $driver=Start-BenchmarkDriver -RunRoot $runRoot -TracePath $tracePath
        $session=Open-TransferSession -Executable $build.exe -DriverUri $driver.uri
        $ui=Invoke-TransferRun -Session $session -Trial $script:Trial -Run $run
        $trace=if($tracePath){Read-CompletedTrace -Path $tracePath -JobId $ui.job_id}else{$null}
        $result=Join-TransferRun -UiResult $ui -Trace $trace
        $result.stage=$Stage
        $output=Assert-TransferLocalPath -Root $TrialRoot -Path (Join-Path $destination $fixture.remote_name)
        if((Get-Item -LiteralPath $output).Length -ne $Bytes -or (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant() -cne $fixture.sha256){throw 'Published output failed independent verification.'}
        if($Kind -eq 'copy'){$null=Assert-TransferSource -Trial $script:Trial -FixtureId $fixture.id}
        else {
            $remote="'/sdcard/DCIM/OpenCamera/$($fixture.remote_name)'"
            $null=Invoke-TransferAdb -Trial $script:Trial -Arguments @('shell','-T',"test ! -e $remote && test ! -L $remote")
        }
        $result | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $runRoot 'result.json')
        Get-TransferConditions -Trial $script:Trial | ConvertTo-Json -Depth 15 | Set-Content -LiteralPath (Join-Path $runRoot 'conditions-after.json')
        Write-TransferJournal -Trial $script:Trial -Entry @{event='run_verified';result=$result;output_sha256=$fixture.sha256}
        # The immutable backup is retained; only this newly published owned copy is temporary.
        Remove-Item -LiteralPath $output
        Write-Host ("Recorded {0} {1} {2} bytes: UI {3:N1} ms, backend {4} ms; warmup={5}" -f $BuildName,$Kind,$Bytes,$result.ui_ms,$result.backend_ms,$Warmup)
        return $result
    } catch {
        Write-TransferJournal -Trial $script:Trial -Entry @{event='run_invalid';run=$run;reason=$_.Exception.Message}
        throw
    } finally {
        try {Complete-TransferTrial -Trial $script:Trial}
        finally {
            if($session){try {Close-TransferSession -Session $session} catch {Write-TransferJournal -Trial $script:Trial -Entry @{event='session_close_failed';reason=$_.Exception.Message}}}
            if($driver){Stop-BenchmarkDriver $driver}
            if($prepared -and $driver) {
                foreach($owned in @(Get-CimInstance Win32_Process -Filter "Name = 'takedock.exe'" | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.Equals($build.exe,[StringComparison]::OrdinalIgnoreCase)})) {
                    Stop-Process -Id $owned.ProcessId -ErrorAction SilentlyContinue
                }
            }
            if($prepared) {
                try {
                    Restore-TransferProfile -Path $settingsPath -Before $before -LastWritten $last
                    Write-TransferJournal -Trial $script:Trial -Entry @{event='run_cleanup_complete';run_id=$runId}
                }
                catch {
                    if(Test-Path -LiteralPath $settingsPath){Copy-Item -LiteralPath $settingsPath -Destination (Join-Path $runRoot 'profile-unexpected.bin')}
                    throw
                }
            }
        }
    }
}

$TrialRoot=[IO.Path]::GetFullPath($TrialRoot)
$allowed=Join-Path $projectRoot 'test-results/transfer-performance'
$null=Assert-TransferLocalPath -Root $allowed -Path $TrialRoot
if(-not $Manifest){$Manifest=Join-Path $TrialRoot 'manifest.json'}
$null=Assert-TransferLocalPath -Root $TrialRoot -Path $Manifest
foreach($tool in @($AdbPath,$TauriDriver,$EdgeDriver,$FfmpegPath,$FfprobePath)) {
    if(-not [IO.Path]::IsPathFullyQualified($tool) -or -not (Test-Path -LiteralPath $tool -PathType Leaf)){throw 'All benchmark tools must resolve to existing absolute executables.'}
}
if(-not $IsWindows){throw 'The native UI benchmark requires Windows.'}
if(-not [Diagnostics.Stopwatch]::IsHighResolution){throw 'High resolution host clock unavailable.'}
Assert-TransferDriverPin -Executable $TauriDriver
$runtime=@(Get-Item 'C:/Program Files (x86)/Microsoft/EdgeWebView/Application/*/msedgewebview2.exe' | ForEach-Object {$_.VersionInfo.ProductVersion}) | Sort-Object {[version]$_} -Descending | Select-Object -First 1
$edgeVersion=& $EdgeDriver --version
if($LASTEXITCODE -ne 0 -or $edgeVersion -notmatch [regex]::Escape($runtime)){throw 'EdgeDriver must match installed WebView2.'}
$script:BuildInfo=Get-Content -LiteralPath $Builds -Raw | ConvertFrom-Json -AsHashtable
if($script:BuildInfo.schema -ne 1){throw 'Unknown frozen build schema.'}
$buildNames=if($Stage -eq 'Baseline'){@('released','baseline')}else{@('baseline','candidate')}
foreach($name in $buildNames){Assert-BenchmarkBuild $script:BuildInfo[$name]}
foreach($resource in $script:BuildInfo.baseline.resources) {
    if($resource.path.StartsWith('file-helper/',[StringComparison]::Ordinal)) {
        foreach($name in $buildNames) {
            $matching=@($script:BuildInfo[$name].resources | Where-Object path -CEQ $resource.path)
            if($matching.Count -ne 1 -or $matching[0].sha256 -cne $resource.sha256){throw 'Helper bytes differ between compared builds.'}
        }
    }
}
if(-not $Serial) {
    $devices=& $AdbPath devices
    if($LASTEXITCODE -ne 0){throw 'ADB inventory unavailable.'}
    $connected=@($devices | Where-Object {$_ -match '^\S+\s+device$'})
    if($connected.Count -ne 1){throw 'Exactly one authorized available phone required.'}
    $Serial=($connected[0] -split '\s+')[0]
}
$script:Trial=$null
$script:StageAttempt=$null
$comparisonReport=$null
try {
    if(Test-Path -LiteralPath $Manifest) {$script:Trial=Read-TransferTrial -Manifest $Manifest -TrialRoot $TrialRoot}
    else {
        $null=New-Item -ItemType Directory -Path $TrialRoot -Force
        $seeds=@()
        for($i=0;$i -lt 2;$i++) {
            $seed=Join-Path $TrialRoot "seed-$i.mp4"
            & $FfmpegPath -nostdin -v error -f lavfi -i 'testsrc2=size=320x240:rate=10' -t 1 -c:v libx264 -pix_fmt yuv420p -movflags +faststart $seed
            if($LASTEXITCODE -ne 0){throw 'New owned seed generation failed.'}
            $probe=& $FfprobePath -v error -show_entries format=duration -of 'default=noprint_wrappers=1:nokey=1' $seed
            if($LASTEXITCODE -ne 0 -or [double]::Parse($probe,[Globalization.CultureInfo]::InvariantCulture) -le 0){throw 'Seed did not finalize.'}
            $seeds+=,$seed
        }
        $script:Trial=Prepare-TransferTrial -TrialRoot $TrialRoot -AdbPath $AdbPath -Serial $Serial -Seeds $seeds -Resources (Split-Path $script:BuildInfo.baseline.exe -Parent) -FfmpegPath $FfmpegPath
    }
    if($script:Trial.serial -cne $Serial -or $script:Trial.adb_path -cne $AdbPath){throw 'Existing trial belongs to another device/ADB selection.'}
    $calibration=if($Stage -ceq 'Compare'){Read-TransferCalibration -TrialRoot $TrialRoot -Path (Join-Path $TrialRoot 'baseline.json')}else{$null}
    $stageAccessibility=Get-TransferAccessibility -Trial $script:Trial
    $script:StageAttempt=New-TransferStageAttempt -Trial $script:Trial -Stage $Stage -Accessibility $stageAccessibility
    $results=[Collections.Generic.List[object]]::new()
    $futility=@{stop=$false;matched_pairs=0;failures=@()}
    $pairs=if($Stage -eq 'Baseline'){1}else{3}
    :matrix foreach($bytes in @(500000000,1000000000)) {
        foreach($kind in @('copy','move')) {
            foreach($name in $buildNames){$results.Add((Invoke-BenchmarkRun -BuildName $name -Bytes $bytes -Kind $kind -Pair 0 -Order 0 -Warmup $true))}
            for($pair=1;$pair -le $pairs;$pair++) {
                $order=if($pair % 2){$buildNames}else{@($buildNames[1],$buildNames[0])}
                for($i=0;$i -lt 2;$i++) {$results.Add((Invoke-BenchmarkRun -BuildName $order[$i] -Bytes $bytes -Kind $kind -Pair $pair -Order $i -Warmup $false))}
                if($Stage -eq 'Compare') {
                    $futility=Get-TransferFutility -Runs $results.ToArray()
                    if($futility.stop) {
                        Write-TransferJournal -Trial $script:Trial -Entry @{event='comparison_futility_stop';decision=$futility}
                        Write-Host 'Candidate rejected by a retained non-improving pair; remaining comparison cells will be reported unrun.'
                        break matrix
                    }
                }
            }
        }
    }
    if($Stage -eq 'Baseline') {
        $uncertainty=0.0
        foreach($bytes in @('500000000','1000000000')) {
            foreach($kind in @('copy','move')) {
                $group=@($results | Where-Object {-not $_.warmup -and $_.bytes -eq $bytes -and $_.kind -eq $kind})
                $uncertainty=[math]::Max($uncertainty,[math]::Abs($group[0].ui_ms-$group[1].ui_ms))
            }
        }
        $report=@{stage='baseline';instrumentation_uncertainty_ms=$uncertainty;runs=$results.ToArray()}
        Set-TransferStageReport -Attempt $script:StageAttempt -Report $report
    } else {
        $verdict=Get-TransferVerdict -Runs $results.ToArray() -UncertaintyMs $calibration.instrumentation_uncertainty_ms
        :extra foreach($group in @($verdict.groups | Where-Object {-not $_.accepted})) {
            if($futility.stop){break}
            if($group.metrics.Count -eq 0){continue}
            if(@($group.metrics.Values | ForEach-Object {$_.paired_differences} | Where-Object {$_ -le 0}).Count){continue}
            foreach($pair in @(4,5)) {
                $order=if($pair % 2){$buildNames}else{@($buildNames[1],$buildNames[0])}
                for($i=0;$i -lt 2;$i++){$results.Add((Invoke-BenchmarkRun -BuildName $order[$i] -Bytes ([long]$group.bytes) -Kind $group.kind -Pair $pair -Order $i -Warmup $false))}
                $futility=Get-TransferFutility -Runs $results.ToArray()
                if($futility.stop){Write-TransferJournal -Trial $script:Trial -Entry @{event='comparison_futility_stop';decision=$futility};break extra}
            }
        }
        $verdict=Get-TransferVerdict -Runs $results.ToArray() -UncertaintyMs $calibration.instrumentation_uncertainty_ms
        if($futility.stop -and $verdict.accepted){throw 'Futility must never accept an incomplete or non-improving comparison.'}
        $unrun=@(foreach($bytes in @('500000000','1000000000')) {
            foreach($kind in @('copy','move')) {
                foreach($name in $buildNames) {
                    foreach($pair in @(0,1,2,3)) {
                        $warmup=($pair -eq 0)
                        if(@($results | Where-Object {$_.bytes -ceq $bytes -and $_.kind -ceq $kind -and $_.build -ceq $name -and $_.pair -eq $pair -and $_.warmup -eq $warmup}).Count -eq 0){@{bytes=$bytes;kind=$kind;build=$name;pair=$pair;warmup=$warmup;status='unrun'}}
                    }
                }
            }
        })
        $decision=if($futility.stop){'rejected_futility'}elseif($verdict.accepted){'accepted'}else{'rejected'}
        $comparisonReport=@{stage='compare';stage_valid=$false;stage_invalid_reason='Preservation checks pending.';decision=$decision;futility=$futility;unrun_cells=$unrun;verdict=$verdict;runs=$results.ToArray();accessibility_baseline=$stageAccessibility}
        Set-TransferStageReport -Attempt $script:StageAttempt -Report $comparisonReport
    }
    Assert-TransferUnrelatedPreserved -Trial $script:Trial
    Assert-TransferAccessibilityPreserved -Trial $script:Trial -Before $stageAccessibility
} catch {
    if($script:StageAttempt){Close-TransferStageAttempt -Trial $script:Trial -Attempt $script:StageAttempt -Valid $false -Reason $_.Exception.Message}
    throw
} finally {
    if($script:Trial) {
        try {Complete-TransferTrial -Trial $script:Trial}
        catch {
            if($script:StageAttempt){Close-TransferStageAttempt -Trial $script:Trial -Attempt $script:StageAttempt -Valid $false -Reason $_.Exception.Message}
            throw
        }
    }
}
if($script:StageAttempt){Close-TransferStageAttempt -Trial $script:Trial -Attempt $script:StageAttempt -Valid $true}
Write-Output "PASS: $Stage trial recorded privately; screen cleanup and pairing preservation confirmed."
