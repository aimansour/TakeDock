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
    $state=@{snapshots=0;destination=$destination}
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
                $jobs=if($state.snapshots -ge 3){@(@{id='job';kind='copy';count=1;generation=7;status='running'})}else{@()}
                $notices=if($state.snapshots -ge 4){@(@{key=@{operation_id='job'};outcome='completed';count=1;completed=1})}else{@()}
                return @{session=@{connected=$true;generation=7};settings=@{destination=$state.destination};jobs=@($jobs);file_results=@{notices=@($notices)}}
            }
            if($path -eq '/element'){return @{$key='element'}}
            if($path -eq '/elements'){return @(@{$key='element'})}
            if($path.EndsWith('/attribute/data-operation-id')){return 'job'}
            if($path -eq '/actions' -or $path.EndsWith('/click')){return $null}
            throw 'Unexpected driver command.'
        }.GetNewClosure()
    } $state
    $run=@{fixture_id='master-500';kind='copy';bytes='4';destination=$destination}
    $result=Invoke-TransferRun -Session @{uri='http://127.0.0.1:1';id='fake'} -Trial $trial -Run $run
    Assert-TransferTest ($result.outcome -eq 'completed' -and $state.snapshots -ge 4) 'Native completion used a stale snapshot.'
}
Write-Output "PASS: $script:Cases UI-measurement cases."
