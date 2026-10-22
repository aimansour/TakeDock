$ErrorActionPreference = 'Stop'
$script:Cases = 0
function Assert-TransferTest([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Assert-TransferReject([scriptblock]$Action) {
    $rejected = $false
    try { & $Action | Out-Null } catch { $rejected = $true }
    Assert-TransferTest $rejected 'Expected refusal.'
}
function Test-TransferCase([string]$Name, [scriptblock]$Action) {
    & $Action
    $script:Cases++
    Write-Output "PASS: $Name"
}
function New-TransferTestRoot {
    $root = Join-Path ([IO.Path]::GetTempPath()) ('takedock-transfer-test-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $root | Out-Null
    return $root
}
function New-TransferTestTrial([string]$Root) {
    $id = [guid]::NewGuid().ToString('N')
    $backup = Join-Path $Root 'backup.mp4'
    [IO.File]::WriteAllBytes($backup, [byte[]](1,2,3,4))
    return @{
        schema = 1; trial_id = $id; root = $Root; serial = 'test-only';
        adb_path = 'test-only'; helper_path = '/data/local/tmp/test-helper';
        destination = (Join-Path $Root 'outputs'); journal = (Join-Path $Root 'journal.jsonl');
        fixtures = @(@{
            id = 'master-500'; remote_name = "TDPerf-$id-master-500.mp4";
            source_identity = '{"device":1,"inode":2,"size":4,"modified_ns":1,"changed_ns":1}';
            bytes = '4'; sha256 = (Get-FileHash -LiteralPath $backup -Algorithm SHA256).Hash.ToLowerInvariant();
            backup = $backup; decoded = $true; role = 'master'
        })
    }
}
