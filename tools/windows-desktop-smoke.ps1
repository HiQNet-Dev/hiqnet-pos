param(
    [string]$Executable = "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe",
    [int]$StartupSeconds = 10
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$exePath = Join-Path $repoRoot $Executable
if (-not (Test-Path $exePath)) {
    throw "No existe el ejecutable: $exePath"
}

$smokeRoot = Join-Path $env:TEMP "hiqnet-pos-smoke"
if (Test-Path $smokeRoot) {
    Remove-Item -Recurse -Force $smokeRoot
}
New-Item -ItemType Directory -Path $smokeRoot | Out-Null

$dbPath = Join-Path $smokeRoot "hiqnet-pos.db"
$env:HIQNET_POS_DB_PATH = $dbPath

$process = Start-Process -FilePath $exePath -PassThru
Start-Sleep -Seconds $StartupSeconds

$running = -not $process.HasExited
$dbExists = Test-Path $dbPath
$dbSize = if ($dbExists) { (Get-Item $dbPath).Length } else { 0 }

if (-not $process.HasExited) {
    Stop-Process -Id $process.Id
}

Write-Output "exe=$exePath"
Write-Output "running_after_startup=$running"
Write-Output "db_created=$dbExists"
Write-Output "db_size_bytes=$dbSize"

if (-not $running -or -not $dbExists -or $dbSize -le 0) {
    throw "Smoke test del ejecutable falló"
}

Write-Output "OK: el ejecutable inició y creó una base SQLite real."
