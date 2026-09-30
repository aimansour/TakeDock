$ErrorActionPreference='Stop'
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    foreach($command in @(@('run','check'),@('run','test:ui'),@('run','build'))){
        & npm @command
        if ($LASTEXITCODE -ne 0){throw 'Frontend verification failed.'}
    }
} finally {Pop-Location}
