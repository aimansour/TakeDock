function Assert-ObserverSigningIdentity {
    param([string[]]$Output, [string]$Expected)
    if($Expected -notmatch '^[0-9a-fA-F]{64}$'){throw 'Invalid expected Observer certificate digest'}
    $signers=0
    foreach($line in $Output){
        if($line -notmatch 'certificate SHA-256 digest:'){continue}
        if($line -match '^Source Stamp Signer:? certificate SHA-256 digest: [0-9a-fA-F]{64}$'){continue}
        if($line -notmatch '^(?:Signer #[1-9][0-9]*|V[1-4](?:\.[0-9]+)? Signer:) certificate SHA-256 digest: ([0-9a-fA-F]{64})$'){
            throw 'Unrecognized Observer certificate digest output'
        }
        $signers++
        if($Matches[1] -ne $Expected){throw 'Observer signing identity changed'}
    }
    if(-not $signers){throw 'Observer signing certificate digest missing'}
}
