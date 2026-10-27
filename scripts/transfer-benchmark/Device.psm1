Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
Import-Module (Join-Path $PSScriptRoot 'Core.psm1')
$script:AdbAdapter=$null

function Quote-TransferShell([string]$Value) {
    if($Value.Contains([char]0)){throw 'NUL shell argument.'}
    return "'"+$Value.Replace("'",("'"+[char]34+"'"+[char]34+"'"))+"'"
}
function Invoke-TransferAdb {
    param([hashtable]$Trial,[string[]]$Arguments,[int]$TimeoutSeconds=600)
    if($null -ne $script:AdbAdapter){return & $script:AdbAdapter $Trial $Arguments}
    if(-not [IO.Path]::IsPathFullyQualified($Trial.adb_path) -or -not (Test-Path -LiteralPath $Trial.adb_path -PathType Leaf)){throw 'Real absolute ADB executable required.'}
    $info=[Diagnostics.ProcessStartInfo]::new($Trial.adb_path)
    $info.UseShellExecute=$false;$info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true
    foreach($arg in @('-s',$Trial.serial)+$Arguments){$info.ArgumentList.Add($arg)}
    $process=[Diagnostics.Process]::new();$process.StartInfo=$info
    try {
        if(-not $process.Start()){throw 'ADB client did not start.'}
        $output=$process.StandardOutput.ReadToEndAsync();$errors=$process.StandardError.ReadToEndAsync()
        if(-not $process.WaitForExit($TimeoutSeconds*1000)){
            $process.Kill();$process.WaitForExit();throw 'Owned ADB client timed out.'
        }
        $stdout=$output.GetAwaiter().GetResult();$stderr=$errors.GetAwaiter().GetResult()
        if($process.ExitCode -ne 0){throw "ADB client failed: $stderr"}
        return $stdout
    } finally {$process.Dispose()}
}
function Invoke-TransferHelper {
    param([hashtable]$Trial,[string[]]$Arguments)
    if($Trial.helper_path -notmatch '^/data/local/tmp/takedock-files-[a-f0-9]{64}$' -and $null -eq $script:AdbAdapter){throw 'Helper is not digest-pinned.'}
    $command=(@(Quote-TransferShell $Trial.helper_path)+@($Arguments | ForEach-Object {Quote-TransferShell $_})) -join ' '
    $output=Invoke-TransferAdb -Trial $Trial -Arguments @('shell','-T',$command)
    if($Arguments[0] -eq 'list'){return $output}
    return $output.Trim()
}
function Assert-TransferCapacity {
    param([decimal]$AvailableBytes,[decimal]$RequiredBytes)
    if($AvailableBytes -lt $RequiredBytes -or $RequiredBytes -lt 0){throw 'Insufficient space for preserved trial.'}
}
function Assert-TransferBackup {
    param([hashtable]$Trial,[hashtable]$Fixture)
    if($Trial.schema -ne 1 -or $Trial.trial_id -notmatch '^[a-f0-9]{32}$' -or $Fixture.remote_name -notmatch ('^TDPerf-'+[regex]::Escape($Trial.trial_id)+'-[a-zA-Z0-9-]+\.mp4$')){throw 'Stale/unowned fixture.'}
    $backup=Assert-TransferLocalPath -Root $Trial.root -Path $Fixture.backup
    if($Fixture.decoded -ne $true -or -not (Test-Path -LiteralPath $backup -PathType Leaf)){throw 'Verified decoded backup required.'}
    if((Get-Item -LiteralPath $backup).Length -ne [long]$Fixture.bytes -or (Get-FileHash -LiteralPath $backup -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Fixture.sha256){throw 'Backup size/digest changed.'}
}
function Assert-TransferSource {
    param([Parameter(Mandatory)][hashtable]$Trial,[Parameter(Mandatory)][string]$FixtureId)
    $matches=@($Trial.fixtures | Where-Object id -CEQ $FixtureId)
    if($matches.Count -ne 1){throw 'Ambiguous/unowned source selection.'}
    $fixture=$matches[0]
    Assert-TransferBackup -Trial $Trial -Fixture $fixture
    $identity=Invoke-TransferHelper -Trial $Trial -Arguments @('stat',$fixture.remote_name)
    if($identity -cne $fixture.source_identity){throw 'Source identity changed; no mutation authorized.'}
    $digest=Invoke-TransferHelper -Trial $Trial -Arguments @('hash',$fixture.remote_name,$fixture.source_identity)
    if($digest -cne $fixture.sha256){throw 'Source bytes differ from preserved backup.'}
    return $fixture
}
function New-TransferFixture {
    param([Parameter(Mandatory)][string]$SeedVideo,[Parameter(Mandatory)][long]$TargetBytes,[Parameter(Mandatory)][string]$Output)
    if(-not [IO.Path]::IsPathFullyQualified($SeedVideo) -or -not [IO.Path]::IsPathFullyQualified($Output)){throw 'Absolute fixture paths required.'}
    $null=Assert-TransferLocalPath -Root (Split-Path $SeedVideo -Parent) -Path $SeedVideo
    $null=Assert-TransferLocalPath -Root (Split-Path $Output -Parent) -Path $Output
    $seedLength=(Get-Item -LiteralPath $SeedVideo).Length
    $padding=$TargetBytes-$seedLength
    if($padding -lt 8 -or $padding -gt [uint32]::MaxValue){throw 'Target cannot hold seed and valid free box.'}
    $source=[IO.File]::OpenRead($SeedVideo)
    $destination=$null
    try {
        $destination=[IO.File]::Open($Output,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
        $source.CopyTo($destination,1048576)
        $header=[byte[]]::new(8)
        for($i=0;$i -lt 4;$i++){$header[$i]=[byte](($padding -shr (24-$i*8)) -band 255)}
        [Text.Encoding]::ASCII.GetBytes('free').CopyTo($header,4)
        $destination.Write($header,0,8)
        $buffer=[byte[]]::new(4*1024*1024)
        $remaining=$padding-8
        while($remaining -gt 0) {
            [Security.Cryptography.RandomNumberGenerator]::Fill($buffer)
            $count=[int][math]::Min($remaining,$buffer.Length)
            $destination.Write($buffer,0,$count);$remaining-=$count
        }
        $destination.Flush($true)
    } finally {if($null -ne $destination){$destination.Dispose()};$source.Dispose()}
}
function Write-TransferJournal {
    param([hashtable]$Trial,[hashtable]$Entry)
    $path=Assert-TransferLocalPath -Root $Trial.root -Path $Trial.journal
    $record=@{}+$Entry;$record.utc=[DateTimeOffset]::UtcNow.ToString('O');$record.trial_id=$Trial.trial_id
    [IO.File]::AppendAllText($path,($record | ConvertTo-Json -Compress -Depth 20)+[Environment]::NewLine)
}
function Get-TransferDeviceSpace([hashtable]$Trial) {
    $output=Invoke-TransferAdb -Trial $Trial -Arguments @('shell','df','-k','/sdcard')
    $last=@($output -split '\r?\n' | Where-Object {$_ -match '^/|^emulated'})[-1]
    $columns=@($last.Trim() -split '\s+')
    return ([decimal]$columns[3]*1024)
}
function Get-TransferConditions {
    param([Parameter(Mandatory)][hashtable]$Trial)
    return @{
        battery=(Invoke-TransferAdb -Trial $Trial -Arguments @('shell','dumpsys','battery'))
        thermal=(Invoke-TransferAdb -Trial $Trial -Arguments @('shell','dumpsys','thermalservice'))
        available_bytes=[string](Get-TransferDeviceSpace $Trial)
        host_os=[Environment]::OSVersion.VersionString
        powershell=$PSVersionTable.PSVersion.ToString()
        host_available_bytes=[string]([IO.DriveInfo]::new([IO.Path]::GetPathRoot($Trial.root))).AvailableFreeSpace
    }
}
function Get-TransferAccessibility {
    param([Parameter(Mandatory)][hashtable]$Trial)
    return @{
        enabled=(Invoke-TransferAdb -Trial $Trial -Arguments @('shell','settings','get','secure','accessibility_enabled')).Trim()
        services=(Invoke-TransferAdb -Trial $Trial -Arguments @('shell','settings','get','secure','enabled_accessibility_services')).Trim()
    }
}
function Assert-TransferAccessibilityPreserved {
    param([hashtable]$Trial,[hashtable]$Before)
    $after=Get-TransferAccessibility -Trial $Trial
    Write-TransferJournal -Trial $Trial -Entry @{event='stage_accessibility_verified';before=$Before;after=$after;unchanged=($after.enabled -ceq $Before.enabled -and $after.services -ceq $Before.services)}
    if($after.enabled -cne $Before.enabled -or $after.services -cne $Before.services){throw 'Accessibility changed during this stage; observations retained without restoring settings.'}
}
function Get-TransferUnrelatedSnapshot {
    param([Parameter(Mandatory)][hashtable]$Trial)
    $listing=Invoke-TransferHelper -Trial $Trial -Arguments @('list')
    if(-not $listing){return}
    $parts=$listing.Split([char]0)
    if($parts[-1] -ne '' -or ($parts.Count-1)%2 -ne 0){throw 'Incomplete unrelated-file listing.'}
    for($i=0;$i -lt $parts.Count-1;$i+=2) {
        $name=$parts[$i]
        if($name.StartsWith("TDPerf-$($Trial.trial_id)-",[StringComparison]::Ordinal)){continue}
        $metadata=$parts[$i+1] | ConvertFrom-Json -AsHashtable
        $identity=Invoke-TransferHelper -Trial $Trial -Arguments @('stat',$name)
        if($metadata.ready) {$digest=Invoke-TransferHelper -Trial $Trial -Arguments @('hash',$name,$identity)}
        else {
            $remote=Quote-TransferShell ('/sdcard/DCIM/OpenCamera/'+$name)
            $output=Invoke-TransferAdb -Trial $Trial -Arguments @('shell','-T',"sha256sum $remote")
            if($output -notmatch '^([a-f0-9]{64})\s'){throw 'Unrelated digest unavailable.'}
            $digest=$Matches[1]
        }
        if($digest -notmatch '^[a-f0-9]{64}$' -or (Invoke-TransferHelper -Trial $Trial -Arguments @('stat',$name)) -cne $identity){throw 'Unrelated source changed during preservation snapshot.'}
        @{name=$name;identity=$identity;sha256=$digest}
    }
}
function Assert-TransferUnrelatedPreserved {
    param([Parameter(Mandatory)][hashtable]$Trial)
    $after=@(Get-TransferUnrelatedSnapshot -Trial $Trial)
    foreach($before in $Trial.unrelated_before) {
        $matches=@($after | Where-Object name -CEQ $before.name)
        if($matches.Count -ne 1 -or $matches[0].identity -cne $before.identity -or $matches[0].sha256 -cne $before.sha256){throw 'Unrelated file preservation could not be confirmed.'}
    }
    Write-TransferJournal -Trial $Trial -Entry @{event='unrelated_verified';files=$after}
}
function Assert-TransferTrialSpace([hashtable]$Trial) {
    $drive=[IO.DriveInfo]::new([IO.Path]::GetPathRoot($Trial.root))
    Assert-TransferCapacity -AvailableBytes $drive.AvailableFreeSpace -RequiredBytes 6000000000
    Assert-TransferCapacity -AvailableBytes (Get-TransferDeviceSpace $Trial) -RequiredBytes 1500000000
}
function Copy-TransferPhoneFixture {
    param([hashtable]$Trial,[string]$SourceFixtureId,[hashtable]$Fixture)
    Assert-TransferBackup -Trial $Trial -Fixture $Fixture
    $masters=@($Trial.fixtures | Where-Object id -CEQ $SourceFixtureId)
    if($masters.Count -ne 1 -or $masters[0].role -cne 'master'){throw 'One owned master is required.'}
    $master=$masters[0]
    if($Fixture.role -cne 'move_duplicate' -or $Fixture.id -notmatch '^move-[a-f0-9]{32}$' -or $Fixture.remote_name -cne "TDPerf-$($Trial.trial_id)-$($Fixture.id).mp4" -or @($Trial.fixtures | Where-Object id -CEQ $Fixture.id).Count -ne 0 -or $Fixture.remote_name -ceq $master.remote_name){throw 'Fresh owned Move duplicate required.'}
    if($Fixture.bytes -cne $master.bytes -or $Fixture.sha256 -cne $master.sha256){throw 'Duplicate metadata differs from preserved master.'}
    $null=Assert-TransferSource -Trial $Trial -FixtureId $SourceFixtureId
    if($Trial.helper_path -notmatch '^/data/local/tmp/takedock-files-[a-f0-9]{64}$' -and $null -eq $script:AdbAdapter){throw 'Helper is not digest-pinned.'}
    Write-TransferJournal -Trial $Trial -Entry @{event='copy_planned';creation_method='identity_bound_on_phone_read';source_fixture_id=$SourceFixtureId;fixture=$Fixture}
    $target=Quote-TransferShell ('/sdcard/DCIM/OpenCamera/'+$Fixture.remote_name)
    $guard="test ! -e $target && test ! -L $target"
    $null=Invoke-TransferAdb -Trial $Trial -Arguments @('shell','-T',$guard)
    $read=(@(Quote-TransferShell $Trial.helper_path)+@('read',$master.remote_name,$master.source_identity | ForEach-Object {Quote-TransferShell $_})) -join ' '
    $null=Invoke-TransferAdb -Trial $Trial -Arguments @('shell','-T',"$guard && (set -C; $read > $target)")
    $identity=Invoke-TransferHelper -Trial $Trial -Arguments @('stat',$Fixture.remote_name)
    $metadata=$identity | ConvertFrom-Json -AsHashtable
    $original=$master.source_identity | ConvertFrom-Json -AsHashtable
    if([long]$metadata.size -ne [long]$Fixture.bytes -or ($metadata.device -eq $original.device -and $metadata.inode -eq $original.inode)){throw 'Duplicate does not have a fresh exact-size identity.'}
    $digest=Invoke-TransferHelper -Trial $Trial -Arguments @('hash',$Fixture.remote_name,$identity)
    if($digest -cne $Fixture.sha256 -or (Invoke-TransferHelper -Trial $Trial -Arguments @('stat',$Fixture.remote_name)) -cne $identity){throw 'Duplicate failed full digest/identity verification.'}
    return $identity
}
function Add-TransferPhoneFixture {
    param([hashtable]$Trial,[string]$Id,[string]$Backup,[string]$Role,[string]$FfmpegPath,[string]$SourceFixtureId)
    $name="TDPerf-$($Trial.trial_id)-$Id.mp4"
    $fixture=@{id=$Id;remote_name=$name;backup=$Backup;role=$Role;bytes=[string](Get-Item -LiteralPath $Backup).Length;sha256=(Get-FileHash -LiteralPath $Backup -Algorithm SHA256).Hash.ToLowerInvariant();decoded=$false;source_identity=''}
    & $FfmpegPath -nostdin -v error -i $Backup -map 0 -f null - *> $null
    if($LASTEXITCODE -ne 0){throw 'Fixture full decode failed.'}
    $fixture.decoded=$true
    if($SourceFixtureId) {
        $fixture.creation_method='identity_bound_on_phone_read'
        $fixture.source_identity=Copy-TransferPhoneFixture -Trial $Trial -SourceFixtureId $SourceFixtureId -Fixture $fixture
    } else {
        $fixture.creation_method='adb_push'
        Write-TransferJournal -Trial $Trial -Entry @{event='push_planned';fixture=$fixture}
        $remote='/sdcard/DCIM/OpenCamera/'+$name
        $quoted=Quote-TransferShell $remote
        $null=Invoke-TransferAdb -Trial $Trial -Arguments @('shell','-T',"test ! -e $quoted && test ! -L $quoted")
        $null=Invoke-TransferAdb -Trial $Trial -Arguments @('push',$Backup,$remote)
        $fixture.source_identity=Invoke-TransferHelper -Trial $Trial -Arguments @('stat',$name)
    }
    $Trial.fixtures=@($Trial.fixtures)+@($fixture)
    $null=Assert-TransferSource -Trial $Trial -FixtureId $Id
    Write-TransferJournal -Trial $Trial -Entry @{event='fixture_ready';fixture=$fixture}
    return $fixture
}
function Prepare-TransferTrial {
    param([Parameter(Mandatory)][string]$TrialRoot,[Parameter(Mandatory)][string]$AdbPath,[Parameter(Mandatory)][string]$Serial,[Parameter(Mandatory)][string[]]$Seeds,[Parameter(Mandatory)][string]$Resources,[Parameter(Mandatory)][string]$FfmpegPath)
    $root=[IO.Path]::GetFullPath($TrialRoot)
    $null=Assert-TransferLocalPath -Root $root -Path $root -AllowRoot
    if($Seeds.Count -ne 2){throw 'Two newly owned seed videos required.'}
    foreach($seed in $Seeds){$null=Assert-TransferLocalPath -Root $root -Path $seed}
    $id=([IO.Path]::GetFileName($root) -replace '^device-','')
    if($id -notmatch '^[a-f0-9]{32}$'){throw 'Fresh GUID trial directory required.'}
    $manifest=Join-Path $root 'manifest.json'
    if((Test-Path -LiteralPath $manifest) -or (Test-Path -LiteralPath (Join-Path $root 'journal.jsonl'))){throw 'Existing or interrupted trial must not be prepared again.'}
    $trial=@{schema=1;trial_id=$id;root=$root;serial=$Serial;adb_path=$AdbPath;helper_path='';destination=(Join-Path $root 'outputs');journal=(Join-Path $root 'journal.jsonl');fixtures=@()}
    try {
        $trial.accessibility_before=Get-TransferAccessibility -Trial $trial
        $trial.conditions_before=Get-TransferConditions -Trial $trial
        $drive=[IO.DriveInfo]::new([IO.Path]::GetPathRoot($root))
        Assert-TransferCapacity -AvailableBytes $drive.AvailableFreeSpace -RequiredBytes 9000000000
        Assert-TransferCapacity -AvailableBytes (Get-TransferDeviceSpace $trial) -RequiredBytes 3500000000
        $abi=(Invoke-TransferAdb -Trial $trial -Arguments @('shell','getprop','ro.product.cpu.abi')).Trim()
        if($abi -notin @('arm64-v8a','armeabi-v7a','x86_64')){throw 'Unsupported phone ABI.'}
        $helper=Join-Path $Resources "file-helper/$abi/takedock-files"
        $digest=(Get-FileHash -LiteralPath $helper -Algorithm SHA256).Hash.ToLowerInvariant()
        $trial.helper_path="/data/local/tmp/takedock-files-$digest"
        $quoted=Quote-TransferShell $trial.helper_path
        $presentHash=(Invoke-TransferAdb -Trial $trial -Arguments @('shell','-T',"if [ -e $quoted ]; then sha256sum $quoted; fi")).Trim()
        if($presentHash -and -not $presentHash.StartsWith($digest+' ',[StringComparison]::Ordinal)){throw 'Pinned helper path contains unexpected bytes.'}
        if(-not $presentHash) {
            $null=Invoke-TransferAdb -Trial $trial -Arguments @('push',$helper,$trial.helper_path)
            $null=Invoke-TransferAdb -Trial $trial -Arguments @('shell','chmod','700',$trial.helper_path)
        }
        $remoteHash=(Invoke-TransferAdb -Trial $trial -Arguments @('shell','sha256sum',$trial.helper_path)).Trim().Split(' ')[0]
        if($remoteHash -cne $digest){throw 'Deployed helper does not match measured build.'}
        $trial.unrelated_before=@(Get-TransferUnrelatedSnapshot -Trial $trial)
        Write-TransferJournal -Trial $trial -Entry @{event='unrelated_baseline';files=$trial.unrelated_before}
        New-Item -ItemType Directory -Path $trial.destination | Out-Null
        for($i=0;$i -lt 2;$i++) {
            $bytes=@(500000000,1000000000)[$i]
            $backup=Join-Path $root "master-$bytes.mp4"
            New-TransferFixture -SeedVideo $Seeds[$i] -TargetBytes $bytes -Output $backup
            $null=Add-TransferPhoneFixture -Trial $trial -Id "master-$bytes" -Backup $backup -Role master -FfmpegPath $FfmpegPath
        }
        $trial | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $manifest
        return $trial
    } finally {Complete-TransferTrial -Trial $trial}
}
function Complete-TransferTrial {
    param([Parameter(Mandatory)][hashtable]$Trial)
    $errors=[Collections.Generic.List[object]]::new()
    foreach($key in @('KEYCODE_HOME','KEYCODE_SLEEP')) {
        try {$null=Invoke-TransferAdb -Trial $Trial -Arguments @('shell','input','keyevent',$key) -TimeoutSeconds 10}
        catch {$errors.Add($_)}
    }
    if($errors.Count){throw 'Phone cleanup could not confirm Home and screen sleep; connection may be unavailable.'}
}
Export-ModuleMember -Function Invoke-TransferAdb,Invoke-TransferHelper,Assert-TransferCapacity,Assert-TransferSource,New-TransferFixture,Prepare-TransferTrial,Complete-TransferTrial,Write-TransferJournal,Add-TransferPhoneFixture,Assert-TransferTrialSpace,Get-TransferUnrelatedSnapshot,Assert-TransferUnrelatedPreserved,Get-TransferConditions,Get-TransferAccessibility,Assert-TransferAccessibilityPreserved
