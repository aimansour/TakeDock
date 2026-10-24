Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-TransferLocalPath {
    param([Parameter(Mandatory)][string]$Root,[Parameter(Mandatory)][string]$Path,[switch]$AllowRoot)
    if(-not [IO.Path]::IsPathFullyQualified($Root) -or -not [IO.Path]::IsPathFullyQualified($Path)){throw 'Absolute trial paths required.'}
    $rootFull=[IO.Path]::GetFullPath($Root).TrimEnd([IO.Path]::DirectorySeparatorChar)
    $full=[IO.Path]::GetFullPath($Path)
    $comparison=if($IsWindows){[StringComparison]::OrdinalIgnoreCase}else{[StringComparison]::Ordinal}
    if(-not $full.StartsWith($rootFull+[IO.Path]::DirectorySeparatorChar,$comparison) -and -not ($AllowRoot -and $full.Equals($rootFull,$comparison))){throw 'Path escapes its trial root.'}
    $ancestor=$full
    while($ancestor) {
        if(Test-Path -LiteralPath $ancestor) {
            $item=Get-Item -LiteralPath $ancestor -Force
            if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Reparse/symlink path is not a trial target.'}
        }
        $ancestor=[IO.Path]::GetDirectoryName($ancestor)
    }
    return $full
}
function Assert-TransferDriverPin {
    param([Parameter(Mandatory)][string]$Executable)
    $root=Split-Path (Split-Path $Executable -Parent) -Parent
    $receipt=Get-Content -LiteralPath (Join-Path $root '.crates2.json') -Raw | ConvertFrom-Json -AsHashtable
    $matching=@($receipt.installs.Keys | Where-Object {$_ -match '^tauri-driver 2\.1\.0 \(registry\+https://github\.com/rust-lang/crates\.io-index\)$'})
    if($matching.Count -ne 1 -or [IO.Path]::GetFileName($Executable) -notin $receipt.installs[$matching[0]].bins){throw 'Pinned tauri-driver 2.1.0 Cargo receipt required.'}
}
function Get-TransferCompletedRun {
    param([object[]]$Events,[string]$Build,[string]$Sha256,[long]$Bytes,[string]$Kind,[int]$Pair,[int]$Order,[bool]$Warmup,[string]$Stage)
    $matches=@($Events | Where-Object {$_.event -eq 'run_verified'} | Where-Object {
        $_.result.build -eq $Build -and [long]$_.result.bytes -eq $Bytes -and $_.result.kind -eq $Kind -and $_.result.pair -eq $Pair -and $_.result.order -eq $Order -and $_.result.warmup -eq $Warmup -and (-not $Stage -or ($_.result.ContainsKey('stage') -and $_.result.stage -ceq $Stage))
    })
    if($matches.Count -eq 0){return $null}
    if($matches.Count -ne 1){throw 'Repeated completed run in journal.'}
    $result=$matches[0].result
    if(@($Events | Where-Object {$_.event -ceq 'stage_invalid' -and $_.ContainsKey('run_ids') -and $result.run_id -cin $_.run_ids}).Count){throw 'Completed operation belongs to an invalid stage; preserve evidence and explicitly prepare a fresh trial, never retry its mutation.'}
    $planned=@($Events | Where-Object {$_.event -eq 'run_planned' -and $_.run.run_id -ceq $result.run_id})
    $cleanup=@($Events | Where-Object {$_.event -eq 'run_cleanup_complete' -and $_.run_id -ceq $result.run_id})
    if($result.valid -ne $true -or $result.outcome -ne 'completed' -or $planned.Count -ne 1 -or $planned[0].build_sha256 -cne $Sha256 -or $cleanup.Count -ne 1){throw 'Existing completed operation needs explicit recovery; never retry its mutation.'}
    if($planned[0].run.ContainsKey('stage_attempt_id')) {
        $attempt=$planned[0].run.stage_attempt_id
        $closed=@($Events | Where-Object {$_.event -ceq 'stage_validated' -and $_.stage_attempt_id -ceq $attempt -and $result.run_id -cin $_.run_ids})
        if($closed.Count -ne 1){throw 'Stage closure is pending or unverified; no automatic operation retry.'}
    }
    return $result
}

function Read-TransferTrial {
    param([Parameter(Mandatory)][string]$Manifest,[Parameter(Mandatory)][string]$TrialRoot)
    $null=Assert-TransferLocalPath -Root $TrialRoot -Path $TrialRoot -AllowRoot
    $null=Assert-TransferLocalPath -Root $TrialRoot -Path $Manifest
    $trial=Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json -AsHashtable
    if($trial.schema -ne 1 -or $trial.trial_id -notmatch '^[a-f0-9]{32}$'){throw 'Unknown trial schema/owner.'}
    if([IO.Path]::GetFullPath($trial.root) -ne [IO.Path]::GetFullPath($TrialRoot)){throw 'Manifest belongs to another root.'}
    foreach($path in @($trial.destination,$trial.journal)){$null=Assert-TransferLocalPath -Root $TrialRoot -Path $path}
    $ids=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($fixture in $trial.fixtures) {
        if(-not $ids.Add($fixture.id)){throw 'Duplicate fixture ownership.'}
        if($fixture.remote_name -notmatch ('^TDPerf-'+[regex]::Escape($trial.trial_id)+'-[a-zA-Z0-9-]+\.mp4$')){throw 'Unowned remote name.'}
        $null=Assert-TransferLocalPath -Root $TrialRoot -Path $fixture.backup
        if([string]::IsNullOrWhiteSpace($fixture.source_identity) -or $fixture.sha256 -notmatch '^[a-f0-9]{64}$' -or [long]$fixture.bytes -le 0){throw 'Invalid fixture identity/size/digest.'}
    }
    return $trial
}

function Get-TransferStatistics([double[]]$Values) {
    $ordered=@($Values | Sort-Object)
    $middle=[int][math]::Floor($ordered.Count/2)
    $median=if($ordered.Count % 2){$ordered[$middle]}else{($ordered[$middle-1]+$ordered[$middle])/2}
    return @{median=$median;range=$ordered[-1]-$ordered[0];samples=$Values}
}

function Get-TransferVerdict {
    param([Parameter(Mandatory)][object[]]$Runs,[Parameter(Mandatory)][double]$UncertaintyMs)
    $reasons=[Collections.Generic.List[string]]::new()
    $groups=[Collections.Generic.List[object]]::new()
    if(-not [double]::IsFinite($UncertaintyMs) -or $UncertaintyMs -lt 0){throw 'Invalid uncertainty.'}
    $measured=@($Runs | Where-Object {-not $_.warmup})
    foreach($run in $measured) {
        if($run.build -notin @('baseline','candidate') -or [string]$run.bytes -notin @('500000000','1000000000') -or $run.kind -notin @('copy','move')){$reasons.Add('Unexpected measured group/build.')}
        if($run.valid -ne $true -or $run.outcome -ne 'completed'){$reasons.Add('Incomplete/failed run cannot be performance evidence.')}
        foreach($metric in @('backend_ms','ui_ms','observation_uncertainty_ms')) {
            if($null -eq $run[$metric] -or -not [double]::IsFinite([double]$run[$metric]) -or [double]$run[$metric] -lt 0){$reasons.Add('Missing/nonfinite measurement.')}
        }
    }
    foreach($bytes in @('500000000','1000000000')) {
        foreach($kind in @('copy','move')) {
            $group=@($measured | Where-Object {[string]$_.bytes -eq $bytes -and $_.kind -eq $kind})
            $baseline=@($group | Where-Object build -eq baseline | Sort-Object pair)
            $candidate=@($group | Where-Object build -eq candidate | Sort-Object pair)
            $groupReasons=[Collections.Generic.List[string]]::new()
            if($baseline.Count -notin @(3,4,5) -or $candidate.Count -ne $baseline.Count){$groupReasons.Add('Require 3 matched pairs, at most 2 extra.')}
            $seen=[Collections.Generic.HashSet[string]]::new()
            foreach($run in $group) {
                if(-not $seen.Add("$($run.build):$($run.pair)")){$groupReasons.Add('Duplicate pair.')}
                $expectedOrder=if(([int]$run.pair % 2 -eq 1) -eq ($run.build -eq 'baseline')){0}else{1}
                if($run.order -ne $expectedOrder){$groupReasons.Add('Comparison order did not alternate.')}
            }
            $metrics=@{}
            if($groupReasons.Count -eq 0) {
                for($i=0;$i -lt $baseline.Count;$i++) {
                    if($baseline[$i].pair -ne $i+1 -or $candidate[$i].pair -ne $i+1){$groupReasons.Add('Missing pair index.')}
                }
                foreach($metric in @('backend_ms','ui_ms')) {
                    $left=Get-TransferStatistics @($baseline | ForEach-Object {[double]$_[$metric]})
                    $right=Get-TransferStatistics @($candidate | ForEach-Object {[double]$_[$metric]})
                    $uncertainty=$UncertaintyMs
                    foreach($run in $group){$uncertainty=[math]::Max($uncertainty,[double]$run.observation_uncertainty_ms)}
                    $gain=$left.median-$right.median
                    $differences=@(for($i=0;$i -lt $baseline.Count;$i++){[double]$baseline[$i][$metric]-[double]$candidate[$i][$metric]})
                    if(@($differences | Where-Object {$_ -le 0 -or -not [double]::IsFinite($_)}).Count){$groupReasons.Add("$metric did not improve in every pair.")}
                    if(-not ($gain -gt [math]::Max($left.range,$right.range) -and $gain -gt $uncertainty)){$groupReasons.Add("$metric gain does not exceed variation/uncertainty.")}
                    $metrics[$metric]=@{baseline=$left;candidate=$right;median_gain_ms=$gain;paired_differences=$differences;uncertainty_ms=$uncertainty}
                }
            }
            foreach($reason in $groupReasons){$reasons.Add("${bytes}/${kind}: $reason")}
            $groups.Add(@{bytes=$bytes;kind=$kind;accepted=($groupReasons.Count -eq 0);metrics=$metrics;reasons=$groupReasons.ToArray()})
        }
    }
    return @{accepted=($reasons.Count -eq 0);groups=$groups.ToArray();reasons=$reasons.ToArray()}
}

function Get-TransferFutility {
    param([Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Runs)
    $measured=@($Runs | Where-Object {-not $_.warmup})
    foreach($run in $measured) {
        if($run.build -cnotin @('baseline','candidate') -or [string]$run.bytes -cnotin @('500000000','1000000000') -or $run.kind -cnotin @('copy','move') -or [int]$run.pair -notin @(1,2,3,4,5) -or $run.valid -ne $true -or $run.outcome -cne 'completed'){throw 'Invalid measured comparison evidence.'}
        foreach($metric in @('backend_ms','ui_ms','observation_uncertainty_ms')) {
            if($null -eq $run[$metric] -or -not [double]::IsFinite([double]$run[$metric]) -or [double]$run[$metric] -lt 0){throw 'Invalid measured comparison total.'}
        }
        $expected=if(([int]$run.pair % 2 -eq 1) -eq ($run.build -ceq 'baseline')){0}else{1}
        if($run.order -ne $expected){throw 'Comparison pair order did not alternate.'}
    }
    $failures=[Collections.Generic.List[object]]::new();$pairs=0
    foreach($group in @($measured | Group-Object {"$($_.bytes)/$($_.kind)/$($_.pair)"})) {
        $baseline=@($group.Group | Where-Object build -CEQ baseline)
        $candidate=@($group.Group | Where-Object build -CEQ candidate)
        if($baseline.Count -ne 1 -or $candidate.Count -ne 1){throw 'Incomplete, mismatched or duplicate pair cannot decide futility.'}
        $pairs++
        foreach($metric in @('backend_ms','ui_ms')) {
            if([double]$candidate[0][$metric] -ge [double]$baseline[0][$metric]){$failures.Add(@{bytes=$baseline[0].bytes;kind=$baseline[0].kind;pair=$baseline[0].pair;metric=$metric;baseline_ms=$baseline[0][$metric];candidate_ms=$candidate[0][$metric]})}
        }
    }
    return @{stop=($failures.Count -gt 0);matched_pairs=$pairs;failures=$failures.ToArray()}
}

function Set-TransferStageStatus {
    param([hashtable]$Report,[bool]$Valid,[string]$Reason)
    if($Valid -and $Report.ContainsKey('stage_status') -and $Report.stage_status -ceq 'invalid'){throw 'An invalid stage cannot be relabelled valid.'}
    if(-not $Valid -and [string]::IsNullOrWhiteSpace($Reason)){throw 'Invalid stage requires a preserved reason.'}
    $Report.stage_valid=$Valid
    $Report.stage_status=if($Valid){'valid'}else{'invalid'}
    $Report.stage_invalid_reason=if($Valid){''}else{$Reason}
    if(-not $Valid) {
        $Report.decision='invalid_stage'
        if($Report.ContainsKey('verdict')) {
            $Report.verdict.accepted=$false
            $Report.verdict.reasons=@($Report.verdict.reasons)+@('Stage invalid: '+$Reason)
        }
    }
}

function Read-TransferTrace {
    param([Parameter(Mandatory)][string]$Path,[Parameter(Mandatory)][string]$JobId)
    $rows=@(Get-Content -LiteralPath $Path | Where-Object {$_} | ForEach-Object {ConvertFrom-Json -InputObject $_ -AsHashtable})
    if($rows.Count -eq 0 -or $rows[0].record -ne 'header' -or $rows[0].schema -ne 1 -or $rows[0].frequency -notmatch '^[1-9][0-9]*$'){throw 'Invalid trace header.'}
    if(@($rows | Where-Object record -eq header).Count -ne 1){throw 'Ambiguous trace header.'}
    $selected=@($rows | Where-Object {$_.record -ne 'header' -and $_.job_id -ceq $JobId})
    $footers=@($selected | Where-Object record -eq job_complete)
    if($footers.Count -ne 1 -or $footers[0].dropped -ne 0 -or $footers[0].writer_status -ne 'ok'){throw 'Trace is not complete and loss-free.'}
    $generation=[string]$footers[0].generation
    $edges=@($selected | Where-Object record -eq edge)
    foreach($row in $edges) {
        if([string]$row.generation -cne $generation -or $row.ticks -notmatch '^[0-9]+$' -or [decimal]$row.ticks -le 0){throw 'Invalid tick/generation correlation.'}
    }
    $marks=@{}
    foreach($stage in @('accepted','enqueued','worker','terminal')) {
        $matching=@($edges | Where-Object {$_.stage -eq $stage -and $_.edge -eq 'mark' -and $null -eq $_.file_index})
        if($matching.Count -ne 1){throw "Missing/duplicate $stage boundary."}
        $marks[$stage]=[decimal]$matching[0].ticks
    }
    if(-not ($marks.accepted -le $marks.enqueued -and $marks.enqueued -le $marks.worker -and $marks.worker -le $marks.terminal)){throw 'Unordered operation boundaries.'}
    $spans=[Collections.Generic.List[object]]::new()
    $spanGroups=@($edges | Where-Object edge -in @('begin','end') | Group-Object -Property stage,file_index)
    foreach($group in $spanGroups) {
        $begin=@($group.Group | Where-Object edge -eq begin)
        $end=@($group.Group | Where-Object edge -eq end)
        if($begin.Count -ne 1 -or $end.Count -ne 1){throw 'Incomplete/duplicate span.'}
        $startTick=[decimal]$begin[0].ticks;$endTick=[decimal]$end[0].ticks
        if($startTick -gt $endTick -or $startTick -lt $marks.worker -or $endTick -gt $marks.terminal){throw 'Unordered span ticks.'}
        $spans.Add(@{stage=$begin[0].stage;file_index=$begin[0].file_index;start_ticks=$startTick;end_ticks=$endTick;ms=[double](($endTick-$startTick)*1000/[decimal]$rows[0].frequency)})
    }
    foreach($required in @('prepare','initial_stat','stream','sync','stored_size','post_transfer_stat','verify','remote_hash','local_hash','publish')) {
        if(@($spans | Where-Object stage -eq $required).Count -ne 1){throw "Missing $required span."}
    }
    $verification=@($spans | Where-Object stage -eq verify)[0]
    foreach($hash in @($spans | Where-Object stage -in @('remote_hash','local_hash'))) {
        if($hash.start_ticks -lt $verification.start_ticks -or $hash.end_ticks -gt $verification.end_ticks){throw 'Hash falls outside verification.'}
    }
    return @{header=$rows[0];generation=$generation;job_id=$JobId;marks=$marks;spans=$spans.ToArray();footer=$footers[0]}
}

function Join-TransferRun {
    param([Parameter(Mandatory)][hashtable]$UiResult,[AllowNull()][hashtable]$Trace)
    $result=@{}+$UiResult
    $frequency=[decimal]$UiResult.clock_frequency
    if($frequency -le 0){throw 'Invalid host clock.'}
    $start=[decimal]$UiResult.ui_start_ticks;$end=[decimal]$UiResult.ui_end_ticks
    if($end -lt $start){throw 'Unordered UI clock.'}
    $result.ui_ms=[double](($end-$start)*1000/$frequency)
    $result.backend_ms=$null;$result.stages=@{};$result.valid=($UiResult.outcome -eq 'completed');$result.invalid_reason=''
    if($null -eq $Trace) {
        if($UiResult.build -ne 'released'){throw 'Instrumented build has no trace.'}
        return $result
    }
    if($Trace.header.clock -ne 'qpc' -or [decimal]$Trace.header.frequency -ne $frequency -or $Trace.job_id -cne $UiResult.job_id){throw 'Clock/job mismatch.'}
    if($Trace.marks.accepted -lt $start -or $Trace.marks.terminal -gt $end){throw 'UI did not bracket operation.'}
    if($UiResult.kind -eq 'move') {
        foreach($required in @('move_stat','delete','index')) {
            if(@($Trace.spans | Where-Object stage -eq $required).Count -ne 1){throw "Incomplete Move $required."}
        }
    }
    $result.backend_ms=[double](($Trace.marks.terminal-$Trace.marks.accepted)*1000/$frequency)
    $result.queue_ms=[double](($Trace.marks.worker-$Trace.marks.enqueued)*1000/$frequency)
    $result.request_to_accept_ms=[double](($Trace.marks.accepted-$start)*1000/$frequency)
    $result.terminal_to_visible_ms=[double](($end-$Trace.marks.terminal)*1000/$frequency)
    foreach($span in $Trace.spans){$result.stages[$span.stage]=$span.ms}
    return $result
}
function Get-TransferRunPaths {
    param([Parameter(Mandatory)][string]$TrialRoot,[Parameter(Mandatory)][string]$RunId,[Parameter(Mandatory)][string]$RemoteName)
    if($RunId -cnotmatch '^[a-f0-9]{32}$' -or $RemoteName -cne [IO.Path]::GetFileName($RemoteName)){throw 'Invalid owned run path components.'}
    $runRoot=Assert-TransferLocalPath -Root $TrialRoot -Path (Join-Path $TrialRoot ('runs/'+$RunId.Substring(0,16)))
    $destination=Join-Path $runRoot 'd'
    if((Join-Path $destination $RemoteName).Length -ge 260){throw 'Trial root is too long for native Windows publication; no device mutation permitted.'}
    return @{root=$runRoot;destination=$destination}
}
Export-ModuleMember -Function Assert-TransferLocalPath,Read-TransferTrial,Get-TransferVerdict,Read-TransferTrace,Join-TransferRun,Assert-TransferDriverPin,Get-TransferCompletedRun,Get-TransferRunPaths,Get-TransferFutility,Set-TransferStageStatus
