. (Join-Path $PSScriptRoot 'TestSupport.ps1')
Import-Module (Join-Path $PSScriptRoot 'Device.psm1') -Force -DisableNameChecking

Test-TransferCase 'replacement and missing preservation receipts stop before mutation' {
    $root=New-TransferTestRoot
    $trial=New-TransferTestTrial $root
    $calls=[Collections.Generic.List[object]]::new()
    & (Get-Module Device) {param($calls)
        $script:AdbAdapter={param($trial,$arguments) $calls.Add($arguments); return '{"device":1,"inode":99,"size":4,"modified_ns":1,"changed_ns":1}'}.GetNewClosure()
    } $calls
    Assert-TransferReject {Assert-TransferSource -Trial $trial -FixtureId master-500}
    Assert-TransferTest ($calls.Count -eq 1) 'Replacement reached additional commands.'
    $calls.Clear()
    $trial.fixtures[0].decoded=$false
    Assert-TransferReject {Assert-TransferSource -Trial $trial -FixtureId master-500}
    Assert-TransferTest ($calls.Count -eq 0) 'Missing decode reached device.'
    $trial.fixtures[0].decoded=$true
    $trial.fixtures[0].sha256='0'*64
    Assert-TransferReject {Assert-TransferSource -Trial $trial -FixtureId master-500}
    Assert-TransferTest ($calls.Count -eq 0) 'Invalid backup reached device.'
}
Test-TransferCase 'stale fixture ownership and low space prevent preparation' {
    $root=New-TransferTestRoot
    $trial=New-TransferTestTrial $root
    $trial.fixtures[0].remote_name='personal.mp4'
    Assert-TransferReject {Assert-TransferSource -Trial $trial -FixtureId master-500}
    Assert-TransferReject {Assert-TransferCapacity -AvailableBytes 999 -RequiredBytes 1000}
}
Test-TransferCase 'fixture construction is bounded exact-size and exclusive' {
    $root=New-TransferTestRoot
    $seed=Join-Path $root 'seed.mp4'
    [IO.File]::WriteAllBytes($seed,[byte[]](0,0,0,8,102,116,121,112))
    $output=Join-Path $root 'new.mp4'
    New-TransferFixture -SeedVideo $seed -TargetBytes 1048576 -Output $output
    Assert-TransferTest ((Get-Item -LiteralPath $output).Length -eq 1048576) 'Wrong size.'
    $stream=[IO.File]::OpenRead($output)
    try {
        $null=$stream.Seek(8,[IO.SeekOrigin]::Begin)
        $header=[byte[]]::new(8);$null=$stream.Read($header,0,8)
        Assert-TransferTest ([Text.Encoding]::ASCII.GetString($header,4,4) -eq 'free') 'Missing valid padding box.'
        $sample=[byte[]]::new(65536);$null=$stream.Read($sample,0,$sample.Length)
        Assert-TransferTest (@($sample | Select-Object -Unique).Count -gt 240) 'Trivial/compressible padding.'
    } finally {$stream.Dispose()}
    Assert-TransferReject {New-TransferFixture -SeedVideo $seed -TargetBytes 1048576 -Output $output}
    Assert-TransferReject {New-TransferFixture -SeedVideo $seed -TargetBytes 15 -Output (Join-Path $root 'too-small.mp4')}
}
Test-TransferCase 'phone cleanup always leaves camera and sleeps without severing ADB' {
    $calls=[Collections.Generic.List[string]]::new()
    & (Get-Module Device) {param($calls)
        $script:AdbAdapter={param($trial,$arguments) $calls.Add(($arguments -join ' ')); return ''}.GetNewClosure()
    } $calls
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    Complete-TransferTrial -Trial $trial
    Assert-TransferTest (($calls -join '|') -match 'KEYCODE_HOME.*KEYCODE_SLEEP') 'Camera/screen cleanup missing.'
    Assert-TransferTest (($calls -join '|') -notmatch 'disconnect|kill-server|unpair|reboot|poweroff') 'Cleanup severed device connection.'
}
Test-TransferCase 'unrelated file content or identity changes fail preservation' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $state=@{identity='{"device":1,"inode":9,"size":7,"modified_ns":1,"changed_ns":1}';digest=('a'*64)}
    & (Get-Module Device) {param($state)
        $script:AdbAdapter={param($trial,$arguments)
            $command=$arguments -join ' '
            if($command -match "'list'"){return "personal.mp4`0{`"ready`":true}`0"}
            if($command -match "'stat'"){return $state.identity}
            if($command -match "'hash'"){return $state.digest}
            throw 'Unexpected adapter request.'
        }.GetNewClosure()
    } $state
    $trial.unrelated_before=@(Get-TransferUnrelatedSnapshot -Trial $trial)
    Assert-TransferUnrelatedPreserved -Trial $trial
    $state.digest='b'*64
    Assert-TransferReject {Assert-TransferUnrelatedPreserved -Trial $trial}
    $state.digest='a'*64;$state.identity='{"device":1,"inode":999,"size":7,"modified_ns":1,"changed_ns":1}'
    Assert-TransferReject {Assert-TransferUnrelatedPreserved -Trial $trial}
}
Test-TransferCase 'NUL listing preserves leading whitespace in an unrelated name' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    & (Get-Module Device) {
        $script:AdbAdapter={param($trial,$arguments)
            $command=$arguments -join ' '
            if($command -match "'list'"){return " personal'clip.mp4`0{`"ready`":true}`0"}
            if($command -match "'stat'"){return '{"device":1,"inode":9,"size":7,"modified_ns":1,"changed_ns":1}'}
            if($command -match "'hash'"){return 'a'*64}
            throw 'Unexpected adapter request.'
        }
    }
    $snapshot=@(Get-TransferUnrelatedSnapshot -Trial $trial)
    Assert-TransferTest ($snapshot[0].name -ceq " personal'clip.mp4") 'Listing trimmed an unrelated filename.'
}


function New-CloneCase([string]$Mode='valid') {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $master=$trial.fixtures[0]
    $target=@{}+$master
    $target.id='move-0123456789abcdef0123456789abcdef'
    $target.remote_name="TDPerf-$($trial.trial_id)-$($target.id).mp4"
    $target.role='move_duplicate';$target.source_identity=''
    $state=@{mode=$Mode;calls=[Collections.Generic.List[string]]::new();reads=0;target=$target;master=$master;identity='{"device":1,"inode":3,"size":4,"modified_ns":2,"changed_ns":2}'}
    & (Get-Module Device) {param($state)
        $script:AdbAdapter={param($trial,$arguments)
            $command=$arguments -join ' ';$state.calls.Add($command)
            if($command -match "'read'") {
                $state.reads++
                if($state.mode -eq 'read_failed'){throw 'Owned helper read failed.'}
                if(-not $command.Contains('set -C') -or -not $command.Contains(' > ') -or -not $command.Contains($state.master.source_identity) -or -not $command.Contains($state.master.remote_name) -or -not $command.Contains($state.target.remote_name)){throw 'Missing exclusive identity-bound creation.'}
                $events=@(Get-Content $trial.journal | ForEach-Object {$_ | ConvertFrom-Json -AsHashtable})
                if(@($events | Where-Object event -EQ copy_planned).Count -ne 1){throw 'Creation was not journaled before read.'}
                return ''
            }
            if($command -match 'test ! -e') {if($state.mode -eq 'collision'){throw 'Target already exists.'};return ''}
            if($command -match "'stat'") {
                if($command.Contains($state.master.remote_name)) {if($state.mode -eq 'replacement'){return $state.identity};return $state.master.source_identity}
                if($command.Contains($state.target.remote_name)){return $state.identity}
            }
            if($command -match "'hash'") {
                if($command.Contains($state.target.remote_name) -and $state.mode -eq 'wrong_digest'){return '0'*64}
                return $state.master.sha256
            }
            throw 'Unexpected external operation.'
        }.GetNewClosure()
    } $state
    return @{trial=$trial;target=$target;state=$state}
}
function Invoke-OwnedClone([hashtable]$Case) {
    return & (Get-Module Device) {param($trial,$target) Copy-TransferPhoneFixture -Trial $trial -SourceFixtureId master-500 -Fixture $target} $Case.trial $Case.target
}
Test-TransferCase 'on-phone duplicate returns only verified fresh identity after exclusive owned creation' {
    $case=New-CloneCase
    $identity=Invoke-OwnedClone $case
    Assert-TransferTest ($identity -ceq '{"device":1,"inode":3,"size":4,"modified_ns":2,"changed_ns":2}') 'Unverified duplicate identity returned.'
    Assert-TransferTest ($case.state.reads -eq 1) 'Copy retried or missing.'
    Assert-TransferTest (@($case.state.calls | Where-Object {$_ -match '^push|delete|rename'}).Count -eq 0) 'Preparation changed transport or source.'
    Assert-TransferTest ($case.trial.fixtures.Count -eq 1) 'Private clone mutated manifest before caller verification.'
}
Test-TransferCase 'source replacement prevents on-phone creation' {
    $case=New-CloneCase replacement
    Assert-TransferReject {Invoke-OwnedClone $case}
    Assert-TransferTest ($case.state.reads -eq 0) 'Replacement was copied.'
}
Test-TransferCase 'changed backup bytes prevent any device access for duplicate preparation' {
    $case=New-CloneCase
    [IO.File]::WriteAllBytes($case.target.backup,[byte[]](4,3,2,1))
    Assert-TransferReject {Invoke-OwnedClone $case}
    Assert-TransferTest ($case.state.calls.Count -eq 0) 'Bad backup reached device.'
}
Test-TransferCase 'existing duplicate target is retained without a read retry' {
    $case=New-CloneCase collision
    Assert-TransferReject {Invoke-OwnedClone $case}
    Assert-TransferTest ($case.state.reads -eq 0) 'Existing target overwritten.'
}
Test-TransferCase 'failed helper acknowledgement cannot produce a ready duplicate or retry' {
    $case=New-CloneCase read_failed
    Assert-TransferReject {Invoke-OwnedClone $case}
    Assert-TransferTest ($case.state.reads -eq 1) 'Failed mutation retried.'
    Assert-TransferTest (@($case.state.calls | Where-Object {$_ -match "'stat'" -and $_.Contains($case.target.remote_name)}).Count -eq 0) 'Failed copy was treated as acknowledged.'
}
Test-TransferCase 'wrong duplicate digest cannot return a selectable identity' {
    $case=New-CloneCase wrong_digest
    Assert-TransferReject {Invoke-OwnedClone $case}
    Assert-TransferTest ($case.state.reads -eq 1) 'Digest failure retried mutation.'
}
Test-TransferCase 'master aliases unowned names and inconsistent backup metadata prevent duplicate creation' {
    foreach($change in @('alias','unowned','role','bytes','digest','existing_id')) {
        $case=New-CloneCase
        switch($change) {
            alias {$case.target.id=$case.trial.fixtures[0].id;$case.target.remote_name=$case.trial.fixtures[0].remote_name}
            unowned {$case.target.remote_name='personal.mp4'}
            role {$case.trial.fixtures[0].role='move_duplicate'}
            bytes {$case.target.bytes='5'}
            digest {$case.target.sha256='0'*64}
            existing_id {$case.trial.fixtures+=@(@{}+$case.target)}
        }
        Assert-TransferReject {Invoke-OwnedClone $case}
        Assert-TransferTest ($case.state.reads -eq 0) "Unsafe target reached read: $change"
    }
}

Test-TransferCase 'stage accessibility preservation uses its current baseline without restoring old settings' {
    $trial=New-TransferTestTrial (New-TransferTestRoot)
    $trial.accessibility_before=@{enabled='1';services='old.service'}
    $state=@{enabled='0';services='null';commands=[Collections.Generic.List[string]]::new()}
    & (Get-Module Device) {param($state)
        $script:AdbAdapter={param($trial,$arguments)
            $command=$arguments -join ' ';$state.commands.Add($command)
            if($command -ceq 'shell settings get secure accessibility_enabled'){return $state.enabled}
            if($command -ceq 'shell settings get secure enabled_accessibility_services'){return $state.services}
            throw 'Unexpected write or operation.'
        }.GetNewClosure()
    } $state
    $before=Get-TransferAccessibility -Trial $trial
    Assert-TransferAccessibilityPreserved -Trial $trial -Before $before
    $state.services='new.service'
    Assert-TransferReject {Assert-TransferAccessibilityPreserved -Trial $trial -Before $before}
    Assert-TransferTest ($trial.accessibility_before.services -ceq 'old.service' -and $state.services -ceq 'new.service') 'Old or current accessibility settings were overwritten.'
}
Write-Output "PASS: $script:Cases device-preservation cases."
