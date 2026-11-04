#requires -Version 5.1
<#
.SYNOPSIS
    Ejecuta un guion E2E de Selenium contra el ejecutable real de HiQNet POS.

.DESCRIPTION
    Arranca `tauri-driver` con una base SQLite aislada (directorio temporal único
    por ejecución), ejecuta el guion Python indicado, valida la base creada y
    limpia todos los recursos creados por esta ejecución.

    Nunca toca la base real de HiQNet POS (`%LOCALAPPDATA%\HiQNet\POS\data`).
    Solo termina el `tauri-driver` y los procesos `pos-desktop` que haya lanzado
    esta propia ejecución.

.PARAMETER Script
    Ruta al guion Python (por ejemplo `e2e_flow.py`).

.PARAMETER AppPath
    Ruta del ejecutable. Relativa a la raíz del repositorio si no es absoluta.

.PARAMETER KeepArtifacts
    Conserva el directorio temporal y los logs del driver para depuración.

.EXAMPLE
    powershell -NoProfile -File tools/windows-native-e2e/run-e2e.ps1 -Script tools/windows-native-e2e/e2e_flow.py
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Script,

    [string]$AppPath = "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe",

    [int]$Port = 4444,

    [string]$NativeDriver = (Join-Path $env:TEMP "edgedriver\msedgedriver.exe"),

    [int]$TimeoutSeconds = 300,

    [switch]$KeepArtifacts
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not [System.IO.Path]::IsPathRooted($AppPath)) {
    $AppPath = Join-Path $repoRoot $AppPath
}
if (-not [System.IO.Path]::IsPathRooted($Script)) {
    $Script = Join-Path $repoRoot $Script
}

if (-not (Test-Path -LiteralPath $AppPath -PathType Leaf)) {
    throw "No existe el ejecutable: $AppPath. Compílalo con 'pnpm dlx @tauri-apps/cli@2 build --debug --no-bundle'."
}
if (-not (Test-Path -LiteralPath $Script -PathType Leaf)) {
    throw "No existe el guion: $Script"
}
if (-not (Test-Path -LiteralPath $NativeDriver -PathType Leaf)) {
    throw "No existe msedgedriver: $NativeDriver. Descárgalo desde https://msedgedriver.microsoft.com/<version>/edgedriver_win64.zip"
}

# Directorio y base aislados, únicos por ejecución.
$runId = [guid]::NewGuid().ToString("N")
$runDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("hiqnet-e2e-" + $runId)
New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
$databasePath = Join-Path $runDirectory "hiqnet-pos.db"

if (-not ($runDirectory -like (Join-Path ([System.IO.Path]::GetTempPath()) "hiqnet-e2e-*"))) {
    throw "Directorio temporal inesperado: $runDirectory"
}

$driverLog = Join-Path $runDirectory "tauri-driver.log"
$driverErrorLog = Join-Path $runDirectory "tauri-driver.err.log"

$previousDatabasePath = $env:HIQNET_POS_DB_PATH
$previousArtifactDir = $env:HIQNET_E2E_ARTIFACT_DIR
$env:HIQNET_POS_DB_PATH = $databasePath
$env:HIQNET_E2E_ARTIFACT_DIR = $runDirectory

$driver = $null
$exitCode = 1
$dbState = "missing"

function Stop-RunProcesses {
    param([datetime]$Since)

    if ($driver -and -not $driver.HasExited) {
        try { Stop-Process -Id $driver.Id -Force -ErrorAction Stop } catch { }
    }
    Get-Process -Name "msedgedriver" -ErrorAction SilentlyContinue |
        Where-Object { $_.StartTime -ge $Since } |
        ForEach-Object { try { Stop-Process -Id $_.Id -Force -ErrorAction Stop } catch { } }
    Get-Process -Name "pos-desktop" -ErrorAction SilentlyContinue |
        Where-Object { $_.StartTime -ge $Since } |
        ForEach-Object { try { Stop-Process -Id $_.Id -Force -ErrorAction Stop } catch { } }
}

$startedAt = Get-Date
Write-Output "run_directory=$runDirectory"
Write-Output "database=$databasePath"
Write-Output "application=$AppPath"

try {
    $driver = Start-Process -FilePath "tauri-driver" `
        -ArgumentList @("--port", $Port, "--native-driver", $NativeDriver) `
        -RedirectStandardOutput $driverLog `
        -RedirectStandardError $driverErrorLog `
        -PassThru
    Write-Output "driver_pid=$($driver.Id)"

    $deadline = (Get-Date).AddSeconds(30)
    $ready = $false
    while (-not $ready -and (Get-Date) -lt $deadline) {
        if ($driver.HasExited) {
            throw "tauri-driver terminó con código $($driver.ExitCode). Revisa $driverErrorLog"
        }
        $client = New-Object System.Net.Sockets.TcpClient
        try {
            $client.Connect("127.0.0.1", $Port)
            $ready = $true
        }
        catch {
            Start-Sleep -Milliseconds 250
        }
        finally {
            $client.Dispose()
        }
    }
    if (-not $ready) {
        throw "tauri-driver no aceptó conexiones en el puerto $Port"
    }
    Write-Output "driver_ready=True"

    & python $Script $AppPath
    $exitCode = $LASTEXITCODE

    if (Test-Path -LiteralPath $databasePath -PathType Leaf) {
        $length = (Get-Item -LiteralPath $databasePath).Length
        $stream = [System.IO.File]::OpenRead($databasePath)
        try {
            $buffer = New-Object byte[] 16
            $read = $stream.Read($buffer, 0, 16)
            $header = [System.Text.Encoding]::ASCII.GetString($buffer, 0, $read)
        }
        finally {
            $stream.Dispose()
        }
        if ($header -eq "SQLite format 3`0") {
            $dbState = "valid ($length bytes)"
        }
        else {
            $dbState = "invalid header"
            if ($exitCode -eq 0) { $exitCode = 1 }
        }
    }
    else {
        $dbState = "missing"
        if ($exitCode -eq 0) { $exitCode = 1 }
    }
}
catch {
    Write-Output "error=$($_.Exception.Message)"
    $exitCode = 1
}
finally {
    Stop-RunProcesses -Since $startedAt
    if ($previousDatabasePath) {
        $env:HIQNET_POS_DB_PATH = $previousDatabasePath
    }
    else {
        Remove-Item Env:\HIQNET_POS_DB_PATH -ErrorAction SilentlyContinue
    }
    if ($previousArtifactDir) {
        $env:HIQNET_E2E_ARTIFACT_DIR = $previousArtifactDir
    }
    else {
        Remove-Item Env:\HIQNET_E2E_ARTIFACT_DIR -ErrorAction SilentlyContinue
    }

    Write-Output "sqlite=$dbState"

    if ($KeepArtifacts) {
        Write-Output "artifacts=$runDirectory"
    }
    else {
        Start-Sleep -Milliseconds 500
        try { Remove-Item -LiteralPath $runDirectory -Recurse -Force -ErrorAction Stop } catch { }
        Write-Output "cleanup=True"
    }
}

if ($exitCode -eq 0) {
    Write-Output "result=PASS"
}
else {
    Write-Output "result=FAIL"
}
exit $exitCode
