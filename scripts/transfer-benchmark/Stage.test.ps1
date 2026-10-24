. (Join-Path $PSScriptRoot 'TestSupport.ps1')
Import-Module (Join-Path $PSScriptRoot 'Core.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'Device.psm1') -Force -DisableNameChecking
Import-Module (Join-Path $PSScriptRoot 'Stage.psm1') -Force -DisableNameChecking

function New-StageTestReport {
    $runs=@(foreach($bytes in @('500000000','1000000000')) {
        foreach($kind in @('copy','move')) {
            foreach($build in @('released','baseline')) {
                foreach($warmup in @($true,$false)) {
                    @{bytes=$bytes;kind=$kind;build=$build;warmup=$warmup;valid=$true;outcome='completed';ui_ms=if($build -ceq 'released'){100}else{110};backend_ms=if($build -ceq 'baseline'){109}else{$null}}
                }
            }
        }
    })
    return @{stage='baseline';instrumentation_uncertainty_ms=10;runs=$runs}
}
Test-TransferCase 'early comparison failure invalidates registered settled rows before report assembly' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $attempt=New-TransferStageAttempt -Trial $trial -Stage Compare -Accessibility @{enabled='1';services='test'}
    $id='a'*32
    Register-TransferStageRun -Attempt $attempt -RunId $id
    $result=@{run_id=$id;stage='Compare';stage_attempt_id=$attempt.id;build='baseline';bytes='500000000';kind='copy';pair=0;order=0;warmup=$true;valid=$true;outcome='completed'}
    Write-TransferJournal -Trial $trial -Entry @{event='run_planned';run=$result;build_sha256=('f'*64)}
    Write-TransferJournal -Trial $trial -Entry @{event='run_verified';result=$result}
    Write-TransferJournal -Trial $trial -Entry @{event='run_cleanup_complete';run_id=$id}
    $pending=@(Get-Content $trial.journal | ForEach-Object {$_|ConvertFrom-Json -AsHashtable})
    Assert-TransferReject {Get-TransferCompletedRun -Events $pending -Build baseline -Sha256 ('f'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true -Stage Compare}
    Close-TransferStageAttempt -Trial $trial -Attempt $attempt -Valid $false -Reason 'Later run failed before report.'
    $events=@(Get-Content $trial.journal | ForEach-Object {$_|ConvertFrom-Json -AsHashtable})
    Assert-TransferReject {Get-TransferCompletedRun -Events $events -Build baseline -Sha256 ('f'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true -Stage Compare}
    $saved=Get-Content $attempt.latest_path -Raw|ConvertFrom-Json -AsHashtable
    Assert-TransferTest ($saved.stage_valid -eq $false -and $saved.stage_status -ceq 'invalid' -and $id -cin $saved.run_ids) 'Early failure lost its invalidated row.'
}
Test-TransferCase 'run verification followed by cleanup failure remains bound to the invalid attempt' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $attempt=New-TransferStageAttempt -Trial $trial -Stage Baseline -Accessibility @{}
    Register-TransferStageRun -Attempt $attempt -RunId ('b'*32)
    Write-TransferJournal -Trial $trial -Entry @{event='run_verified';result=@{run_id=('b'*32)}}
    Close-TransferStageAttempt -Trial $trial -Attempt $attempt -Valid $false -Reason 'Profile cleanup failed.'
    $failed=@(Get-Content $trial.journal|ForEach-Object {$_|ConvertFrom-Json -AsHashtable}|Where-Object event -CEQ stage_invalid)[-1]
    Assert-TransferTest ($failed.run_ids.Count -eq 1 -and $failed.run_ids[0] -ceq ('b'*32)) 'Cleanup-failed row omitted from invalidation.'
}
Test-TransferCase 'baseline remains pending until closure and failed preservation cannot feed Compare' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $attempt=New-TransferStageAttempt -Trial $trial -Stage Baseline -Accessibility @{}
    Set-TransferStageReport -Attempt $attempt -Report (New-StageTestReport)
    Assert-TransferReject {Read-TransferCalibration -TrialRoot $trial.root -Path $attempt.latest_path}
    Close-TransferStageAttempt -Trial $trial -Attempt $attempt -Valid $false -Reason 'Accessibility changed.'
    Assert-TransferReject {Read-TransferCalibration -TrialRoot $trial.root -Path $attempt.latest_path}
    $saved=Get-Content $attempt.latest_path -Raw|ConvertFrom-Json -AsHashtable
    Assert-TransferTest ($saved.stage_status -ceq 'invalid' -and $saved.runs.Count -eq 16) 'Raw calibration or invalid status was lost.'
}
Test-TransferCase 'valid calibration requires all cells and the declared uncertainty' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $attempt=New-TransferStageAttempt -Trial $trial -Stage Baseline -Accessibility @{}
    Set-TransferStageReport -Attempt $attempt -Report (New-StageTestReport)
    Close-TransferStageAttempt -Trial $trial -Attempt $attempt -Valid $true
    $valid=Read-TransferCalibration -TrialRoot $trial.root -Path $attempt.latest_path
    Assert-TransferTest ($valid.runs.Count -eq 16 -and $valid.instrumentation_uncertainty_ms -eq 10) 'Complete calibration refused.'
    foreach($change in @('missing','duplicate','uncertainty','legacy')) {
        $report=Get-Content $attempt.latest_path -Raw|ConvertFrom-Json -AsHashtable
        switch($change) {
            missing {$report.runs=$report.runs[0..14]}
            duplicate {$report.runs[15]=@{}+$report.runs[14]}
            uncertainty {$report.instrumentation_uncertainty_ms=0}
            legacy {$report.Remove('stage_valid')}
        }
        $report|ConvertTo-Json -Depth 20|Set-Content (Join-Path $trial.root 'altered.json')
        Assert-TransferReject {Read-TransferCalibration -TrialRoot $trial.root -Path (Join-Path $trial.root 'altered.json')}
    }
}
Test-TransferCase 'latest stage status preserves the prior report and cannot validate an incomplete attempt' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    [IO.File]::WriteAllText((Join-Path $trial.root 'comparison.json'),'prior evidence')
    $attempt=New-TransferStageAttempt -Trial $trial -Stage Compare -Accessibility @{}
    Assert-TransferTest ([IO.File]::ReadAllText($attempt.previous_path) -ceq 'prior evidence') 'Prior evidence overwritten.'
    Assert-TransferReject {Close-TransferStageAttempt -Trial $trial -Attempt $attempt -Valid $true}
}
Test-TransferCase 'a valid-looking calibration without a completed journal closure is refused' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $attempt=New-TransferStageAttempt -Trial $trial -Stage Baseline -Accessibility @{}
    Set-TransferStageReport -Attempt $attempt -Report (New-StageTestReport)
    $report=Get-Content $attempt.latest_path -Raw|ConvertFrom-Json -AsHashtable
    $report.stage_valid=$true;$report.stage_status='valid'
    $report|ConvertTo-Json -Depth 20|Set-Content $attempt.latest_path
    Assert-TransferReject {Read-TransferCalibration -TrialRoot $trial.root -Path $attempt.latest_path}
}
Test-TransferCase 'a registered settled receipt can resume only after its successful stage closure' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $attempt=New-TransferStageAttempt -Trial $trial -Stage Compare -Accessibility @{}
    $id='c'*32
    Register-TransferStageRun -Attempt $attempt -RunId $id
    $run=@{run_id=$id;stage='Compare';stage_attempt_id=$attempt.id;build='candidate';bytes='500000000';kind='copy';pair=1;order=1;warmup=$false;valid=$true;outcome='completed'}
    Write-TransferJournal -Trial $trial -Entry @{event='run_planned';run=$run;build_sha256=('e'*64)}
    Write-TransferJournal -Trial $trial -Entry @{event='run_verified';result=$run}
    Write-TransferJournal -Trial $trial -Entry @{event='run_cleanup_complete';run_id=$id}
    Set-TransferStageReport -Attempt $attempt -Report @{stage='compare';decision='rejected';verdict=@{accepted=$false;reasons=@()};runs=@($run)}
    Close-TransferStageAttempt -Trial $trial -Attempt $attempt -Valid $true
    $events=@(Get-Content $trial.journal|ForEach-Object {$_|ConvertFrom-Json -AsHashtable})
    $saved=Get-TransferCompletedRun -Events $events -Build candidate -Sha256 ('e'*64) -Bytes 500000000 -Kind copy -Pair 1 -Order 1 -Warmup $false -Stage Compare
    Assert-TransferTest ($saved.run_id -ceq $id) 'A successfully closed stage could not resume its exact receipt.'
}
Write-Output "PASS: $script:Cases stage-attempt cases."
