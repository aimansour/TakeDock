. (Join-Path $PSScriptRoot 'TestSupport.ps1')
Import-Module (Join-Path $PSScriptRoot 'Core.psm1') -Force

function New-Runs([double[]]$Candidate = @(90,91,92)) {
    foreach ($bytes in @('500000000','1000000000')) {
        foreach ($kind in @('copy','move')) {
            for ($index=0; $index -lt 3; $index++) {
                foreach ($build in @('baseline','candidate')) {
                    $time = if ($build -eq 'baseline') { 100 + $index } else { $Candidate[$index] }
                    @{
                        run_id = [guid]::NewGuid().ToString('N'); job_id = [guid]::NewGuid().ToString();
                        bytes=$bytes; kind=$kind; build=$build; pair=$index+1;
                        order= if (($index % 2 -eq 0) -eq ($build -eq 'baseline')) {0} else {1};
                        warmup=$false; backend_ms=$time; ui_ms=$time; stages=@{};
                        valid=$true; invalid_reason=''; outcome='completed'; observation_uncertainty_ms=1
                    }
                }
            }
        }
    }
}
Test-TransferCase 'all four groups improve beyond range and uncertainty' {
    Assert-TransferTest (Get-TransferVerdict -Runs @(New-Runs) -UncertaintyMs 1).accepted 'Expected measured gain.'
}
Test-TransferCase 'one slower paired result rejects apparent median gain' {
    Assert-TransferTest (-not (Get-TransferVerdict -Runs @(New-Runs @(90,110,91)) -UncertaintyMs 1).accepted) 'Slower pair accepted.'
}
Test-TransferCase 'fast hashes cannot hide regressed user total' {
    $runs = @(New-Runs)
    $runs[1].ui_ms = 150
    $runs[1].stages = @{remote_hash_ms=1;local_hash_ms=1}
    Assert-TransferTest (-not (Get-TransferVerdict -Runs $runs -UncertaintyMs 1).accepted) 'UI regression accepted.'
}
Test-TransferCase 'missing and duplicate pairs fail' {
    $runs=@(New-Runs)
    Assert-TransferTest (-not (Get-TransferVerdict -Runs $runs[0..22] -UncertaintyMs 1).accepted) 'Missing pair accepted.'
    Assert-TransferTest (-not (Get-TransferVerdict -Runs @($runs + @($runs[0])) -UncertaintyMs 1).accepted) 'Duplicate accepted.'
}
Test-TransferCase 'warmups uncertainty invalid outcomes and nonfinite totals do not pass' {
    foreach ($change in @('warmup','uncertainty','failed','nan')) {
        $runs=@(New-Runs)
        switch($change) {
            warmup {$runs[0].warmup=$true}
            uncertainty {$runs[0].observation_uncertainty_ms=20}
            failed {$runs[0].outcome='failed';$runs[0].valid=$false}
            nan {$runs[0].backend_ms=[double]::NaN}
        }
        Assert-TransferTest (-not (Get-TransferVerdict -Runs $runs -UncertaintyMs 1).accepted) "Accepted $change."
    }
}
Test-TransferCase 'manifest stays inside exact root and never accepts reparse ancestors' {
    $root=New-TransferTestRoot
    $trial=New-TransferTestTrial $root
    $manifest=Join-Path $root 'manifest.json'
    $trial | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $manifest
    $null=Read-TransferTrial -Manifest $manifest -TrialRoot $root
    $trial.destination=Join-Path (Split-Path $root -Parent) 'outside'
    $trial | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $manifest
    Assert-TransferReject {Read-TransferTrial -Manifest $manifest -TrialRoot $root}
    $outside=New-TransferTestRoot
    $link=Join-Path $root 'junction'
    if($IsWindows) {New-Item -ItemType Junction -Path $link -Target $outside | Out-Null}
    else {New-Item -ItemType SymbolicLink -Path $link -Target $outside | Out-Null}
    Assert-TransferReject {Assert-TransferLocalPath -Root $root -Path (Join-Path $link 'never.mp4')}
}

function Write-TestTrace([string]$Path) {
    $rows=@(@{schema=1;record='header';clock='qpc';frequency='1000';process=1})
    foreach($mark in @(@('accepted',10),@('enqueued',11),@('worker',12),@('terminal',100))) {
        $rows+=@{record='edge';job_id='job';generation='9007199254740993';file_index=$null;stage=$mark[0];edge='mark';ticks=[string]$mark[1]}
    }
    $tick=13
    foreach($stage in @('prepare','initial_stat','stream','sync','stored_size','post_transfer_stat','verify','remote_hash','local_hash','publish')) {
        $fileIndex=if($stage -eq 'prepare'){$null}else{0}
        foreach($edge in @('begin','end')) {
            $edgeTick=if($stage -eq 'verify' -and $edge -eq 'end'){30}else{$tick}
            $rows+=@{record='edge';job_id='job';generation='9007199254740993';file_index=$fileIndex;stage=$stage;edge=$edge;ticks=[string]$edgeTick}
            $tick++
        }
    }
    $rows+=@{record='job_complete';job_id='job';generation='9007199254740993';dropped=0;writer_status='ok'}
    $rows | ForEach-Object {ConvertTo-Json -InputObject $_ -Compress -Depth 8} | Set-Content -LiteralPath $Path
}
Test-TransferCase 'trace requires a complete job and exact generation correlation' {
    $path=Join-Path (New-TransferTestRoot) 'trace.jsonl'
    Write-TestTrace $path
    $trace=Read-TransferTrace -Path $path -JobId job
    Assert-TransferTest ($trace.generation -eq '9007199254740993') 'Identity precision lost.'
    $lines=@(Get-Content -LiteralPath $path)
    $lines[0..($lines.Count-2)] | Set-Content -LiteralPath $path
    Assert-TransferReject {Read-TransferTrace -Path $path -JobId job}
    Write-TestTrace $path
    $raw=[IO.File]::ReadAllText($path).Replace('"ticks":"14"','"ticks":"1"')
    [IO.File]::WriteAllText($path,$raw)
    Assert-TransferReject {Read-TransferTrace -Path $path -JobId job}
}
Test-TransferCase 'UI and trace require same host frequency' {
    $path=Join-Path (New-TransferTestRoot) 'trace.jsonl'
    Write-TestTrace $path
    $trace=Read-TransferTrace -Path $path -JobId job
    $ui=@{job_id='job';clock_frequency='2000';ui_start_ticks='1';ui_end_ticks='110';kind='copy';build='candidate';bytes='500000000';outcome='completed';observation_uncertainty_ms=1}
    Assert-TransferReject {Join-TransferRun -UiResult $ui -Trace $trace}
}
Test-TransferCase 'both hashes are inside the enclosing verification interval' {
    $path=Join-Path (New-TransferTestRoot) 'trace.jsonl'
    Write-TestTrace $path
    $rows=@(Get-Content -LiteralPath $path | ForEach-Object {$_ | ConvertFrom-Json -AsHashtable})
    @($rows | Where-Object {$_.record -eq 'edge' -and $_.stage -eq 'verify' -and $_.edge -eq 'end'})[0].ticks='26'
    $rows | ForEach-Object {ConvertTo-Json -InputObject $_ -Compress -Depth 8} | Set-Content -LiteralPath $path
    Assert-TransferReject {Read-TransferTrace -Path $path -JobId job}
}
Test-TransferCase 'pinned native driver is identified by Cargo receipt without unsupported CLI arguments' {
    $root=New-TransferTestRoot
    $bin=Join-Path $root 'bin';New-Item -ItemType Directory -Path $bin | Out-Null
    $exe=Join-Path $bin 'tauri-driver.exe';[IO.File]::WriteAllBytes($exe,[byte[]](1,2,3))
    $receipt=@{installs=@{'tauri-driver 2.1.0 (registry+https://github.com/rust-lang/crates.io-index)'=@{bins=@('tauri-driver.exe')}}}
    $receipt | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $root '.crates2.json')
    $null=Assert-TransferDriverPin -Executable $exe
    $receipt.installs=@{'tauri-driver 2.2.0 (registry+https://github.com/rust-lang/crates.io-index)'=@{bins=@('tauri-driver.exe')}}
    $receipt | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $root '.crates2.json')
    Assert-TransferReject {Assert-TransferDriverPin -Executable $exe}
}
Test-TransferCase 'resume reuses a verified settled run and refuses unresolved cleanup' {
    $result=@{run_id='done';build='released';bytes='500000000';kind='copy';pair=0;order=0;warmup=$true;outcome='completed';valid=$true}
    $events=@(@{event='run_planned';run=$result;build_sha256=('a'*64)},@{event='run_verified';result=$result})
    Assert-TransferReject {Get-TransferCompletedRun -Events $events -Build released -Sha256 ('a'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true}
    $events+=@{event='run_cleanup_complete';run_id='done'}
    $saved=Get-TransferCompletedRun -Events $events -Build released -Sha256 ('a'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true
    Assert-TransferTest ($saved.run_id -ceq 'done') 'Completed operation would be repeated.'
    Assert-TransferReject {Get-TransferCompletedRun -Events $events -Build released -Sha256 ('b'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true}
}
Test-TransferCase 'native benchmark publication paths fit legacy Windows limits before any device mutation' {
    $temporary=New-TransferTestRoot
    $root=Join-Path $temporary ('r'*([math]::Max(1,140-$temporary.Length-1)))
    New-Item -ItemType Directory -Path $root | Out-Null
    $id=[guid]::NewGuid().ToString('N')
    $name='TDPerf-'+('a'*32)+'-move-'+$id+'.mp4'
    $paths=Get-TransferRunPaths -TrialRoot $root -RunId $id -RemoteName $name
    Assert-TransferTest ((Join-Path $paths.destination $name).Length -lt 260) 'Benchmark creates an unpublishable Windows path.'
    $tooLong=Join-Path $root ('x'*100)
    Assert-TransferReject {Get-TransferRunPaths -TrialRoot $tooLong -RunId $id -RemoteName $name}
    Assert-TransferReject {Get-TransferRunPaths -TrialRoot $root -RunId '../escape' -RemoteName $name}
}

function New-FutilityPair([double]$Backend=90,[double]$Ui=91) {
    return @(@{build='baseline';bytes='500000000';kind='copy';pair=1;order=0;warmup=$false;valid=$true;outcome='completed';backend_ms=100;ui_ms=101;observation_uncertainty_ms=1},@{build='candidate';bytes='500000000';kind='copy';pair=1;order=1;warmup=$false;valid=$true;outcome='completed';backend_ms=$Backend;ui_ms=$Ui;observation_uncertainty_ms=1})
}
Test-TransferCase 'one positive measured pair continues without claiming acceptance' {
    $decision=Get-TransferFutility -Runs (New-FutilityPair)
    Assert-TransferTest (-not $decision.stop -and $decision.matched_pairs -eq 1) 'Positive pair stopped or no pair matched.'
    Assert-TransferTest (-not $decision.ContainsKey('accepted')) 'Partial futility decision masqueraded as acceptance.'
}
Test-TransferCase 'any equal or slower backend or native UI total permanently rejects candidate' {
    foreach($times in @(@(100,91),@(110,91),@(90,101),@(90,110))) {
        $decision=Get-TransferFutility -Runs (New-FutilityPair $times[0] $times[1])
        Assert-TransferTest ($decision.stop -and $decision.matched_pairs -eq 1 -and $decision.failures.Count -ge 1) 'Non-improving pair did not stop.'
    }
}
Test-TransferCase 'a later positive pair never erases an earlier valid non-improving pair' {
    $runs=@(New-FutilityPair 110 91)
    $next=@(New-FutilityPair 50 51)
    $next[0].pair=2;$next[0].order=1;$next[1].pair=2;$next[1].order=0
    $decision=Get-TransferFutility -Runs @($runs+$next)
    Assert-TransferTest ($decision.stop -and $decision.matched_pairs -eq 2) 'Earlier failure disappeared.'
}
Test-TransferCase 'warmups cannot trigger futility or stand in for measured pairs' {
    $runs=@(New-FutilityPair 200 201)
    foreach($run in $runs){$run.warmup=$true;$run.pair=0}
    $decision=Get-TransferFutility -Runs $runs
    Assert-TransferTest (-not $decision.stop -and $decision.matched_pairs -eq 0) 'Warmup decided measured outcome.'
}
Test-TransferCase 'incomplete mismatched duplicate invalid and nonfinite pairs are refused as evidence' {
    foreach($change in @('incomplete','group','pair','duplicate','order','failed','nan','negative')) {
        $runs=@(New-FutilityPair)
        switch($change) {
            incomplete {$runs=@($runs[0])}
            group {$runs[1].kind='move'}
            pair {$runs[1].pair=2}
            duplicate {$runs+=@($runs[0])}
            order {$runs[1].order=0}
            failed {$runs[1].valid=$false;$runs[1].outcome='failed'}
            nan {$runs[1].backend_ms=[double]::NaN}
            negative {$runs[1].ui_ms=-1}
        }
        Assert-TransferReject {Get-TransferFutility -Runs $runs}
    }
}

Test-TransferCase 'comparison receipt scope never reuses old calibration warmups' {
    $result=@{run_id='done';build='baseline';bytes='500000000';kind='copy';pair=0;order=0;warmup=$true;outcome='completed';valid=$true}
    $events=@(@{event='run_planned';run=$result;build_sha256=('a'*64)},@{event='run_verified';result=$result},@{event='run_cleanup_complete';run_id='done'})
    $saved=Get-TransferCompletedRun -Events $events -Build baseline -Sha256 ('a'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true -Stage Compare
    Assert-TransferTest ($null -eq $saved) 'Calibration warmup crossed comparison stage.'
    $result.stage='Compare'
    $saved=Get-TransferCompletedRun -Events $events -Build baseline -Sha256 ('a'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true -Stage Compare
    Assert-TransferTest ($saved.run_id -ceq 'done') 'A settled comparison would repeat.'
    Assert-TransferReject {Get-TransferCompletedRun -Events $events -Build baseline -Sha256 ('b'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true -Stage Compare}
}

Test-TransferCase 'failed stage preservation cannot leave an accepted timing report' {
 $report=@{decision='accepted';verdict=@{accepted=$true;reasons=@()};runs=@(@{valid=$true;ui_ms=100;backend_ms=90})}
 Set-TransferStageStatus -Report $report -Valid $false -Reason 'Accessibility changed during stage.'
 Assert-TransferTest (-not $report.stage_valid -and $report.decision -ceq 'invalid_stage' -and -not $report.verdict.accepted) 'Invalid environment still accepted.'
 Assert-TransferTest ($report.stage_invalid_reason -ceq 'Accessibility changed during stage.' -and $report.runs[0].ui_ms -eq 100 -and $report.runs[0].valid) 'Raw byte/clock observations were rewritten or reason lost.'
 Assert-TransferReject {Set-TransferStageStatus -Report $report -Valid $true}
}
Test-TransferCase 'successful stage preservation retains a truthful rejected timing verdict' {
 $report=@{decision='rejected_futility';verdict=@{accepted=$false;reasons=@('Non-improving pair')};runs=@()}
 Set-TransferStageStatus -Report $report -Valid $true
 Assert-TransferTest ($report.stage_valid -and $report.decision -ceq 'rejected_futility' -and -not $report.verdict.accepted) 'Preservation replaced the timing rejection.'
}
Test-TransferCase 'invalid stage requires a preserved diagnostic reason' {
 $report=@{decision='accepted';verdict=@{accepted=$true;reasons=@()};runs=@()}
 Assert-TransferReject {Set-TransferStageStatus -Report $report -Valid $false -Reason ''}
}

Test-TransferCase 'settled operations from an invalid stage never resume as valid performance evidence' {
 $result=@{run_id='done';stage='Compare';build='baseline';bytes='500000000';kind='copy';pair=0;order=0;warmup=$true;outcome='completed';valid=$true}
 $events=@(@{event='run_planned';run=$result;build_sha256=('a'*64)},@{event='run_verified';result=$result},@{event='run_cleanup_complete';run_id='done'},@{event='stage_invalid';stage='Compare';run_ids=@('done');reason='Services changed'})
 Assert-TransferReject {Get-TransferCompletedRun -Events $events -Build baseline -Sha256 ('a'*64) -Bytes 500000000 -Kind copy -Pair 0 -Order 0 -Warmup $true -Stage Compare}
}
Write-Output "PASS: $script:Cases core benchmark cases."
