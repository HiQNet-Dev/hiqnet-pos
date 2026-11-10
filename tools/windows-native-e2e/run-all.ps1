#requires -Version 5.1
<#
.SYNOPSIS
    Ejecuta la suite E2E nativa completa y agrega los resultados.

.DESCRIPTION
    Encadena los guiones sobre el ejecutable real, verifica las bases generadas
    y deja un resumen JSON y las capturas en un directorio de artefactos.

    Cada guion usa su propia base temporal aislada (nunca la instalacion real).
    `e2e_flow` y `e2e_returns` se validan despues con `verify_db.py` y
    `verify_returns_db.py`; `restore_multiphase_check.py` se ejecuta sobre la
    base del flujo.

.PARAMETER NativeDriver
    Ruta de msedgedriver.exe de la misma version que el runtime de WebView2.

.PARAMETER AppPath
    Ejecutable compilado con `tauri build --debug --no-bundle`.

.PARAMETER Suite
    Subconjunto de guiones a ejecutar. Por defecto, todos.

.PARAMETER ReportDirectory
    Directorio donde se agregan resumen y capturas.
#>
param(
    [string]$NativeDriver = (Join-Path $env:TEMP "edgedriver\msedgedriver.exe"),

    [string]$AppPath = "",

    [string[]]$Suite = @(),

    [string]$ReportDirectory = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$runner = Join-Path $PSScriptRoot "run-e2e.ps1"
$python = "python"

if (-not $AppPath) {
    $AppPath = Join-Path $repoRoot "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe"
}
if (-not $ReportDirectory) {
    $ReportDirectory = Join-Path $repoRoot "native-e2e-artifacts"
}
New-Item -ItemType Directory -Path $ReportDirectory -Force | Out-Null

# `-File script.ps1 -Suite a,b` llega como una sola cadena: se normaliza aqui.
$Suite = @($Suite | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })

if (-not $Suite -or $Suite.Count -eq 0) {
    $Suite = @(
        "e2e_smoke",
        "e2e_contracts",
        "e2e_flow",
        "e2e_returns",
        "e2e_restore_guard",
        "e2e_restore_corruption",
        "e2e_crash_recovery",
        "e2e_offline",
        "perf_smoke",
        "perf_volume"
    )
}

function Invoke-Suite {
    param([string]$Name)

    $script = Join-Path $PSScriptRoot "$Name.py"
    if (-not (Test-Path -LiteralPath $script -PathType Leaf)) {
        throw "No existe el guion: $script"
    }

    $output = & powershell -NoProfile -ExecutionPolicy Bypass -File $runner `
        -Script $script -AppPath $AppPath -NativeDriver $NativeDriver `
        -KeepArtifacts -TimeoutSeconds 1800 2>&1
    $code = $LASTEXITCODE
    $text = ($output | Out-String)
    Write-Host "===== $Name (exit=$code) ====="
    Write-Host $text

    $database = ""
    $match = [regex]::Match($text, "(?m)^database=(.+)$")
    if ($match.Success) { $database = $match.Groups[1].Value.Trim() }

    $artifacts = ""
    $match = [regex]::Match($text, "(?m)^artifacts=(.+)$")
    if ($match.Success) { $artifacts = $match.Groups[1].Value.Trim() }

    if ($artifacts -and (Test-Path -LiteralPath $artifacts -PathType Container)) {
        $target = Join-Path $ReportDirectory $Name
        New-Item -ItemType Directory -Path $target -Force | Out-Null
        try { Copy-Item -Path (Join-Path $artifacts "*") -Destination $target -Recurse -Force }
        catch { Write-Host "copia_artefactos_error=$($_.Exception.Message)" }
    }

    return [pscustomobject]@{
        suite     = $Name
        exit      = $code
        database  = $database
        artifacts = $artifacts
    }
}

$results = New-Object System.Collections.Generic.List[object]
$failedCount = 0

foreach ($name in $Suite) {
    $result = Invoke-Suite -Name $name
    $results.Add($result)
    if ($result.exit -ne 0) { $failedCount++ }

    if ($name -eq "e2e_flow" -and $result.exit -eq 0 -and $result.database) {
        & $python (Join-Path $PSScriptRoot "verify_db.py") $result.database
        $results.Add([pscustomobject]@{
                suite = "verify_db"; exit = $LASTEXITCODE; database = $result.database; artifacts = ""
            })
        if ($LASTEXITCODE -ne 0) { $failedCount++ }
        & $python (Join-Path $PSScriptRoot "restore_multiphase_check.py") $result.database $AppPath
        $results.Add([pscustomobject]@{
                suite = "restore_multiphase_check"; exit = $LASTEXITCODE; database = $result.database; artifacts = ""
            })
        if ($LASTEXITCODE -ne 0) { $failedCount++ }
    }

    if ($name -eq "e2e_returns" -and $result.exit -eq 0 -and $result.database) {
        & $python (Join-Path $PSScriptRoot "verify_returns_db.py") $result.database
        $results.Add([pscustomobject]@{
                suite = "verify_returns_db"; exit = $LASTEXITCODE; database = $result.database; artifacts = ""
            })
        if ($LASTEXITCODE -ne 0) { $failedCount++ }
    }
}

$summary = [ordered]@{
    application = $AppPath
    driver      = $NativeDriver
    generated   = (Get-Date).ToString("o")
    suites      = $results.ToArray()
    failed      = $failedCount
}
$summaryPath = Join-Path $ReportDirectory "summary.json"
$json = $summary | ConvertTo-Json -Depth 5
[System.IO.File]::WriteAllText($summaryPath, $json, (New-Object System.Text.UTF8Encoding($false)))
Write-Host "summary=$summaryPath"
Write-Host "failed=$failedCount"

if ($failedCount -gt 0) { exit 1 }
exit 0
