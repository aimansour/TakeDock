param([string]$NdkRoot = $env:ANDROID_NDK_HOME)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $NdkRoot) { throw 'Set ANDROID_NDK_HOME to the installed NDK 30.0.16248370 directory.' }
$projectRoot = Split-Path $PSScriptRoot -Parent
$hostFolder = if ($IsWindows) { 'windows-x86_64' } elseif ($IsMacOS) { 'darwin-x86_64' } else { 'linux-x86_64' }
$extension = if ($IsWindows) { '.cmd' } else { '' }
$targets = @(
    @{Target='aarch64-linux-android'; Clang='aarch64-linux-android36'; Abi='arm64-v8a'},
    @{Target='x86_64-linux-android'; Clang='x86_64-linux-android36'; Abi='x86_64'},
    @{Target='armv7-linux-androideabi'; Clang='armv7a-linux-androideabi36'; Abi='armeabi-v7a'}
)
Push-Location $projectRoot
try {
    $metadata = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }
    foreach ($target in $targets) {
        $linker = Join-Path $NdkRoot "toolchains/llvm/prebuilt/$hostFolder/bin/$($target.Clang)-clang$extension"
        if (-not (Test-Path -LiteralPath $linker)) { throw "NDK linker missing: $($target.Clang)" }
        $variable = 'CARGO_TARGET_' + $target.Target.Replace('-','_').ToUpperInvariant() + '_LINKER'
        $previous = [Environment]::GetEnvironmentVariable($variable,'Process')
        try {
            [Environment]::SetEnvironmentVariable($variable,$linker,'Process')
            & cargo build --locked --release -p takedock-files --target $target.Target
            if ($LASTEXITCODE -ne 0) { throw "Helper build failed: $($target.Target)" }
            $destination = Join-Path $projectRoot "src-tauri/resources/file-helper/$($target.Abi)"
            New-Item -ItemType Directory -Path $destination -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $metadata.target_directory "$($target.Target)/release/takedock-files") -Destination (Join-Path $destination 'takedock-files')
        } finally { [Environment]::SetEnvironmentVariable($variable,$previous,'Process') }
    }
} finally { Pop-Location }
