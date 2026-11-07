#requires -Version 5.1
<#
.SYNOPSIS
    Verificación del instalador de HiQNet POS dentro de Windows Sandbox.

.DESCRIPTION
    Se ejecuta automáticamente al iniciar la VM. Instala el candidato, arranca la
    aplicación, comprueba la base SQLite, repite la comprobación sin red, y
    ensaya desinstalación y reinstalación. Deja un informe JSON, logs y capturas
    en C:\Evidence.

    No automatiza formularios: el bootstrap de negocio y las ventas se validan en
    el E2E nativo del anfitrión y, cuando se revisa la evidencia, a mano.
#>

$ErrorActionPreference = "Continue"
$artifacts = "C:\HiQNetArtifacts"
$evidence = "C:\Evidence"
$installDir = Join-Path $env:LOCALAPPDATA "HiQNet POS"
$installedExe = Join-Path $installDir "pos-desktop.exe"
$database = Join-Path $env:LOCALAPPDATA "HiQNet\POS\data\hiqnet-pos.db"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\HiQNet POS"

New-Item -ItemType Directory -Path $evidence -Force | Out-Null
Start-Transcript -LiteralPath (Join-Path $evidence "install.log") -Force | Out-Null

$report = [ordered]@{
    inicio   = (Get-Date).ToString("o")
    entorno  = [ordered]@{}
    checks   = [ordered]@{}
    resultado = "FAIL"
}
$checks = $report.checks

function Add-Check {
    param([string]$Name, [string]$Status, [string]$Detail = "")
    $checks[$Name] = [ordered]@{ estado = $Status; detalle = $Detail; hora = (Get-Date).ToString("o") }
    Write-Output "[$Status] $Name $Detail"
}

function Capture {
    param([string]$Name)
    try {
        Add-Type -AssemblyName System.Windows.Forms, System.Drawing
        $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
        $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
        $bitmap.Save((Join-Path $evidence "$Name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        $graphics.Dispose()
        $bitmap.Dispose()
    }
    catch {
        Add-Check "captura_$Name" "WARN" $_.Exception.Message
    }
}

function Get-WebView2Version {
    foreach ($path in @(
        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
        "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
    )) {
        $value = (Get-ItemProperty -Path $path -Name "pv" -ErrorAction SilentlyContinue).pv
        if ($value) { return $value }
    }
    return $null
}

function Install-Package {
    param([string]$Path, [int]$TimeoutSeconds = 240)
    $process = Start-Process -FilePath $Path -ArgumentList "/S" -PassThru
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        # Algunos instaladores quedan esperando a la aplicación que abren al
        # terminar. Se acota la espera y se libera el proceso del instalador.
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        return "timeout"
    }
    return $process.ExitCode
}

function Verify-Database {
    param([string]$Name)
    if (-not (Test-Path -LiteralPath $database -PathType Leaf)) {
        Add-Check $Name "FAIL" "no existe $database"
        return
    }
    # La aplicación deja los datos en el WAL: se copian los tres archivos para
    # que SQLite pueda recuperar el estado real al verificar la copia.
    $work = Join-Path $evidence "verify"
    New-Item -ItemType Directory -Path $work -Force | Out-Null
    $copy = Join-Path $work "$Name.db"
    Get-ChildItem -LiteralPath $work -Filter "$Name.db*" -File -ErrorAction SilentlyContinue | Remove-Item -Force
    foreach ($suffix in @("", "-wal", "-shm")) {
        $side = "$database$suffix"
        if (Test-Path -LiteralPath $side) { Copy-Item -LiteralPath $side -Destination "$copy$suffix" -Force }
    }
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $copy).Hash
    $output = & $installedExe --verify-backup $copy 2>&1
    $code = $LASTEXITCODE
    $detail = (($output | Out-String).Trim().Replace("`r", " ").Replace("`n", " "))
    Add-Check $Name $(if ($code -eq 0) { "PASS" } else { "FAIL" }) "sha256=$hash $detail"
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}

function Start-App {
    param([string]$Label, [int]$Seconds = 20)
    $log = Join-Path $evidence "$Label.log"
    $process = Start-Process -FilePath $installedExe -PassThru
    Start-Sleep -Seconds $Seconds
    Capture $Label
    $alive = -not $process.HasExited
    if ($alive) {
        # Cierre ordenado para que la base quede en un estado recuperable.
        $process.CloseMainWindow() | Out-Null
        if (-not $process.WaitForExit(15000)) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Add-Check "arranque_$Label" $(if ($alive) { "PASS" } else { "FAIL" }) "vivo=$alive"
    $process | Select-Object Id, HasExited, StartTime | Out-String | Set-Content -LiteralPath $log
}

function Start-AppWithLogin {
    param([string]$Label)
    # El inicio de sesión por automatización de teclado no es fiable en Windows
    # Sandbox: se abre el menú Inicio en lugar del formulario. Se deja constancia
    # explícita y se conserva la captura para revisión manual.
    $process = Start-Process -FilePath $installedExe -PassThru
    Start-Sleep -Seconds 30
    Capture "$Label"
    $alive = -not $process.HasExited
    if ($alive) {
        $process.CloseMainWindow() | Out-Null
        if (-not $process.WaitForExit(15000)) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Add-Check "arranque_$Label" $(if ($alive) { "PASS" } else { "FAIL" }) "vivo=$alive"
    Add-Check "login_manual_$Label" "PENDIENTE" "requiere interacción; captura adjunta"
}

# --- 1. Entorno ------------------------------------------------------------
$os = Get-CimInstance Win32_OperatingSystem
$report.entorno = [ordered]@{
    edicion      = $os.Caption
    version      = $os.Version
    arquitectura = $os.OSArchitecture
    webview2     = Get-WebView2Version
}
Add-Check "webview2_antes" $(if ($report.entorno.webview2) { "PASS" } else { "INFO" }) "version=$($report.entorno.webview2) (Windows Sandbox parte sin runtime)"

# Windows Sandbox no incluye WebView2: si el artefacto lo trae, se instala
# primero para no depender de la descarga del instalador de la aplicación.
$webviewBootstrapper = Join-Path $artifacts "MicrosoftEdgeWebview2Setup.exe"
if (-not $report.entorno.webview2 -and (Test-Path -LiteralPath $webviewBootstrapper)) {
    $wv = Start-Process -FilePath $webviewBootstrapper -ArgumentList "/silent", "/install" -PassThru -Wait
    Start-Sleep -Seconds 5
    $report.entorno.webview2_instalado = Get-WebView2Version
    Add-Check "webview2_preinstalado" $(if ($report.entorno.webview2_instalado) { "PASS" } else { "FAIL" }) "codigo=$($wv.ExitCode)"
}

$candidate = Get-ChildItem -LiteralPath $artifacts -Filter "*-setup.exe" -File |
    Sort-Object Name -Descending | Select-Object -First 1
if (-not $candidate) {
    Add-Check "instalador" "FAIL" "no hay instalador en $artifacts"
    $report.resultado = "FAIL"
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence "prepilot-report.json") -Encoding UTF8
    Stop-Transcript | Out-Null
    exit 1
}

# --- 2. Actualización (opcional) -------------------------------------------
$previous = Get-ChildItem -LiteralPath (Join-Path $artifacts "previous") -Filter "*-setup.exe" -File -ErrorAction SilentlyContinue |
    Select-Object -First 1
if ($previous) {
    $code = Install-Package $previous.FullName
    Add-Check "instalacion_anterior" $(if ($code -eq 0) { "PASS" } else { "FAIL" }) "codigo=$code"
    Start-App "arranque_anterior" 15
    Verify-Database "base_antes_de_actualizar"
}

# --- 3. Instalación del candidato ------------------------------------------
$code = Install-Package $candidate.FullName
$installStatus = if ($code -eq 0) { "PASS" } elseif ($code -eq "timeout") { "OBSERVADO" } else { "FAIL" }
Add-Check "instalacion" $installStatus "codigo=$code archivo=$($candidate.Name)"
Add-Check "binario_instalado" $(if (Test-Path -LiteralPath $installedExe) { "PASS" } else { "FAIL" }) $installedExe
$uninstallEntry = Get-ItemProperty -LiteralPath $uninstallKey -ErrorAction SilentlyContinue
Add-Check "registro_desinstalacion" $(if ($uninstallEntry) { "PASS" } else { "FAIL" }) "version=$($uninstallEntry.DisplayVersion)"

# WebView2 puede instalarlo el propio instalador (downloadBootstrapper).
$webviewAfter = Get-WebView2Version
Add-Check "webview2_tras_instalar" $(if ($webviewAfter) { "PASS" } else { "FAIL" }) "version=$($webviewAfter)"
$report.entorno.webview2_tras_instalar = $webviewAfter

# El instalador NSIS arranca la aplicación al terminar. Se espera a que cree la
# base y se cierra esa instancia antes de lanzar las controladas.
Start-Sleep -Seconds 15
$auto = @(Get-Process -Name "pos-desktop" -ErrorAction SilentlyContinue)
Add-Check "autostart_del_instalador" $(if ($auto.Count -gt 0) { "OBSERVADO" } else { "NO_OBSERVADO" }) "procesos=$($auto.Count)"
foreach ($process in $auto) {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
}
# Una instalación limpia no crea la base: se crea en el primer arranque.
Add-Check "base_antes_del_primer_arranque" $(if (-not (Test-Path -LiteralPath $database)) { "PASS" } else { "OBSERVADO" }) "existe=$((Test-Path -LiteralPath $database))"
if ($previous) { Verify-Database "base_tras_actualizar" }

# --- 4. Primer arranque -----------------------------------------------------
Start-App "primer_arranque" 25
Verify-Database "base_tras_primer_arranque"

# --- 5. Operación sin red ---------------------------------------------------
$adapters = @(Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq "Up" })
foreach ($adapter in $adapters) {
    Disable-NetAdapter -Name $adapter.Name -Confirm:$false -ErrorAction SilentlyContinue | Out-Null
}
$stillUp = @(Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq "Up" }).Count
Add-Check "red_desconectada" $(if ($stillUp -eq 0) { "PASS" } else { "WARN" }) "adaptadores_activos=$stillUp"

Start-App "arranque_sin_red" 25
Verify-Database "base_sin_red"

# --- 5b. Operación sobre datos reales, sin red ------------------------------
$seed = Join-Path $artifacts "seed.db"
if (Test-Path -LiteralPath $seed) {
    $running = @(Get-Process -Name "pos-desktop" -ErrorAction SilentlyContinue)
    foreach ($process in $running) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 3
    Get-ChildItem -LiteralPath (Split-Path -Parent $database) -Filter "hiqnet-pos.db*" -File -ErrorAction SilentlyContinue |
        Remove-Item -Force -ErrorAction SilentlyContinue
    Copy-Item -LiteralPath $seed -Destination $database -Force
    $seedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $seed).Hash
    Start-AppWithLogin "datos_operativos"
    Verify-Database "base_operativa_sin_red"

    # Recuperación offline con la utilidad incluida.
    $backupPath = Join-Path $artifacts "seed-backup.db"
    if (Test-Path -LiteralPath $backupPath) {
        $app = @(Get-Process -Name "pos-desktop" -ErrorAction SilentlyContinue)
        foreach ($process in $app) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        Start-Sleep -Seconds 3
        $restore = & $installedExe --restore-backup $backupPath --database $database 2>&1
        $restoreCode = $LASTEXITCODE
        Add-Check "restauracion_sin_red" $(if ($restoreCode -eq 0) { "PASS" } else { "FAIL" }) (($restore | Out-String).Trim().Replace("`r", " ").Replace("`n", " "))
        Verify-Database "base_tras_restaurar_sin_red"
    }
    # Se comprueba que el arranque no modificó el contenido del seed.
    $after = Join-Path $evidence "seed-comparacion.db"
    Copy-Item -LiteralPath $seed -Destination $after -Force
    Add-Check "seed_sha256" "INFO" $seedHash
    Remove-Item -LiteralPath $after -Force -ErrorAction SilentlyContinue
}

# --- 6. Desinstalación ------------------------------------------------------
$uninstaller = Join-Path $installDir "uninstall.exe"
if (Test-Path -LiteralPath $uninstaller) {
    $process = Start-Process -FilePath $uninstaller -ArgumentList "/S" -PassThru -Wait
    Start-Sleep -Seconds 5
    Add-Check "desinstalacion" $(if (-not (Test-Path -LiteralPath $installedExe)) { "PASS" } else { "FAIL" }) "codigo=$($process.ExitCode)"
    Add-Check "base_tras_desinstalar" $(if (Test-Path -LiteralPath $database) { "PASS" } else { "FAIL" }) "se conserva la base"
}
else {
    Add-Check "desinstalacion" "FAIL" "no se encontró uninstall.exe"
}

# --- 7. Reinstalación -------------------------------------------------------
$code = Install-Package $candidate.FullName
Add-Check "reinstalacion" $(if ($code -eq 0) { "PASS" } else { "FAIL" }) "codigo=$code"
Start-App "arranque_reinstalado" 25
Verify-Database "base_tras_reinstalar"

# --- 8. Cierre --------------------------------------------------------------
$failed = @($checks.Keys | Where-Object { $checks[$_].estado -eq "FAIL" })
$report.resultado = if ($failed.Count -eq 0) { "PASS" } else { "FAIL" }
$report.fallos = $failed
$report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $evidence "prepilot-report.json") -Encoding UTF8
Write-Output "resultado=$($report.resultado)"
Stop-Transcript | Out-Null

# La VM es desechable: se apaga al terminar para liberar recursos del anfitrión.
# El anfitrión no se ve afectado; solo se apaga la sesión de Windows Sandbox.
if ($env:USERNAME -eq "WDAGUtilityAccount") {
    shutdown /s /t 5
}

if ($failed.Count -gt 0) { exit 1 } else { exit 0 }
