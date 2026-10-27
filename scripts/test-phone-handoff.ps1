param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Adb,
    [Parameter(Mandatory)][string]$Serial,
    [Parameter(Mandatory)][string]$Manifest,
    [Parameter(Mandatory)][string]$TrialRoot,
    [Parameter(Mandatory)][string]$TauriDriver,
    [Parameter(Mandatory)][string]$EdgeDriver
)
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
Import-Module (Join-Path $PSScriptRoot 'phone-handoff/Guard.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'transfer-benchmark/Core.psm1') -Force
$device=Import-Module (Join-Path $PSScriptRoot 'transfer-benchmark/Device.psm1') -Force -PassThru -DisableNameChecking
$native=Import-Module (Join-Path $PSScriptRoot 'transfer-benchmark/Ui.psm1') -Force -PassThru -DisableNameChecking
$TrialRoot=[IO.Path]::GetFullPath($TrialRoot)
$null=Assert-TransferLocalPath -Root (Join-Path $projectRoot 'test-results/phone-handoff') -Path $TrialRoot
$trial=Read-HandoffManifest -Manifest $Manifest -TrialRoot $TrialRoot
if(-not $IsWindows -or -not [Diagnostics.Stopwatch]::IsHighResolution){throw 'Native Windows/high-resolution clock required.'}
foreach($tool in @($Executable,$Adb,$TauriDriver,$EdgeDriver,$trial.ffmpeg)) {
    if(-not [IO.Path]::IsPathFullyQualified($tool) -or -not (Test-Path -LiteralPath $tool -PathType Leaf)){throw 'Existing absolute tool paths required.'}
}
if($Serial -cne $trial.serial -or $trial.revision -notmatch '^[a-f0-9]{40}$'){throw 'Frozen source/phone provenance required.'}
if((Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant() -cne $trial.executable_sha256){throw 'Executable changed since freezing.'}
if((Get-FileHash -LiteralPath $Adb -Algorithm SHA256).Hash.ToLowerInvariant() -cne $trial.adb_sha256){throw 'SDK executable changed.'}
$sdkProperties=Join-Path (Split-Path $Adb -Parent) 'source.properties'
if(-not (Test-Path -LiteralPath $sdkProperties) -or (Get-Content -LiteralPath $sdkProperties -Raw) -notmatch 'Pkg.Revision\s*=\s*37\.0\.1'){throw 'Verified real Android SDK 37.0.1 required.'}
Assert-TransferDriverPin -Executable $TauriDriver
$exeRoot=Split-Path $Executable -Parent
foreach($resource in $trial.resources) {
    $path=Assert-TransferLocalPath -Root $exeRoot -Path (Join-Path $exeRoot $resource.path)
    if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $resource.sha256){throw 'Frozen native resources changed.'}
}
foreach($required in @('observer.apk','file-helper/arm64-v8a/takedock-files','file-helper/x86_64/takedock-files','file-helper/armeabi-v7a/takedock-files')) {
    if(@($trial.resources | Where-Object path -CEQ $required).Count -ne 1){throw 'Complete frozen resource set required.'}
}
$trial.adb_path=$Adb
$trial.journal=Join-Path $TrialRoot 'journal.jsonl'
if(Test-Path -LiteralPath $trial.journal){throw 'This trial has already started. Preserve its journal; never retry its mutations.'}
$trial.helper_path='/data/local/tmp/takedock-files-'+@($trial.resources | Where-Object path -CEQ 'file-helper/arm64-v8a/takedock-files')[0].sha256
$realProfile=Join-Path $env:APPDATA 'app.takedock/settings.json'
$profileBefore=if(Test-Path -LiteralPath $realProfile){(Get-FileHash -LiteralPath $realProfile -Algorithm SHA256).Hash}else{''}
$isolated=Join-Path $TrialRoot 'profile'
$profilePath=Join-Path $isolated 'app.takedock/settings.json'
$null=New-Item -ItemType Directory -Path (Split-Path $profilePath -Parent)
$preferences=if($profileBefore){Get-Content -LiteralPath $realProfile -Raw | ConvertFrom-Json -AsHashtable}else{@{}}
$settings=@{language='en';adb_path=$Adb;success_sound=$false;ready_sound=$false;ready_delay_ms=800;verification_ms=10000}
foreach($key in @('ready_delay_ms','verification_ms')){if($preferences.ContainsKey($key)){$settings[$key]=$preferences[$key]}}
$settings | ConvertTo-Json | Set-Content -LiteralPath $profilePath
$driver=$null;$session=$null;$failure=$null
$results=[Collections.Generic.List[object]]::new()
$settled=[Collections.Generic.List[object]]::new()
function Journal([hashtable]$Entry){Write-TransferJournal -Trial $trial -Entry $Entry}
function PhoneQuote([string]$Value){& $device {param($v) Quote-TransferShell $v} $Value}
function Request([string]$Method,[string]$Path,[AllowNull()][object]$Body=$null) {
    & $native {param($s,$m,$p,$b) Invoke-TransferDriver -Session $s -Method $m -Path $p -Body $b} $session $Method $Path $Body
}
function Evaluate([string]$Script,[object[]]$Arguments=@()) {Request Post '/execute/sync' @{script=$Script;args=$Arguments}}
function Bootstrap {Evaluate 'return window.__TAURI_INTERNALS__.invoke("bootstrap")'}
function Videos {param([long]$Generation) Evaluate 'return window.__TAURI_INTERNALS__.invoke("list_videos", {generation:arguments[0]})' @($Generation)}
function Element([string]$XPath) {
    $value=Request Post '/element' @{using='xpath';value=$XPath}
    return $value['element-6066-11e4-a52e-4f735466cecf']
}
function RowControl([string]$Name,[string]$Selector) {
    $value=Evaluate 'return [...document.querySelectorAll("tbody tr")].find(row=>row.querySelector("td:nth-child(2) bdi")?.textContent===arguments[0])?.querySelector(arguments[1]) ?? null' @($Name,$Selector)
    if(-not $value){throw "Owned source control missing: $Name"}
    return $value['element-6066-11e4-a52e-4f735466cecf']
}
function Click([string]$Id){$null=Request Post "/element/$Id/click" @{}}
function Button([string]$Name){Element ('//button[normalize-space(.)="'+$Name+'"]')}
function Wait-Until([scriptblock]$Predicate,[string]$Reason,[int]$Seconds=30) {
    $watch=[Diagnostics.Stopwatch]::StartNew()
    do {if(& $Predicate){return};if($watch.Elapsed.TotalSeconds -gt $Seconds){throw $Reason};Start-Sleep -Milliseconds 125}while($true)
}
function RemoteHash([string]$Root,[string]$Name) {
    $path=PhoneQuote ($Root+'/'+$Name)
    $output=Invoke-TransferAdb -Trial $trial -Arguments @('shell','-T',"sha256sum -- $path") -TimeoutSeconds 120
    $hash=($output.Trim() -split '\s+')[0]
    if($hash -notmatch '^[a-f0-9]{64}$'){throw 'Owned phone digest unavailable.'}
    return $hash
}
function Assert-Source([hashtable]$Fixture,[object[]]$Listing) {
    $sourceRows=@($Listing | Where-Object name -CEQ $Fixture.name)
    if($sourceRows.Count -ne 1 -or -not $sourceRows[0].ready -or $sourceRows[0].identity -cne $Fixture.identity){throw 'Owned source is missing, unfinished or replaced.'}
    if((RemoteHash $trial.source_root $Fixture.name) -cne $Fixture.sha256){throw 'Phone source differs from its verified reference.'}
}
function New-Port {
    $socket=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0)
    try {$socket.Start();return $socket.LocalEndpoint.Port} finally {$socket.Stop()}
}
function Open-Native {
    if(Get-Process takedock -ErrorAction SilentlyContinue){throw 'An existing application session must be closed before this owned trial.'}
    $port=New-Port;$nativePort=New-Port
    $info=[Diagnostics.ProcessStartInfo]::new($TauriDriver)
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach($argument in @('--port',[string]$port,'--native-port',[string]$nativePort,'--native-driver',$EdgeDriver)){$info.ArgumentList.Add($argument)}
    $info.Environment['APPDATA']=$isolated
    $info.Environment['LOCALAPPDATA']=Join-Path $TrialRoot 'local-profile'
    if($trial.configuration -ceq 'desktop,transfer-diagnostics'){$info.Environment['TAKEDOCK_TRANSFER_TRACE']=Join-Path $TrialRoot 'trace.jsonl'}
    else {$null=$info.Environment.Remove('TAKEDOCK_TRANSFER_TRACE')}
    $process=[Diagnostics.Process]::new();$process.StartInfo=$info
    if(-not $process.Start()){throw 'Owned native driver failed to launch.'}
    $script:driver=@{process=$process;logs=@($process.StandardOutput.ReadToEndAsync(),$process.StandardError.ReadToEndAsync());port=$port}
    Wait-Until {try {(Invoke-RestMethod -Uri "http://127.0.0.1:$port/status" -TimeoutSec 1).value.ready} catch {$false}} 'Native driver unavailable.'
    $script:session=Open-TransferSession -Executable $Executable -DriverUri ([uri]"http://127.0.0.1:$port")
    Wait-Until {(Bootstrap).session.connected} 'Physical phone session unavailable.'
    Wait-Until {try {[bool](Button 'Videos')} catch {$false}} 'Native source-navigation control unavailable.'
    Click (Button 'Videos')
    Click (Button 'Refresh videos')
    Wait-Until {-not (Evaluate 'return !![...document.querySelectorAll("section p")].find(p=>p.textContent==="Loading…")')} 'Source listing did not settle.'
}
function Close-Native {
    $closeFailure=$null
    try {if($session){Close-TransferSession -Session $session}} catch {$closeFailure=$_}
    finally {
        $script:session=$null
        foreach($owned in @(Get-CimInstance Win32_Process -Filter "Name = 'takedock.exe'" | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.Equals($Executable,[StringComparison]::OrdinalIgnoreCase)})) {
            Stop-Process -Id $owned.ProcessId -ErrorAction SilentlyContinue
        }
        if($driver){
            try {
                foreach($child in @(Get-CimInstance Win32_Process -Filter "Name = 'msedgedriver.exe'" | Where-Object ParentProcessId -EQ $driver.process.Id)) {Stop-Process -Id $child.ProcessId -ErrorAction SilentlyContinue}
                # Shared ADB is never part of a recursive process-tree kill.
                if(-not $driver.process.HasExited){$driver.process.Kill();$driver.process.WaitForExit()}
                for($i=0;$i -lt 2;$i++){[IO.File]::AppendAllText((Join-Path $TrialRoot "driver-$i.log"),$driver.logs[$i].GetAwaiter().GetResult())}
            } finally {$driver.process.Dispose();$script:driver=$null}
        }
    }
    if($closeFailure){throw $closeFailure}
}
try {
    Journal @{event='trial_started';revision=$trial.revision;executable_sha256=$trial.executable_sha256;configuration=$trial.configuration;manifest_sha256=(Get-FileHash -LiteralPath $Manifest -Algorithm SHA256).Hash}
    if((Invoke-TransferAdb -Trial $trial -Arguments @('shell','getprop','ro.product.model')).Trim() -cne 'SM-A155F'){throw 'Unexpected physical phone model.'}
    $null=Invoke-TransferAdb -Trial $trial -Arguments @('shell','input','keyevent','KEYCODE_HOME')
    $null=Invoke-TransferAdb -Trial $trial -Arguments @('shell','input','keyevent','KEYCODE_SLEEP')
    foreach($fixture in $trial.fixtures) {
        if((Get-FileHash -LiteralPath $fixture.backup -Algorithm SHA256).Hash.ToLowerInvariant() -cne $fixture.sha256){throw 'Reference backup digest changed.'}
        & $trial.ffmpeg -nostdin -v error -xerror -i $fixture.backup -f null - 2>> (Join-Path $TrialRoot 'reference-decode.log')
        if($LASTEXITCODE -ne 0){throw 'Reference backup did not fully decode.'}
    }
    Open-Native
    $baselineGeneration=(Bootstrap).session.generation
    $baseline=@(Videos $baselineGeneration)
    $ownedNames=@($trial.fixtures | ForEach-Object name)
    $unrelated=@($baseline | Where-Object {$_.name -cnotin $ownedNames})
    $unrelated | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $TrialRoot 'unrelated-source-before.json')
    foreach($fixture in $trial.fixtures){Assert-Source $fixture $baseline}
    foreach($operation in $trial.operations) {
        if($operation.restart_before){Close-Native;Open-Native}
        $initial=Bootstrap
        if(@($initial.jobs | Where-Object {$_.status -in @('queued','running')}).Count){throw 'An unrelated active job prevents a measured action.'}
        $selected=@($operation.fixtures | ForEach-Object {$id=$_;@($trial.fixtures | Where-Object id -CEQ $id)[0]})
        $listing=@(Videos $initial.session.generation)
        foreach($fixture in $selected){Assert-Source $fixture $listing}
        if($operation.kind -eq 'collision') {
            $target=PhoneQuote ($trial.handoff_root+'/'+$selected[0].name)
            $payload=PhoneQuote ('TDHandoff-'+$trial.trial_id+'-collision')
            Journal @{event='collision_fixture_planned';name=$selected[0].name;sha256=$selected[0].collision_sha256}
            $null=Invoke-TransferAdb -Trial $trial -Arguments @('shell','-T',"set -C; printf %s $payload > $target")
            if((RemoteHash $trial.handoff_root $selected[0].name) -cne $selected[0].collision_sha256){throw 'Exact owned collision fixture could not be prepared.'}
        }
        Click (Button 'Refresh videos')
        Wait-Until {-not (Evaluate 'return !![...document.querySelectorAll("section p")].find(p=>p.textContent==="Loading…")')} 'Source refresh did not settle.'
        if($selected.Count -eq 1 -and $operation.kind -in @('mark','collision')) {
            $action=RowControl $selected[0].name 'button[data-mark]'
        } else {
            if(Evaluate 'return !![...document.querySelectorAll("button")].find(b=>b.textContent.trim()==="Clear selection")'){Click (Button 'Clear selection')}
            foreach($fixture in $selected){Click (RowControl $fixture.name 'input')}
            if((Evaluate 'return document.querySelectorAll("tbody input:checked").length') -ne $selected.Count){throw 'Unexpected native selection.'}
            switch($operation.kind) {
                mark {$action=Button 'Mark selected'}
                rename {
                    Click (Button 'Rename video');$renameElement=Element '//input[@id="new-name"]'
                    $null=Request Post "/element/$renameElement/clear" @{}
                    $null=Request Post "/element/$renameElement/value" @{text=$operation.stem}
                    $action=Button 'Save name'
                }
                delete {Click (Button 'Delete from phone');$action=Button 'Confirm deletion'}
            }
        }
        # The UI selection is now stable. Recheck exact owned identities and
        # bytes after its final refresh, with no later navigation before click.
        $finalListing=@(Videos $initial.session.generation)
        foreach($fixture in $selected){Assert-Source $fixture $finalListing}
        $seen=@($initial.jobs | ForEach-Object id)
        Journal @{event='mutation_planned';operation=$operation;identities=@($selected | ForEach-Object identity)}
        $start=[Diagnostics.Stopwatch]::GetTimestamp()
        Click $action
        $watch=[Diagnostics.Stopwatch]::StartNew();$job=$null;$uncertainty=0.0
        do {
            $observation=[Diagnostics.Stopwatch]::StartNew();$snapshot=Bootstrap
            $new=@($snapshot.jobs | Where-Object {$_.id -cnotin $seen})
            if($new.Count -gt 1){throw 'Concurrent operation invalidates the trial.'}
            if($new.Count -eq 1) {
                $job=$new[0]
                $expectedKind=if($operation.kind -eq 'collision'){'mark'}else{$operation.kind}
                if($job.kind -cne $expectedKind -or $job.generation -ne $initial.session.generation -or $job.count -ne $selected.Count){throw 'Wrong operation kind/generation/count.'}
                $visible=Evaluate 'return [...document.querySelectorAll(".file-result-notice")].some(node=>node.dataset.operationId===arguments[0])' @($job.id)
                if($visible -and $job.status -in @('completed','failed','cancelled')){break}
            }
            if($watch.Elapsed.TotalSeconds -gt 30){throw 'Mutation acknowledgement timed out. Preserve/reconcile this trial; never repeat the action.'}
            $uncertainty=[math]::Max($uncertainty,125+$observation.Elapsed.TotalMilliseconds)
            Start-Sleep -Milliseconds 125
        }while($true)
        $notice=@($snapshot.file_results.notices | Where-Object {$_.key.operation_id -ceq $job.id})
        if($notice.Count -ne 1){throw 'Matched shared result missing.'}
        if($operation.kind -eq 'collision') {
            if($notice[0].outcome -cnotin @('retained','failed') -or $job.results[0].status -cne 'retained' -or $job.results[0].message -cne 'destination_exists'){throw 'Collision did not retain both files.'}
        } elseif($job.status -cne 'completed' -or $notice[0].outcome -cnotin @('completed','warning')) {
            Journal @{event='uncertain_or_failed';job=$job;notice=$notice[0]}
            throw 'Mutation was not confirmed. Stop and reconcile exact owned paths before another mutation.'
        }
        $rowNames=@($selected | ForEach-Object name)
        if($operation.kind -eq 'mark') {
            Wait-Until {Evaluate 'return ![...document.querySelectorAll("tbody tr td:nth-child(2) bdi")].some(node=>arguments[0].includes(node.textContent))' @(,$rowNames)} 'A confirmed source row did not disappear.'
        }
        $end=[Diagnostics.Stopwatch]::GetTimestamp()
        $uncertainty=[math]::Max($uncertainty,125+$observation.Elapsed.TotalMilliseconds)
        $focus=Evaluate 'return {tag:document.activeElement?.tagName,id:document.activeElement?.id,row_key:document.activeElement?.closest("tr[data-video-key]")?.dataset.videoKey??null}'
        $result=@{operation=$operation.id;kind=$operation.kind;job=$job;notice=$notice[0];configuration=$trial.configuration;ui_ms=1000.0*($end-$start)/[Diagnostics.Stopwatch]::Frequency;observation_uncertainty_ms=$uncertainty;focus_after=$focus;start_ticks=[string]$start;end_ticks=[string]$end;clock_frequency=[string][Diagnostics.Stopwatch]::Frequency}
        $results.Add($result);Journal @{event='native_settled';result=$result}
        foreach($fixture in $selected) {
            $name=if($operation.kind -eq 'rename'){$operation.stem+'.mp4'}else{$fixture.name}
            $root=if($operation.kind -eq 'mark'){$trial.handoff_root}else{$trial.source_root}
            if($operation.kind -ne 'delete' -and (RemoteHash $root $name) -cne $fixture.sha256){throw 'Post-operation bytes differ from the preserved reference.'}
            if($operation.kind -in @('mark','delete','rename')) {
                $path=PhoneQuote ($trial.source_root+'/'+$fixture.name)
                $null=Invoke-TransferAdb -Trial $trial -Arguments @('shell','-T',"test ! -e $path && test ! -L $path")
            }
            if($operation.kind -eq 'mark') {
                $phoneLink=Join-Path $trial.phone_link_root ('storage/DCIM/TakeDock/'+$name)
                Wait-Until {Test-Path -LiteralPath $phoneLink -PathType Leaf} 'Handed-off file did not appear in the registered Phone Link root.' 60
                $readback=Join-Path $TrialRoot ('phone-link-'+$fixture.id+'.mp4')
                Copy-Item -LiteralPath $phoneLink -Destination $readback
                if((Get-Item -LiteralPath $readback).Length -ne [long]$fixture.bytes -or (Get-FileHash -LiteralPath $readback -Algorithm SHA256).Hash.ToLowerInvariant() -cne $fixture.sha256){throw 'Phone Link readback differs from the reference.'}
                $settled.Add(@{name=$name;sha256=$fixture.sha256})
            }
            if($operation.kind -eq 'collision' -and (RemoteHash $trial.handoff_root $name) -cne $fixture.collision_sha256){throw 'Existing collision destination changed.'}
        }
        Write-Host ("Verified {0}: native settlement {1:N1} ms; integrity checks excluded." -f $operation.id,$result.ui_ms)
    }
    Close-Native;Open-Native
    $after=@(Videos (Bootstrap).session.generation)
    foreach($original in $unrelated){if(@($after | Where-Object {$_.name -ceq $original.name -and $_.identity -ceq $original.identity}).Count -ne 1){throw 'Unrelated source metadata changed.'}}
    foreach($handoff in $settled){if((RemoteHash $trial.handoff_root $handoff.name) -cne $handoff.sha256){throw 'An already handed-off fixture changed after source operations/restart.'}}
    $results.ToArray() | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $TrialRoot 'results.json')
    Journal @{event='trial_verified';operations=$results.Count;unrelated_sources=$unrelated.Count}
} catch {$failure=$_;Journal @{event='trial_failed';reason=$_.Exception.Message}}
finally {
    try {Close-Native} catch {Journal @{event='native_cleanup_failed';reason=$_.Exception.Message};if(-not $failure){$failure=$_}}
    try {
        $null=Invoke-TransferAdb -Trial $trial -Arguments @('shell','input','keyevent','KEYCODE_HOME')
        $null=Invoke-TransferAdb -Trial $trial -Arguments @('shell','input','keyevent','KEYCODE_SLEEP')
        $power=Invoke-TransferAdb -Trial $trial -Arguments @('shell','dumpsys','power')
        if($power -notmatch 'mWakefulness=(Asleep|Dozing)'){throw 'Phone sleep was not confirmed.'}
        $activity=Invoke-TransferAdb -Trial $trial -Arguments @('shell','dumpsys','activity','activities')
        $resumed=@($activity -split "`n" | Where-Object {$_ -match 'mResumedActivity|topResumedActivity'})
        if($resumed -match 'net\.sourceforge\.opencamera'){throw 'Open Camera is still foreground after Home/sleep.'}
        $null=Invoke-TransferAdb -Trial $trial -Arguments @('get-state')
        $profileAfter=if(Test-Path -LiteralPath $realProfile){(Get-FileHash -LiteralPath $realProfile -Algorithm SHA256).Hash}else{''}
        if($profileAfter -cne $profileBefore){throw 'The shared user profile changed during this isolated trial; no automatic overwrite.'}
        Journal @{event='cleanup_verified';phone_asleep=$true;camera_foreground=$false;adb_pairing_preserved=$true;shared_profile_unchanged=$true}
    } catch {Journal @{event='cleanup_failed';reason=$_.Exception.Message};if(-not $failure){$failure=$_}}
}
if($failure){throw $failure}
Write-Output 'PASS: native source-only operations, exact owned Phone Link readback, restart preservation and phone sleep verified.'
