Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
Import-Module (Join-Path $PSScriptRoot 'Core.psm1')
Import-Module (Join-Path $PSScriptRoot 'Device.psm1') -DisableNameChecking

function Save-TransferStageAttempt([hashtable]$Attempt) {
    $Attempt.report.stage_attempt_id=$Attempt.id
    $Attempt.report.run_ids=@($Attempt.run_ids)
    $json=$Attempt.report | ConvertTo-Json -Depth 40
    [IO.File]::WriteAllText($Attempt.attempt_path,$json)
    [IO.File]::WriteAllText($Attempt.latest_path,$json)
}
function New-TransferStageAttempt {
    param([hashtable]$Trial,[ValidateSet('Baseline','Compare')][string]$Stage,[hashtable]$Accessibility)
    $id=[guid]::NewGuid().ToString('N')
    $directory=Assert-TransferLocalPath -Root $Trial.root -Path (Join-Path $Trial.root 'stages')
    $null=New-Item -ItemType Directory -Path $directory -Force
    $filename=if($Stage -ceq 'Baseline'){'baseline.json'}else{'comparison.json'}
    $latest=Assert-TransferLocalPath -Root $Trial.root -Path (Join-Path $Trial.root $filename)
    $attempt=@{id=$id;stage=$Stage;run_ids=[Collections.Generic.List[string]]::new();report_ready=$false;latest_path=$latest;attempt_path=(Join-Path $directory "$id.json");previous_path=(Join-Path $directory "previous-$id.json");report=@{stage=$Stage.ToLowerInvariant();stage_valid=$false;stage_status='pending';stage_invalid_reason='Preservation and cleanup pending.';decision='pending';runs=@();accessibility_baseline=$Accessibility}}
    if(Test-Path -LiteralPath $latest){Copy-Item -LiteralPath $latest -Destination $attempt.previous_path}
    Save-TransferStageAttempt $attempt
    Write-TransferJournal -Trial $Trial -Entry @{event='stage_started';stage=$Stage;stage_attempt_id=$id;accessibility=$Accessibility}
    return $attempt
}
function Register-TransferStageRun {
    param([hashtable]$Attempt,[string]$RunId)
    if($RunId -cnotmatch '^[a-f0-9]{32}$'){throw 'Exact run identity required.'}
    if($RunId -cnotin $Attempt.run_ids){$Attempt.run_ids.Add($RunId)}
    Save-TransferStageAttempt $Attempt
}
function Set-TransferStageReport {
    param([hashtable]$Attempt,[hashtable]$Report)
    $Report.stage_valid=$false
    $Report.stage_status='pending'
    $Report.stage_invalid_reason='Preservation and cleanup pending.'
    $Attempt.report=$Report
    $Attempt.report_ready=$true
    Save-TransferStageAttempt $Attempt
}
function Close-TransferStageAttempt {
    param([hashtable]$Trial,[hashtable]$Attempt,[bool]$Valid,[string]$Reason)
    if($Valid -and -not $Attempt.report_ready){throw 'An incomplete stage cannot be validated.'}
    Set-TransferStageStatus -Report $Attempt.report -Valid $Valid -Reason $Reason
    Save-TransferStageAttempt $Attempt
    $event=if($Valid){'stage_validated'}else{'stage_invalid'}
    Write-TransferJournal -Trial $Trial -Entry @{event=$event;stage=$Attempt.stage;stage_attempt_id=$Attempt.id;reason=$Reason;run_ids=@($Attempt.run_ids)}
}
function Read-TransferCalibration {
    param([string]$TrialRoot,[string]$Path)
    $path=Assert-TransferLocalPath -Root $TrialRoot -Path $Path
    $report=Get-Content -LiteralPath $path -Raw|ConvertFrom-Json -AsHashtable
    if($report.stage -cne 'baseline' -or -not $report.ContainsKey('stage_valid') -or $report.stage_valid -isnot [bool] -or -not $report.stage_valid -or $report.stage_status -cne 'valid'){throw 'A completed, preserved calibration stage is required; legacy or failed stages need explicit verification.'}
    $journal=Assert-TransferLocalPath -Root $TrialRoot -Path (Join-Path $TrialRoot 'journal.jsonl')
    $closed=@(Get-Content -LiteralPath $journal|ForEach-Object {$_|ConvertFrom-Json -AsHashtable}|Where-Object {$_.event -ceq 'stage_validated' -and $_.stage_attempt_id -ceq $report.stage_attempt_id})
    if($closed.Count -ne 1){throw 'Calibration stage has no unique successful journal closure.'}
    if($report.runs.Count -ne 16 -or -not [double]::IsFinite([double]$report.instrumentation_uncertainty_ms) -or [double]$report.instrumentation_uncertainty_ms -lt 0){throw 'Incomplete or invalid calibration.'}
    $uncertainty=0.0
    foreach($bytes in @('500000000','1000000000')) {
        foreach($kind in @('copy','move')) {
            foreach($warmup in @($true,$false)) {
                $times=@{}
                foreach($build in @('released','baseline')) {
                    $cell=@($report.runs | Where-Object {$_.bytes -ceq $bytes -and $_.kind -ceq $kind -and $_.build -ceq $build -and $_.warmup -eq $warmup})
                    if($cell.Count -ne 1 -or $cell[0].valid -ne $true -or $cell[0].outcome -cne 'completed' -or -not [double]::IsFinite([double]$cell[0].ui_ms) -or [double]$cell[0].ui_ms -lt 0){throw 'Missing, duplicate or invalid calibration cell.'}
                    if($build -ceq 'baseline' -and ($null -eq $cell[0].backend_ms -or -not [double]::IsFinite([double]$cell[0].backend_ms) -or [double]$cell[0].backend_ms -lt 0)){throw 'Diagnostic calibration total missing.'}
                    $times[$build]=[double]$cell[0].ui_ms
                }
                if(-not $warmup){$uncertainty=[math]::Max($uncertainty,[math]::Abs($times.released-$times.baseline))}
            }
        }
    }
    if([math]::Abs($uncertainty-[double]$report.instrumentation_uncertainty_ms) -gt 0.001){throw 'Calibration uncertainty differs from retained observations.'}
    return $report
}
Export-ModuleMember -Function New-TransferStageAttempt,Register-TransferStageRun,Set-TransferStageReport,Close-TransferStageAttempt,Read-TransferCalibration
