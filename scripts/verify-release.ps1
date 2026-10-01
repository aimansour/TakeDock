param([string]$OutputDirectory='test-results/release')
$ErrorActionPreference='Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    $config=Get-Content src-tauri/tauri.conf.json -Raw|ConvertFrom-Json
    $package=Get-Content package.json -Raw|ConvertFrom-Json
    $version=$config.version
    if($version -notmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$' -or $package.version -ne $version){throw 'Version metadata mismatch'}
    if($env:GITHUB_REF -like 'refs/tags/*' -and $env:GITHUB_REF -ne "refs/tags/v$version"){throw 'Tag does not match metadata'}
    $metadata=& cargo metadata --locked --no-deps --format-version 1|ConvertFrom-Json
    if($LASTEXITCODE -ne 0){throw 'Cargo metadata failed'}
    if(($metadata.packages|Where-Object name -eq 'takedock').version -ne $version){throw 'Cargo version mismatch'}
    if((Get-Content observer/app/build.gradle -Raw) -notmatch "versionName = '$([regex]::Escape($version))'"){throw 'Observer version mismatch'}
    $acceptance=Get-Content 'test-results/desktop/acceptance.json' -Raw|ConvertFrom-Json
    if($acceptance.conclusion -ne 'passed' -or $acceptance.tests.Count -ne 4 -or @($acceptance.tests|Where-Object {-not $_.passed}).Count){throw 'Four passing Desktop E2E cases required'}
    if($acceptance.commit -ne (& git rev-parse HEAD)){throw 'Desktop tests did not run against this commit'}
    $binaryRoot=Join-Path $metadata.target_directory 'release'
    $installers=@(Get-ChildItem -LiteralPath (Join-Path $binaryRoot 'bundle/nsis') -Filter '*-setup.exe')
    if($installers.Count -ne 1){throw 'Exactly one installer required'}
    $installer=$installers[0]
    if(-not(Test-Path -LiteralPath ($installer.FullName+'.sig'))){throw 'Updater signature missing'}
    if(Test-Path -LiteralPath $OutputDirectory){if(@(Get-ChildItem -LiteralPath $OutputDirectory -Force).Count){throw 'Release output directory must be empty'}}
    New-Item -ItemType Directory -Path $OutputDirectory -Force|Out-Null
    if(-not $env:TAKEDOCK_SEVEN_ZIP){./scripts/setup-packaging-tools.ps1}
    $tempBase=if($env:TAKEDOCK_TEST_TMP){$env:TAKEDOCK_TEST_TMP}else{[IO.Path]::GetTempPath()}
    $extracted=Join-Path $tempBase ('takedock-package-'+[guid]::NewGuid().ToString())
    & $env:TAKEDOCK_SEVEN_ZIP x $installer.FullName "-o$extracted" -y | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Installer extraction failed'}
    $files=@(Get-ChildItem -LiteralPath $extracted -File -Recurse)
    foreach($resource in $acceptance.resources){
        $original=Join-Path $binaryRoot $resource.name
        if((Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash -ne $resource.sha256){throw "Tested artifact changed: $($resource.name)"}
        $suffix=$resource.name.Replace('/',[IO.Path]::DirectorySeparatorChar)
        $packaged=@($files|Where-Object {$_.FullName.EndsWith([IO.Path]::DirectorySeparatorChar+$suffix)})
        if($packaged.Count -ne 1 -or (Get-FileHash -LiteralPath $packaged[0].FullName -Algorithm SHA256).Hash -ne $resource.sha256){throw "Packaged artifact differs: $($resource.name)"}
    }
    & cargo run --locked -p takedock --example verify-artifact -- $installer.FullName ($installer.FullName+'.sig') $version
    if($LASTEXITCODE -ne 0){throw 'Final updater signature verification failed'}
    Copy-Item -LiteralPath $installer.FullName,($installer.FullName+'.sig') -Destination $OutputDirectory
    Copy-Item -LiteralPath 'src-tauri/resources/observer.apk' -Destination (Join-Path $OutputDirectory 'TakeDock-Observer.apk')
    Copy-Item -LiteralPath 'test-results/desktop/acceptance.json' -Destination $OutputDirectory
    $manifest=[ordered]@{version=$version;notes="TakeDock $version";pub_date=[DateTime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ');platforms=@{'windows-x86_64'=@{url="https://github.com/aimansour/TakeDock/releases/download/v$version/$($installer.Name)";signature=(Get-Content -LiteralPath ($installer.FullName+'.sig') -Raw).Trim()}}}
    $manifest|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $OutputDirectory 'latest.json')
    $evidence=[ordered]@{version=$version;commit=$acceptance.commit;signature_verified=$true;packaged_resources_match=$true;observer_certificate=(Get-Content observer/signing-certificate.sha256 -Raw).Trim()}
    $evidence|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $OutputDirectory 'release-verification.json')
    $checksums=Get-ChildItem -LiteralPath $OutputDirectory -File|Sort-Object Name|ForEach-Object {"$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $($_.Name)"}
    $checksums|Set-Content -LiteralPath (Join-Path $OutputDirectory 'SHA256SUMS.txt')
    Write-Output 'PASS: tested binary/resources, extracted installer, signed version and release assets verified.'
} finally {Pop-Location}
