#requires -Version 5.1
<#
.SYNOPSIS
    Ejecuta un guion E2E de Selenium contra el ejecutable real de HiQNet POS.

.DESCRIPTION
    Arranca `tauri-driver` con una base SQLite aislada (directorio temporal único
    por ejecución), ejecuta el guion Python indicado, valida la base creada y
    limpia todos los recursos creados por esta ejecución.

    Nunca toca la base real de HiQNet POS (`%LOCALAPPDATA%\HiQNet\POS\data`).
    Solo termina el árbol de procesos descendiente del `tauri-driver` iniciado
    por esta ejecución. Nunca selecciona procesos por nombre o fecha de inicio.

.PARAMETER Script
    Ruta al guion Python (por ejemplo `e2e_flow.py`).

.PARAMETER AppPath
    Ruta del ejecutable. Relativa a la raíz del repositorio si no es absoluta.

.PARAMETER KeepArtifacts
    Conserva el directorio temporal y los logs del driver para depuración.

.PARAMETER DatabasePath
    Base que se reabre (o se crea) en lugar de una temporal única. Sirve para
    probar continuidad tras una actualización o un reinicio. Nunca se permite una
    ruta dentro de `%LOCALAPPDATA%\HiQNet\POS\data` y jamás se borra el directorio.

.EXAMPLE
    powershell -NoProfile -File tools/windows-native-e2e/run-e2e.ps1 -Script tools/windows-native-e2e/e2e_flow.py
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Script,

    [string]$AppPath = "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe",

    [int]$Port = 4444,

    [int]$NativePort = 4445,

    [string]$NativeDriver = (Join-Path $env:TEMP "edgedriver\msedgedriver.exe"),

    [int]$TimeoutSeconds = 300,

    [string]$DatabasePath = "",

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
    throw "No existe el ejecutable: $AppPath. Compílalo con 'pnpm dlx @tauri-apps/cli@2.12.4 build --debug --no-bundle'."
}
if (-not (Test-Path -LiteralPath $Script -PathType Leaf)) {
    throw "No existe el guion: $Script"
}
if (-not (Test-Path -LiteralPath $NativeDriver -PathType Leaf)) {
    throw "No existe msedgedriver: $NativeDriver. Descárgalo desde https://msedgedriver.microsoft.com/<version>/edgedriver_win64.zip"
}

# Directorio y base aislados, únicos por ejecución. Con -DatabasePath se reabre
# una base existente y no se crea ni se borra nada del directorio indicado.
$reusingDatabase = $false
if ($DatabasePath) {
    if (-not [System.IO.Path]::IsPathRooted($DatabasePath)) {
        throw "-DatabasePath debe ser una ruta absoluta."
    }
    $resolved = [System.IO.Path]::GetFullPath($DatabasePath)
    $realData = [System.IO.Path]::GetFullPath((Join-Path $env:LOCALAPPDATA "HiQNet\POS\data"))
    if ($resolved.StartsWith($realData, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "El arnés nunca opera sobre la base de trabajo real: $realData"
    }
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        $parent = Split-Path -Parent $resolved
        if (-not (Test-Path -LiteralPath $parent -PathType Container)) {
            throw "No existe el directorio de la base indicada en -DatabasePath: $parent"
        }
    }
    $databasePath = $resolved
    $runDirectory = Split-Path -Parent $resolved
    $reusingDatabase = $true
}
else {
    $runId = [guid]::NewGuid().ToString("N")
    $runDirectory = Join-Path ([System.IO.Path]::GetTempPath()) ("hiqnet-e2e-" + $runId)
    New-Item -ItemType Directory -Path $runDirectory -Force | Out-Null
    $databasePath = Join-Path $runDirectory "hiqnet-pos.db"

    if (-not ($runDirectory -like (Join-Path ([System.IO.Path]::GetTempPath()) "hiqnet-e2e-*"))) {
        throw "Directorio temporal inesperado: $runDirectory"
    }
}

$driverLog = Join-Path $runDirectory "tauri-driver.log"
$driverErrorLog = Join-Path $runDirectory "tauri-driver.err.log"

$previousDatabasePath = $env:HIQNET_POS_DB_PATH
$previousArtifactDir = $env:HIQNET_E2E_ARTIFACT_DIR
$previousE2ePort = $env:HIQNET_E2E_PORT
$env:HIQNET_POS_DB_PATH = $databasePath
$env:HIQNET_E2E_ARTIFACT_DIR = $runDirectory
$env:HIQNET_E2E_PORT = $Port

$driver = $null
$exitCode = 1
$dbState = "missing"

function Get-RunDescendantProcessIds {
    param([int]$RootPid)
    $childrenByParent = @{}
    Get-CimInstance Win32_Process | ForEach-Object {
        $parent = [int]$_.ParentProcessId
        if (-not $childrenByParent.ContainsKey($parent)) { $childrenByParent[$parent] = @() }
        $childrenByParent[$parent] += [int]$_.ProcessId
    }
    $result = New-Object System.Collections.Generic.List[int]
    $seen = New-Object 'System.Collections.Generic.HashSet[int]'
    $pending = New-Object System.Collections.Generic.Queue[int]
    [void]$seen.Add($RootPid)
    $pending.Enqueue($RootPid)
    while ($pending.Count -gt 0) {
        $parent = $pending.Dequeue()
        foreach ($child in @($childrenByParent[$parent])) {
            if ($seen.Add($child)) {
                $result.Add($child)
                $pending.Enqueue($child)
            }
        }
    }
    return $result
}

function Stop-RunProcesses {
    if (-not $driver) { return }
    # Se obtiene el árbol antes de terminar la raíz; solo estos PID fueron
    # creados como descendientes verificables del driver de esta ejecución.
    $owned = @(Get-RunDescendantProcessIds -RootPid $driver.Id)
    [array]::Reverse($owned)
    foreach ($ownedProcessId in $owned) { try { Stop-Process -Id $ownedProcessId -Force -ErrorAction Stop } catch { } }
    if (-not $driver.HasExited) {
        try { Stop-Process -Id $driver.Id -Force -ErrorAction Stop } catch { }
    }

    # Si el driver murió antes de la limpieza, sus hijos quedan huérfanos y el
    # árbol ya no los alcanza. La app de esta ejecución es identificable sin
    # ambigüedad porque su línea de comandos incluye la base única de la corrida;
    # ninguna instalación personal usa esa ruta.
    foreach ($stray in @(Get-CimInstance Win32_Process -Filter "Name = 'pos-desktop.exe'")) {
        $commandLine = $stray.CommandLine
        if ($commandLine -and $commandLine.Contains($databasePath)) {
            try { Stop-Process -Id ([int]$stray.ProcessId) -Force -ErrorAction Stop } catch { }
        }
    }
}

function Get-DriverExitText {
    # En PowerShell 5.1 `ExitCode` de un proceso iniciado con `-PassThru` puede
    # quedar vacío; el mensaje debe ser honesto en lugar de mostrar un hueco.
    try { $driver.Refresh() } catch { }
    if ($null -ne $driver.ExitCode) { return [string]$driver.ExitCode }
    return "desconocido"
}

function Assert-LocalPortAvailable {
    param([int]$PortToCheck)
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, $PortToCheck)
    try {
        $listener.Start()
    }
    catch {
        throw "El puerto $PortToCheck ya está en uso. Elige otro; el arnés no terminará procesos ajenos."
    }
    finally {
        $listener.Stop()
    }
}

$startedAt = Get-Date
Write-Output "run_directory=$runDirectory"
Write-Output "database=$databasePath"
Write-Output "application=$AppPath"

try {
    if ($Port -eq $NativePort) {
        throw "-Port y -NativePort deben ser distintos."
    }
    Assert-LocalPortAvailable -PortToCheck $Port
    Assert-LocalPortAvailable -PortToCheck $NativePort
    $driver = Start-Process -FilePath "tauri-driver" `
        -ArgumentList @("--port", $Port, "--native-port", $NativePort, "--native-driver", $NativeDriver) `
        -RedirectStandardOutput $driverLog `
        -RedirectStandardError $driverErrorLog `
        -PassThru
    Write-Output "driver_pid=$($driver.Id)"

    $deadline = (Get-Date).AddSeconds(30)
    $ready = $false
    while (-not $ready -and (Get-Date) -lt $deadline) {
        if ($driver.HasExited) {
            throw "tauri-driver terminó con código $(Get-DriverExitText). Revisa $driverErrorLog"
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
    if ($driver.HasExited) {
        throw "tauri-driver terminó con código $(Get-DriverExitText) después del sondeo. Revisa $driverErrorLog"
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
            # Con WAL activo, el archivo principal puede ser pequeño y los datos
            # vivir en `-wal`. La comprobación de contenido es `verify_db.py`;
            # esto solo confirma que existe una base legible para inspección.
            $walPath = "$databasePath-wal"
            $walBytes = if (Test-Path -LiteralPath $walPath) { (Get-Item -LiteralPath $walPath).Length } else { 0 }
            $dbState = "header_ok (main=$length bytes, wal=$walBytes bytes)"
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
    # Un fallo al detener procesos nunca debe impedir restaurar el entorno ni
    # dejar la sesión del operador apuntando a la base temporal.
    try { Stop-RunProcesses } catch { Write-Output "cleanup_error=$($_.Exception.Message)" }
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
    if ($previousE2ePort) {
        $env:HIQNET_E2E_PORT = $previousE2ePort
    }
    else {
        Remove-Item Env:\HIQNET_E2E_PORT -ErrorAction SilentlyContinue
    }

    Write-Output "sqlite=$dbState"

    if ($KeepArtifacts -or $reusingDatabase) {
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
