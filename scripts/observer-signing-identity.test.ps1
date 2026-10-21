$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'observer-signing-identity.ps1')
$expected='7c16891706df11bcacce93150eec72679b0b23e09f1c40b689185e07f0f0e5b4'
$other='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
$cases=@(
    @{Name='legacy signer'; Output=@("Signer #1 certificate SHA-256 digest: $expected"); Pass=$true},
    @{Name='observed Build-Tools37 V2 signer'; Output=@("V2 Signer: certificate SHA-256 digest: $expected"); Pass=$true},
    @{Name='same identity across signing schemes'; Output=@("V2 Signer: certificate SHA-256 digest: $expected", "V3 Signer: certificate SHA-256 digest: $expected"); Pass=$true},
    @{Name='other certificate'; Output=@("V2 Signer: certificate SHA-256 digest: $other"); Pass=$false},
    @{Name='second unexpected signer'; Output=@("Signer #1 certificate SHA-256 digest: $expected", "Signer #2 certificate SHA-256 digest: $other"); Pass=$false},
    @{Name='unexpected identity in another scheme'; Output=@("V2 Signer: certificate SHA-256 digest: $expected", "V3 Signer: certificate SHA-256 digest: $other"); Pass=$false},
    @{Name='missing signer'; Output=@('Verifies'); Pass=$false},
    @{Name='malformed digest'; Output=@('V2 Signer: certificate SHA-256 digest: invalid'); Pass=$false},
    @{Name='unknown signer format beside trusted one'; Output=@("Signer #1 certificate SHA-256 digest: $expected", "New Signer: certificate SHA-256 digest: $other"); Pass=$false},
    @{Name='source stamp is not the APK signer'; Output=@("Source Stamp Signer certificate SHA-256 digest: $other", "V2 Signer: certificate SHA-256 digest: $expected"); Pass=$true}
)
$failed=0
foreach($case in $cases){
    $passed=$true
    try {Assert-ObserverSigningIdentity -Output $case.Output -Expected $expected}
    catch {$passed=$false}
    if($passed -ne $case.Pass){$failed++; Write-Output "FAIL: $($case.Name)"}
}
if($failed){throw "$failed/$($cases.Count) signing identity cases failed"}
Write-Output "PASS: $($cases.Count)/$($cases.Count) strict signing identity cases."
