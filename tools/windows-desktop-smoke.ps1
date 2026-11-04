param(
    [string]$Executable = "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe",
    [int]$TimeoutSeconds = 20,
    [switch]$KeepArtifacts
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $PSScriptRoot
$exePath = if ([System.IO.Path]::IsPathRooted($Executable)) {
    $Executable
} else {
    Join-Path $repoRoot $Executable
}

$smokeRoot = $null
$process = $null
$oldDbPath = $env:HIQNET_POS_DB_PATH
$result = "FAIL"

function Write-Failure([string]$message) {
    Write-Output "FAIL: $message"
}

try {
    if (-not (Test-Path $exePath -PathType Leaf)) {
        throw "No existe el ejecutable: $exePath"
    }

    $smokeRoot = Join-Path $env:TEMP ("hiqnet-pos-smoke-" + [guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $smokeRoot -Force | Out-Null
    if (-not (Test-Path $smokeRoot -PathType Container)) {
        throw "No fue posible crear el directorio temporal aislado"
    }

    $dbPath = Join-Path $smokeRoot "hiqnet-pos.db"
    $realDb = Join-Path $env:LOCALAPPDATA "HiQNet\POS\data\hiqnet-pos.db"
    if ([System.IO.Path]::GetFullPath($dbPath) -eq [System.IO.Path]::GetFullPath($realDb)) {
        throw "La prueba intentó usar la base real de HiQNet POS"
    }

    $env:HIQNET_POS_DB_PATH = $dbPath

    $process = Start-Process -FilePath $exePath -PassThru
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if ($process.HasExited) { break }
        if (Test-Path $dbPath) { break }
        Start-Sleep -Milliseconds 250
    }

    $runningAfterStartup = -not $process.HasExited
    if (-not $runningAfterStartup) {
        throw "El ejecutable terminó durante el arranque con código $($process.ExitCode)"
    }
    if (-not (Test-Path $dbPath -PathType Leaf)) {
        throw "El ejecutable no creó la base SQLite aislada"
    }

    # Detener únicamente el proceso creado por esta prueba antes de validar el archivo.
    if (-not $process.HasExited) {
        Stop-Process -Id $process.Id
        $process.WaitForExit(5000) | Out-Null
    }

    $dbLength = (Get-Item $dbPath).Length
    if ($dbLength -le 0) {
        throw "La base SQLite está vacía"
    }

    $stream = [System.IO.File]::OpenRead($dbPath)
    try {
        $headerBytes = New-Object byte[] 16
        $read = $stream.Read($headerBytes, 0, 16)
    } finally {
        $stream.Dispose()
    }
    $header = [System.Text.Encoding]::ASCII.GetString($headerBytes, 0, $read)
    if ($header -ne "SQLite format 3`0") {
        throw "Cabecera SQLite inválida: $header"
    }

    $schemaVerified = "skipped"
    $sqlite = Get-Command sqlite3 -ErrorAction SilentlyContinue
    if ($sqlite) {
        $tables = & $sqlite.Source $dbPath ".tables"
        if ($LASTEXITCODE -ne 0) {
            throw "sqlite3 no pudo abrir la base"
        }
        $expected = @("organization", "product", "sale", "cash_shift", "audit_event")
        foreach ($table in $expected) {
            if ($tables -notmatch "(^|\s)$table(\s|$)") {
                throw "Falta la tabla esperada: $table"
            }
        }
        $schemaVerified = "ok"
    }

    $result = "PASS"
    Write-Output "exe=$exePath"
    Write-Output "pid=$($process.Id)"
    Write-Output "temp_dir=$smokeRoot"
    Write-Output "db_path=$dbPath"
    Write-Output "running_after_startup=$runningAfterStartup"
    Write-Output "db_header=valid"
    Write-Output "db_size_bytes=$dbLength"
    Write-Output "schema_verified=$schemaVerified"
    Write-Output "real_db_used=False"
    Write-Output "result=$result"
} catch {
    Write-Failure $_.Exception.Message
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id
    }
    Write-Output "result=FAIL"
} finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
    }
    if ($null -eq $oldDbPath) {
        Remove-Item Env:HIQNET_POS_DB_PATH -ErrorAction SilentlyContinue
    } else {
        $env:HIQNET_POS_DB_PATH = $oldDbPath
    }

    if ($smokeRoot -and (Test-Path $smokeRoot)) {
        $tempPrefix = Join-Path $env:TEMP "hiqnet-pos-smoke-"
        if (-not $smokeRoot.StartsWith($tempPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
            Write-Failure "El directorio temporal no pertenece a la prueba; no se elimina: $smokeRoot"
            $result = "FAIL"
        } elseif ($KeepArtifacts) {
            Write-Output "artifacts_kept=$smokeRoot"
        } else {
            Remove-Item -Recurse -Force $smokeRoot -ErrorAction SilentlyContinue
            Write-Output "cleanup=ok"
        }
    }
}

if ($result -ne "PASS") {
    exit 1
}
exit 0
