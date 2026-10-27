. (Join-Path $PSScriptRoot 'TestSupport.ps1')
Import-Module (Join-Path $PSScriptRoot 'Ui.psm1') -Force
Test-TransferCase 'combined imports retain every shared runner command' {
    Import-Module (Join-Path $PSScriptRoot 'Core.psm1') -Force
    Import-Module (Join-Path $PSScriptRoot 'Device.psm1') -Force -DisableNameChecking
    Import-Module (Join-Path $PSScriptRoot 'Ui.psm1') -Force -DisableNameChecking
    foreach($name in @('Assert-TransferLocalPath','Read-TransferTrial','Get-TransferVerdict','Assert-TransferSource','Invoke-TransferRun')) {
        $null=Get-Command $name -ErrorAction Stop
    }
}
Test-TransferCase '100 percent alone and the wrong job are not completion' {
    $snapshot=@{notices=@(@{key=@{operation_id='other'};outcome='completed'})}
    Assert-TransferReject {Assert-TransferTerminal -JobId job -NoticeId other -Snapshot $snapshot}
    $snapshot.notices=@()
    Assert-TransferReject {Assert-TransferTerminal -JobId job -NoticeId job -Snapshot $snapshot}
}
Test-TransferCase 'a matched persistent warning is not a clean performance success' {
    $snapshot=@{notices=@(@{key=@{operation_id='job'};outcome='warning'})}
    Assert-TransferReject {Assert-TransferTerminal -JobId job -NoticeId job -Snapshot $snapshot}
    $snapshot.notices[0].outcome='completed'
    $null=Assert-TransferTerminal -JobId job -NoticeId job -Snapshot $snapshot
}
Test-TransferCase 'profile restore preserves unexpected concurrent changes' {
    $root=New-TransferTestRoot
    $path=Join-Path $root 'settings.json'
    $before=[Text.Encoding]::UTF8.GetBytes('{"language":"ar"}')
    $last=[Text.Encoding]::UTF8.GetBytes('{"language":"en"}')
    [IO.File]::WriteAllText($path,'{"language":"ar","destination":"new external change"}')
    $current=[IO.File]::ReadAllBytes($path)
    Assert-TransferReject {Restore-TransferProfile -Path $path -Before $before -LastWritten $last}
    Assert-TransferTest ([Convert]::ToBase64String([IO.File]::ReadAllBytes($path)) -eq [Convert]::ToBase64String($current)) 'Concurrent settings overwritten.'
    [IO.File]::WriteAllBytes($path,$last)
    Restore-TransferProfile -Path $path -Before $before -LastWritten $last
    Assert-TransferTest ([Convert]::ToBase64String([IO.File]::ReadAllBytes($path)) -eq [Convert]::ToBase64String($before)) 'Original profile not restored.'
}
Test-TransferCase 'profile restore tolerates equivalent application serialization only' {
    $root=New-TransferTestRoot
    $path=Join-Path $root 'settings.json'
    $before=[Text.Encoding]::UTF8.GetBytes('{"language":"ar","ready_sound":true}')
    $last=[Text.Encoding]::UTF8.GetBytes('{"language":"en","ready_sound":true}')
    [IO.File]::WriteAllText($path,"{`n  `"ready_sound`": true,`n  `"language`": `"en`"`n}")
    Restore-TransferProfile -Path $path -Before $before -LastWritten $last
    Assert-TransferTest ([Convert]::ToBase64String([IO.File]::ReadAllBytes($path)) -ceq [Convert]::ToBase64String($before)) 'Equivalent serialization was not restored.'
    [IO.File]::WriteAllText($path,'{"language":"en","ready_sound":false}')
    Assert-TransferReject {Restore-TransferProfile -Path $path -Before $before -LastWritten $last}
}
Test-TransferCase 'native notice appearing after a snapshot is confirmed from a fresh authoritative read' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $destination=Join-Path $trial.root 'output';New-Item -ItemType Directory -Path $destination | Out-Null
    $state=@{snapshots=0;destination=$destination;started=$false;after_start_reads=0}
    & (Get-Module Device) {param($trial)
        $script:AdbAdapter={param($value,$arguments)
            if(($arguments -join ' ') -match "'stat'"){return $trial.fixtures[0].source_identity}
            if(($arguments -join ' ') -match "'hash'"){return $trial.fixtures[0].sha256}
            throw 'Unexpected device command.'
        }.GetNewClosure()
    } $trial
    & (Get-Module Ui) {param($state)
        $script:DriverAdapter={param($session,$method,$path,$body)
            $key='element-6066-11e4-a52e-4f735466cecf'
            if($path -eq '/execute/sync') {
                $state.snapshots++
                if($state.started){$state.after_start_reads++}
                $jobs=if($state.started){@(@{id='job';kind='copy';count=1;generation=7;status='running'})}else{@()}
                $notices=if($state.after_start_reads -ge 2){@(@{key=@{operation_id='job'};outcome='completed';count=1;completed=1})}else{@()}
                return @{session=@{connected=$true;generation=7};settings=@{destination=$state.destination};jobs=@($jobs);file_results=@{notices=@($notices)}}
            }
            if($path -eq '/element'){return @{$key=if($body.value.Contains('Copy to computer')){'action'}else{'element'}}}
            if($path -eq '/elements'){if($body.using -ceq 'xpath'){return @()};return @(@{$key='element'})}
            if($path.EndsWith('/attribute/data-operation-id')){return 'job'}
            if($path -eq '/actions' -or $path.EndsWith('/click')){if($path -ceq '/element/action/click'){$state.started=$true};return $null}
            throw 'Unexpected driver command.'
        }.GetNewClosure()
    } $state
    $run=@{fixture_id='master-500';kind='copy';bytes='4';destination=$destination}
    $result=Invoke-TransferRun -Session @{uri='http://127.0.0.1:1';id='fake'} -Trial $trial -Run $run
    Assert-TransferTest ($result.outcome -eq 'completed' -and $state.snapshots -ge 4) 'Native completion used a stale snapshot.'
    $evidence=Join-Path $trial.root 'terminal-observation.json'
    Assert-TransferTest (Test-Path -LiteralPath $evidence) 'Authoritative native terminal evidence was not preserved before validation.'
    $saved=Get-Content -LiteralPath $evidence -Raw | ConvertFrom-Json -AsHashtable
    Assert-TransferTest ($saved.file_results.notices[0].key.operation_id -ceq 'job') 'Wrong terminal evidence saved.'
}
function New-TransferSelectionCase([string]$Fault) {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $trial.fixtures[0].role='move_duplicate'
    $destination=Join-Path $trial.root 'output';New-Item -ItemType Directory -Path $destination | Out-Null
    $state=@{fault=$Fault;destination=$destination;replaced=$false;stats=0;final_guard=$false;move_clicks=0;loading_reads=0;refresh_settled=$false;early_selection=$false;selected='owned';snapshots=0}
    & (Get-Module Device) {param($trial,$state)
        $script:AdbAdapter={param($value,$arguments)
            $command=$arguments -join ' '
            if($command -match "'stat'") {
                $state.stats++
                if($state.replaced){return '{"device":1,"inode":99,"size":4,"modified_ns":2,"changed_ns":2}'}
                return $trial.fixtures[0].source_identity
            }
            if($command -match "'hash'") {if($state.stats -ge 2){$state.final_guard=$true};return $trial.fixtures[0].sha256}
            throw 'Unexpected device operation.'
        }.GetNewClosure()
    } $trial $state
    & (Get-Module Ui) {param($state)
        $script:DriverAdapter={param($session,$method,$path,$body)
            $key='element-6066-11e4-a52e-4f735466cecf'
            if($path -eq '/actions') {if($state.fault -ceq 'navigation'){$state.replaced=$true};return $null}
            if($path -eq '/execute/sync') {
                $state.snapshots++
                $generation=if($state.final_guard -and $state.fault -ceq 'generation'){8}else{7}
                $destination=if($state.final_guard -and $state.fault -ceq 'destination'){Join-Path $state.destination 'changed'}else{$state.destination}
                $jobs=if($state.move_clicks){@(@{id='job';kind='move';count=1;generation=7;status='running'})}elseif($state.final_guard -and $state.fault -ceq 'jobs'){@(@{id='unrelated';kind='copy';count=1;generation=7;status='queued'})}else{@()}
                return @{session=@{connected=$true;generation=$generation};settings=@{destination=$destination};jobs=$jobs;file_results=@{notices=@(@{key=@{operation_id='job'};outcome='completed';count=1;completed=1})}}
            }
            if($path -eq '/element') {
                $id=if($body.value.Contains('Move to computer')){'move'}elseif($body.value.Contains('Select ')){'owned'}elseif($body.value.Contains('Refresh')){'refresh'}else{'notice'}
                return @{$key=$id}
            }
            if($path -eq '/elements') {
                if($body.using -ceq 'xpath') {
                    $state.loading_reads++
                    if($state.fault -ceq 'delayed' -and $state.loading_reads -le 2){return @(@{$key='loading'})}
                    $state.refresh_settled=$true;return @()
                }
                $selected=if($state.final_guard -and $state.fault -ceq 'checked'){'replacement'}else{$state.selected}
                return @(@{$key=$selected})
            }
            if($path.EndsWith('/click')) {
                if($path -ceq '/element/owned/click'){$state.early_selection=-not $state.refresh_settled;if($state.fault -ceq 'selection'){$state.replaced=$true}}
                if($path -ceq '/element/move/click'){$state.move_clicks++}
                return $null
            }
            if($path.EndsWith('/attribute/data-operation-id')){return 'job'}
            throw 'Unexpected driver operation.'
        }.GetNewClosure()
    } $state
    return @{trial=$trial;state=$state;run=@{fixture_id='master-500';kind='move';bytes='4';destination=$destination}}
}
Test-TransferCase 'settled native selection is guarded before Move without product hooks' {
    $failures=[Collections.Generic.List[string]]::new()
    foreach($fault in @('navigation','selection','generation','destination','jobs','checked','delayed','valid')) {
        $case=New-TransferSelectionCase $fault
        $rejected=$false;$result=$null
        try {$result=Invoke-TransferRun -Session @{uri='http://127.0.0.1:1';id='test'} -Trial $case.trial -Run $case.run} catch {$rejected=$true}
        try {
            if($fault -in @('delayed','valid')) {
                Assert-TransferTest (-not $rejected -and $result.outcome -ceq 'completed' -and $case.state.move_clicks -eq 1) 'Valid native action failed or repeated.'
                Assert-TransferTest (-not $case.state.early_selection -and $case.state.stats -ge 2) 'Selection/action preceded settled refresh or final source guard.'
            } else {
                Assert-TransferTest ($rejected -and $case.state.move_clicks -eq 0) 'Unsafe selection reached the Move button.'
            }
        } catch {$failures.Add("${fault}: $($_.Exception.Message)")}
    }
    Assert-TransferTest ($failures.Count -eq 0) ($failures -join '; ')
}
Write-Output "PASS: $script:Cases UI-measurement cases."
