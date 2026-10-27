$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot 'Guard.psm1') -Force
function Reject([scriptblock]$Action) {
    $rejected=$false
    try {& $Action | Out-Null} catch {$rejected=$true}
    if(-not $rejected){throw 'An unowned or ambiguous handoff manifest was accepted.'}
}
$trial=Join-Path ([IO.Path]::GetTempPath()) ('takedock-handoff-guard-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path $trial
$manifest=Join-Path $trial 'manifest.json'
$id=[guid]::NewGuid().ToString('N')
$name="TDHandoff-$id-small.mp4"
$backup=Join-Path $trial 'reference.mp4'
[IO.File]::WriteAllBytes($backup,[byte[]](1,2,3))
function Valid {
    @{
        schema=1;kind='phone-handoff';trial_id=$id;root=$trial
        source_root='/sdcard/DCIM/OpenCamera';handoff_root='/sdcard/DCIM/TakeDock'
        fixtures=@(@{id='small';name=$name;identity='{"device":9007199254740993,"inode":9007199254740995,"size":3,"modified_ns":1700000000123456789,"changed_ns":1700000000123456790}';bytes=3;sha256=('a'*64);backup=$backup})
        operations=@(@{id='single';kind='mark';fixtures=@('small');restart_before=$false})
    }
}
function Read([hashtable]$Value) {
    $Value | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $manifest
    Read-HandoffManifest -Manifest $manifest -TrialRoot $trial
}
function ValidTrace {
    $rows=@(@{record='header';schema=1;clock='qpc';frequency='1000';process=123})
    foreach($boundary in @(@('accepted','mark','10',$null),@('enqueued','mark','11',$null),@('worker','mark','12',$null),@('prepare','begin','13',$null),@('prepare','end','14',$null),@('initial_stat','begin','15',0),@('initial_stat','end','16',0),@('handoff','begin','17',0),@('handoff','end','18',0),@('terminal','mark','19',$null))) {
        $rows+=@(@{record='edge';job_id='sample';generation='3';stage=$boundary[0];edge=$boundary[1];ticks=$boundary[2];file_index=$boundary[3]})
    }
    $rows+=@(@{record='job_complete';job_id='sample';generation='3';dropped=0;writer_status='ok'})
    return $rows
}
$trace=Join-Path $trial 'trace-test.jsonl'
function Trace([object[]]$Rows,[string]$Job='sample') {
    $Rows | ForEach-Object {ConvertTo-Json -InputObject $_ -Compress} | Set-Content -LiteralPath $trace
    Read-HandoffTrace -Path $trace -JobId $Job -Generation '3' -Files 1 -Frequency 1000
}
try {
    $firstTrace=New-HandoffTracePath -TrialRoot $trial
    $secondTrace=New-HandoffTracePath -TrialRoot $trial
    if($firstTrace -ceq $secondTrace){throw 'Restarted native sessions reused a create-new trace path.'}
    $good=Read (Valid)
    if($good.fixtures[0].identity -cne (Valid).fixtures[0].identity){throw 'Opaque identity was changed.'}
    $bad=Valid; $bad.kind='transfer-performance'; Reject {Read $bad}
    $bad=Valid; $bad.fixtures[0].name='TDPerf-'+$id+'-master.mp4'; Reject {Read $bad}
    $bad=Valid; $bad.fixtures[0].backup=Join-Path (Split-Path $trial -Parent) 'outside.mp4'; Reject {Read $bad}
    $bad=Valid; $bad.fixtures[0].bytes=4; Reject {Read $bad}
    $bad=Valid; $bad.fixtures+=@($bad.fixtures[0]); Reject {Read $bad}
    $bad=Valid; $bad.operations[0].fixtures=@('missing'); Reject {Read $bad}
    $bad=Valid; $bad.operations[0].kind='copy'; Reject {Read $bad}
    $bad=Valid; $bad.operations[0].kind='collision'; Reject {Read $bad}
    $collision=Valid; $collision.operations[0].kind='collision'
    $collision.fixtures[0].collision_sha256=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes('TDHandoff-'+$id+'-collision'))).ToLowerInvariant()
    $null=Read $collision
    $bad=Valid; $bad.operations+=@($bad.operations[0]); Reject {Read $bad}
    $bad=Valid; $bad.handoff_root='/sdcard/DCIM/OpenCamera'; Reject {Read $bad}
    $bad=Valid; $bad.root=Split-Path $trial -Parent; Reject {Read $bad}
    $timing=Trace @(ValidTrace)
    if($timing.backend_ms -ne 9 -or $timing.handoff_ack_ticks[0] -cne '18'){throw 'Handoff timing boundaries were not preserved.'}
    Reject {Trace @((ValidTrace) | Where-Object record -NE job_complete)}
    $broken=@(ValidTrace);$broken[-1].dropped=1;Reject {Trace $broken}
    $broken=@(ValidTrace);$broken[-1].generation='4';Reject {Trace $broken}
    $broken=@(ValidTrace);$broken[0].frequency='2000';Reject {Trace $broken}
    Reject {Trace @((ValidTrace) | Where-Object {-not ($_.record -eq 'edge' -and $_.stage -eq 'handoff' -and $_.edge -eq 'end')})}
    Reject {Trace @(ValidTrace) 'other-job'}
    $broken=@(ValidTrace);$broken+=$broken[1];Reject {Trace $broken}
    $broken=@(ValidTrace);foreach($edge in @((ValidTrace) | Where-Object {$_.ContainsKey('stage') -and $_.stage -in @('initial_stat','handoff')})){$edge.file_index=1;$broken+=@($edge)};Reject {Trace $broken}
    'PASS: fresh handoff manifests bind exact owned sources, references and operations; old trials cannot authorize mutations.'
} finally {
    $resolved=[IO.Path]::GetFullPath($trial)
    $allowed=[IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\')+'\takedock-handoff-guard-'
    if(-not $resolved.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)){throw 'Guard-test cleanup escaped its temporary root.'}
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
