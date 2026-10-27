Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
Import-Module (Join-Path $PSScriptRoot '../transfer-benchmark/Core.psm1')
function Read-HandoffManifest {
    param([string]$Manifest,[string]$TrialRoot)
    $null=Assert-TransferLocalPath -Root $TrialRoot -Path $TrialRoot -AllowRoot
    $null=Assert-TransferLocalPath -Root $TrialRoot -Path $Manifest
    $trial=Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json -AsHashtable
    if($trial.schema -ne 1 -or $trial.kind -cne 'phone-handoff' -or $trial.trial_id -notmatch '^[a-f0-9]{32}$') {throw 'Fresh handoff manifest required; no transfer journal can authorize this trial.'}
    if([IO.Path]::GetFullPath($trial.root) -cne [IO.Path]::GetFullPath($TrialRoot)){throw 'Manifest belongs to another trial root.'}
    if($trial.source_root -cne '/sdcard/DCIM/OpenCamera' -or $trial.handoff_root -cne '/sdcard/DCIM/TakeDock'){throw 'Fixed separate phone roots required.'}
    $ids=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $names=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    if($trial.fixtures.Count -lt 1 -or $trial.fixtures.Count -gt 100){throw 'Invalid owned fixture count.'}
    foreach($fixture in $trial.fixtures) {
        if($fixture.id -notmatch '^[a-zA-Z0-9-]{1,80}$' -or -not $ids.Add($fixture.id) -or -not $names.Add($fixture.name)){throw 'Duplicate/invalid fixture ownership.'}
        if($fixture.name -notmatch ('^TDHandoff-'+[regex]::Escape($trial.trial_id)+'-[a-zA-Z0-9-]+\.mp4$')){throw 'Unowned phone source.'}
        if($fixture.sha256 -notmatch '^[a-f0-9]{64}$' -or [long]$fixture.bytes -le 0){throw 'Invalid reference digest/size.'}
        $backup=Assert-TransferLocalPath -Root $TrialRoot -Path $fixture.backup
        if(-not (Test-Path -LiteralPath $backup -PathType Leaf) -or (Get-Item -LiteralPath $backup).Length -ne [long]$fixture.bytes){throw 'Reference backup missing or changed.'}
        $identity=[regex]::Match($fixture.identity,'^\{"device":\d+,"inode":\d+,"size":(?<size>\d+),"modified_ns":\d+,"changed_ns":\d+\}$')
        if(-not $identity.Success -or $identity.Groups['size'].Value -cne [string]$fixture.bytes){throw 'Exact source identity/size required.'}
    }
    $operations=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $used=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    if($trial.operations.Count -lt 1 -or $trial.operations.Count -gt 100){throw 'Invalid operation count.'}
    foreach($operation in $trial.operations) {
        if($operation.id -notmatch '^[a-zA-Z0-9-]{1,80}$' -or -not $operations.Add($operation.id) -or $operation.kind -cnotin @('mark','rename','delete','collision')){throw 'Invalid or repeated operation.'}
        if($operation.fixtures.Count -lt 1 -or ($operation.kind -in @('rename','collision') -and $operation.fixtures.Count -ne 1)){throw 'Invalid source selection.'}
        foreach($id in $operation.fixtures) {if(-not $ids.Contains($id) -or -not $used.Add($id)){throw 'Unknown or reused source selection; mutations are never retried.'}}
        if($operation.kind -eq 'collision') {
            $fixture=@($trial.fixtures | Where-Object id -CEQ $operation.fixtures[0])[0]
            $payload=[Text.Encoding]::UTF8.GetBytes('TDHandoff-'+$trial.trial_id+'-collision')
            $expected=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($payload)).ToLowerInvariant()
            if($fixture.collision_sha256 -cne $expected){throw 'Collision fixture must have its own known backup digest.'}
        }
        if($operation.kind -eq 'rename' -and ($operation.stem -notmatch ('^TDHandoff-'+[regex]::Escape($trial.trial_id)+'-[a-zA-Z0-9-]+$'))){throw 'Rename must stay in this trial namespace.'}
    }
    return $trial
}
function New-HandoffTracePath { param([string]$TrialRoot) Join-Path $TrialRoot ('trace-'+[guid]::NewGuid().ToString('N')+'.jsonl') }
function Read-HandoffTrace {
    param([string]$Path,[string]$JobId,[string]$Generation,[ValidateRange(1,10000)][int]$Files=1,[long]$Frequency)
    $rows=@(Get-Content -LiteralPath $Path | Where-Object {$_} | ForEach-Object {ConvertFrom-Json -InputObject $_ -AsHashtable})
    if(-not $rows.Count -or $rows[0].record -cne 'header' -or $rows[0].schema -ne 1 -or $rows[0].clock -cne 'qpc' -or $rows[0].frequency -notmatch '^[1-9][0-9]*$' -or [decimal]$rows[0].frequency -ne $Frequency -or $Frequency -le 0 -or @($rows | Where-Object record -EQ header).Count -ne 1){throw 'Invalid or mismatched QPC trace header.'}
    $selected=@($rows | Where-Object {$_.record -ne 'header' -and $_.job_id -ceq $JobId})
    $footers=@($selected | Where-Object record -EQ job_complete)
    if($footers.Count -ne 1 -or $footers[0].dropped -ne 0 -or $footers[0].writer_status -cne 'ok' -or [string]$footers[0].generation -cne $Generation){throw 'Matching complete loss-free trace required.'}
    $edges=@($selected | Where-Object record -EQ edge)
    foreach($edge in $edges){if([string]$edge.generation -cne $Generation -or $edge.ticks -notmatch '^[0-9]+$' -or [decimal]$edge.ticks -le 0){throw 'Invalid trace identity or tick.'}}
    $marks=@{}
    foreach($stage in @('accepted','enqueued','worker','terminal')) {
        $mark=@($edges | Where-Object {$_.stage -ceq $stage -and $_.edge -ceq 'mark' -and $null -eq $_.file_index})
        if($mark.Count -ne 1){throw 'Missing or duplicate operation boundary.'}
        $marks[$stage]=[decimal]$mark[0].ticks
    }
    if(-not ($marks.accepted -le $marks.enqueued -and $marks.enqueued -le $marks.worker -and $marks.worker -le $marks.terminal)){throw 'Unordered operation trace.'}
    $spans=@()
    foreach($group in @($edges | Where-Object edge -In @('begin','end') | Group-Object -Property stage,file_index)) {
        $begin=@($group.Group | Where-Object edge -EQ begin);$end=@($group.Group | Where-Object edge -EQ end)
        if($begin.Count -ne 1 -or $end.Count -ne 1){throw 'Incomplete or duplicate trace span.'}
        $from=[decimal]$begin[0].ticks;$to=[decimal]$end[0].ticks
        if($from -lt $marks.worker -or $to -lt $from -or $to -gt $marks.terminal){throw 'Unordered span trace.'}
        $spans+=@(@{stage=$begin[0].stage;file_index=$begin[0].file_index;begin=$from;end=$to})
    }
    if(@($spans | Where-Object {$_.stage -ceq 'prepare' -and $null -eq $_.file_index}).Count -ne 1){throw 'Preparation span missing.'}
    foreach($stage in @('initial_stat','handoff')) {
        if(@($spans | Where-Object stage -CEQ $stage).Count -ne $Files){throw 'Unexpected per-file trace count.'}
    }
    $acks=@()
    for($index=0;$index -lt $Files;$index++) {
        $initial=@($spans | Where-Object {$_.stage -ceq 'initial_stat' -and $_.file_index -eq $index})
        $handoff=@($spans | Where-Object {$_.stage -ceq 'handoff' -and $_.file_index -eq $index})
        if($initial.Count -ne 1 -or $handoff.Count -ne 1 -or $initial[0].end -gt $handoff[0].begin){throw 'Per-file handoff evidence missing or unordered.'}
        $acks+=@([string]$handoff[0].end)
    }
    return @{path=$Path;job_id=$JobId;generation=$Generation;frequency=[string]$Frequency;accepted_ticks=[string]$marks.accepted;terminal_ticks=[string]$marks.terminal;handoff_ack_ticks=$acks;backend_ms=[double](1000*($marks.terminal-$marks.accepted)/$Frequency);acceptance_to_handoff_ack_ms=@($acks | ForEach-Object {[double](1000*([decimal]$_-$marks.accepted)/$Frequency)});loss_free=$true}
}
Export-ModuleMember -Function Read-HandoffManifest,New-HandoffTracePath,Read-HandoffTrace
