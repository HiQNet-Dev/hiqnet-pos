param(
    [string]$NativeDriver = "C:\Users\gaelg\AppData\Local\Temp\edgedriver-157\msedgedriver.exe"
)

$ErrorActionPreference = "Stop"
# Clave publica de la semilla sintetica de pruebas (bytes 0..31).
$env:HIQNET_POS_LICENSE_PUBLIC_KEY = "A6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg="

& (Join-Path $PSScriptRoot "run-e2e.ps1") `
    -Script (Join-Path $PSScriptRoot "e2e_license.py") `
    -NativeDriver $NativeDriver `
    -KeepArtifacts `
    -TimeoutSeconds 600
exit $LASTEXITCODE
