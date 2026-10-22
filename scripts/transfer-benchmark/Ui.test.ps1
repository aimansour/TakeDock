. (Join-Path $PSScriptRoot 'TestSupport.ps1')
Import-Module (Join-Path $PSScriptRoot 'Ui.psm1') -Force
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
Write-Output "PASS: $script:Cases UI-measurement cases."
