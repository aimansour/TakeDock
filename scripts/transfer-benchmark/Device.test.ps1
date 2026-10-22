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
Write-Output "PASS: $script:Cases device-preservation cases."
