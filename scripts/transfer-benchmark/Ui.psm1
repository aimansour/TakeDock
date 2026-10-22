Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
Import-Module (Join-Path $PSScriptRoot 'Core.psm1')
Import-Module (Join-Path $PSScriptRoot 'Device.psm1') -DisableNameChecking

function Invoke-TransferDriver {
    param([hashtable]$Session,[string]$Method,[string]$Path,[AllowNull()][object]$Body=$null)
    $uri=$Session.uri.TrimEnd('/')+'/session/'+$Session.id+$Path
    $arguments=@{Uri=$uri;Method=$Method;TimeoutSec=30;ContentType='application/json'}
    if($null -ne $Body){$arguments.Body=ConvertTo-Json -InputObject $Body -Compress -Depth 30}
    $response=Invoke-WebRequest @arguments
    $result=ConvertFrom-Json -InputObject $response.Content -AsHashtable
    return $result.value
}
function Open-TransferSession {
    param([Parameter(Mandatory)][string]$Executable,[Parameter(Mandatory)][uri]$DriverUri)
    if(-not [IO.Path]::IsPathFullyQualified($Executable) -or -not (Test-Path -LiteralPath $Executable -PathType Leaf)){throw 'Measured executable missing.'}
    $body=@{capabilities=@{alwaysMatch=@{'tauri:options'=@{application=$Executable}}}} | ConvertTo-Json -Depth 10 -Compress
    $response=Invoke-WebRequest -Uri ($DriverUri.AbsoluteUri.TrimEnd('/')+'/session') -Method Post -Body $body -ContentType application/json -TimeoutSec 60
    $value=($response.Content | ConvertFrom-Json -AsHashtable).value
    if(-not $value.sessionId){throw 'Native WebDriver session was not created.'}
    return @{id=$value.sessionId;uri=$DriverUri.AbsoluteUri;exe=$Executable}
}
function Close-TransferSession {
    param([Parameter(Mandatory)][hashtable]$Session)
    $null=Invoke-TransferDriver -Session $Session -Method Delete -Path ''
}
function Get-TransferElement {
    param([hashtable]$Session,[string]$Using,[string]$Value)
    $element=Invoke-TransferDriver -Session $Session -Method Post -Path '/element' -Body @{using=$Using;value=$Value}
    return $element['element-6066-11e4-a52e-4f735466cecf']
}
function Click-TransferElement([hashtable]$Session,[string]$Element) {
    $null=Invoke-TransferDriver -Session $Session -Method Post -Path "/element/$Element/click" -Body @{}
}
function Get-TransferSnapshot([hashtable]$Session) {
    return Invoke-TransferDriver -Session $Session -Method Post -Path '/execute/sync' -Body @{script="return window.__TAURI_INTERNALS__.invoke('bootstrap')";args=@()}
}
function Assert-TransferTerminal {
    param([Parameter(Mandatory)][string]$JobId,[Parameter(Mandatory)][string]$NoticeId,[Parameter(Mandatory)][hashtable]$Snapshot)
    if($NoticeId -cne $JobId){throw 'Native notice belongs to another job.'}
    $matches=@($Snapshot.notices | Where-Object {$_.key.operation_id -ceq $JobId})
    if($matches.Count -ne 1 -or $matches[0].outcome -ne 'completed'){throw 'A matching clean terminal result is required.'}
    $notice=$matches[0]
    if($notice.ContainsKey('count') -and $notice.completed -ne $notice.count){throw 'Incomplete terminal count.'}
    return $notice
}
function Restore-TransferProfile {
    param([Parameter(Mandatory)][string]$Path,[AllowNull()][byte[]]$Before,[Parameter(Mandatory)][byte[]]$LastWritten)
    if(-not (Test-Path -LiteralPath $Path -PathType Leaf)){throw 'Profile disappeared; preserve its original snapshot.'}
    $actual=[IO.File]::ReadAllBytes($Path)
    if([Convert]::ToBase64String($actual) -cne [Convert]::ToBase64String($LastWritten)){throw 'Profile changed outside harness; preserve both versions without replacing it.'}
    if($null -eq $Before){Remove-Item -LiteralPath $Path}
    else {[IO.File]::WriteAllBytes($Path,$Before)}
}
function Invoke-TransferRun {
    param([Parameter(Mandatory)][hashtable]$Session,[Parameter(Mandatory)][hashtable]$Trial,[Parameter(Mandatory)][hashtable]$Run)
    $fixture=Assert-TransferSource -Trial $Trial -FixtureId $Run.fixture_id
    if($Run.kind -notin @('copy','move') -or [string]$Run.bytes -cne $fixture.bytes){throw 'Run does not match the owned payload.'}
    if($Run.kind -eq 'move' -and $fixture.role -ne 'move_duplicate'){throw 'Reusable master may never be moved.'}
    $destination=Assert-TransferLocalPath -Root $Trial.root -Path $Run.destination
    if(-not (Test-Path -LiteralPath $destination -PathType Container) -or @(Get-ChildItem -LiteralPath $destination -Force).Count){throw 'Fresh empty run destination required.'}
    $deadline=[Diagnostics.Stopwatch]::StartNew()
    do {
        $initial=Get-TransferSnapshot $Session
        if($initial.session.connected){break}
        if($deadline.Elapsed.TotalSeconds -gt 30){throw 'Phone session unavailable before timing.'}
        Start-Sleep -Milliseconds 100
    }while($true)
    if([IO.Path]::GetFullPath($initial.settings.destination) -ne $destination){throw 'Native destination differs from journal.'}
    $actions=@{actions=@(@{type='key';id='keyboard';actions=@(
        @{type='keyDown';value=[string][char]0xE009},@{type='keyDown';value=[string][char]0xE008},
        @{type='keyDown';value='v'},@{type='keyUp';value='v'},
        @{type='keyUp';value=[string][char]0xE008},@{type='keyUp';value=[string][char]0xE009}
    )})}
    $null=Invoke-TransferDriver -Session $Session -Method Post -Path '/actions' -Body $actions
    $checkbox=$null
    do {
        try {$checkbox=Get-TransferElement -Session $Session -Using xpath -Value ('//label[span[normalize-space(.)="Select '+$fixture.remote_name+'"]]/input')}
        catch {if($deadline.Elapsed.TotalSeconds -gt 30){throw};Start-Sleep -Milliseconds 100}
    }while(-not $checkbox)
    Click-TransferElement $Session $checkbox
    $checked=@(Invoke-TransferDriver -Session $Session -Method Post -Path '/elements' -Body @{using='css selector';value='tbody input:checked'})
    if($checked.Count -ne 1){throw 'Refusing unexpected native selection.'}
    $name=if($Run.kind -eq 'copy'){'Copy to computer'}else{'Move to computer'}
    $button=Get-TransferElement -Session $Session -Using xpath -Value ('//button[normalize-space(.)="'+$name+'"]')
    $initial=Get-TransferSnapshot $Session
    $seen=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach($job in $initial.jobs){$null=$seen.Add($job.id)}
    $start=[Diagnostics.Stopwatch]::GetTimestamp()
    Click-TransferElement $Session $button
    $watch=[Diagnostics.Stopwatch]::StartNew();$jobId=$null;$uncertainty=0.0
    while($watch.Elapsed.TotalMinutes -lt 30) {
        $observation=[Diagnostics.Stopwatch]::StartNew()
        $snapshot=Get-TransferSnapshot $Session
        $newJobs=@($snapshot.jobs | Where-Object {-not $seen.Contains($_.id)})
        if($newJobs.Count -gt 1){throw 'Concurrent native operation invalidates timing.'}
        if($newJobs.Count -eq 1) {
            $job=$newJobs[0]
            if($job.kind -ne $Run.kind -or $job.count -ne 1 -or $job.generation -ne $initial.session.generation){throw 'Wrong operation/generation.'}
            $jobId=$job.id
            $elements=@(Invoke-TransferDriver -Session $Session -Method Post -Path '/elements' -Body @{using='css selector';value=".file-result-notice[data-operation-id='$jobId']"})
            if($elements.Count -eq 1) {
                $noticeId=Invoke-TransferDriver -Session $Session -Method Get -Path ('/element/'+$elements[0]['element-6066-11e4-a52e-4f735466cecf']+'/attribute/data-operation-id')
                $end=[Diagnostics.Stopwatch]::GetTimestamp()
                $uncertainty=[math]::Max($uncertainty,100+$observation.Elapsed.TotalMilliseconds)
                $null=Assert-TransferTerminal -JobId $jobId -NoticeId $noticeId -Snapshot $snapshot.file_results
                $result=@{}+$Run
                $result.job_id=$jobId;$result.ui_start_ticks=[string]$start;$result.ui_end_ticks=[string]$end
                $result.clock_frequency=[string][Diagnostics.Stopwatch]::Frequency
                $result.outcome='completed';$result.observation_uncertainty_ms=$uncertainty
                return $result
            }
            if($job.status -in @('completed','failed','cancelled') -and -not @($snapshot.file_results.notices | Where-Object {$_.key.operation_id -ceq $jobId}).Count){throw 'Settled result was not observed before expiry.'}
        }
        $uncertainty=[math]::Max($uncertainty,100+$observation.Elapsed.TotalMilliseconds)
        Start-Sleep -Milliseconds 100
    }
    if($jobId) {
        $cancel=Get-TransferElement -Session $Session -Using xpath -Value ('//button[starts-with(normalize-space(.),"Cancel ·")]')
        Click-TransferElement $Session $cancel
        $cancelWait=[Diagnostics.Stopwatch]::StartNew()
        do {
            $snapshot=Get-TransferSnapshot $Session
            if(-not @($snapshot.jobs | Where-Object {$_.id -ceq $jobId -and $_.status -in @('queued','running')}).Count){break}
            if($cancelWait.Elapsed.TotalSeconds -gt 30){throw 'Cancellation cleanup is still pending; preserve trial and stop.'}
            Start-Sleep -Milliseconds 100
        }while($true)
    }
    throw 'Transfer measurement exceeded its deadline.'
}
Export-ModuleMember -Function Open-TransferSession,Close-TransferSession,Invoke-TransferRun,Assert-TransferTerminal,Restore-TransferProfile
