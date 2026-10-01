param([string]$Directory=(Join-Path (Split-Path $PSScriptRoot -Parent) 'test-results/packaging-tools'))
$ErrorActionPreference='Stop'
$Directory=[IO.Path]::GetFullPath($Directory)
New-Item -ItemType Directory -Path $Directory -Force|Out-Null
$downloads=@(
    @{Name='7zr.exe';Hash='AD4C82FADCBDF93C03B4FC440F300509C7D60C5C2F4D183E35D9D70D6957037D'},
    @{Name='7z2603-x64.exe';Hash='0859C524B8A63551848F0C246ABDDCB1D0B7B656B0FBFE879F8D85E61A9E6EDD'}
)
foreach($download in $downloads){
    $path=Join-Path $Directory $download.Name
    if(-not(Test-Path -LiteralPath $path)){Invoke-WebRequest "https://github.com/ip7z/7zip/releases/download/26.03/$($download.Name)" -OutFile $path}
    if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $download.Hash){throw 'Packaging tool hash mismatch'}
}
$portable=Join-Path $Directory '7zip-26.03'
if(-not(Test-Path -LiteralPath (Join-Path $portable '7z.exe'))){
    & (Join-Path $Directory '7zr.exe') x (Join-Path $Directory '7z2603-x64.exe') "-o$portable" -y | Out-Null
    if($LASTEXITCODE -ne 0){throw 'Portable 7-Zip extraction failed'}
}
$env:TAKEDOCK_SEVEN_ZIP=Join-Path $portable '7z.exe'
