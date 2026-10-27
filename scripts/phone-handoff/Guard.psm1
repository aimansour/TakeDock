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
Export-ModuleMember -Function Read-HandoffManifest
