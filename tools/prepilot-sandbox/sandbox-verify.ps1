#requires -Version 5.1
<#
.SYNOPSIS
    Verificación del instalador de HiQNet POS dentro de Windows Sandbox.

.DESCRIPTION
    Se ejecuta automáticamente al iniciar la VM. Instala el candidato, arranca la
    aplicación, comprueba la base SQLite, repite la comprobación sin red, y
    ensaya desinstalación y reinstalación. Deja un informe JSON, logs y capturas
    en C:\Evidence.

    El informe se reescribe después de **cada** comprobación: si una etapa falla,
    si el instalador no deja el binario o si la VM se cierra antes de terminar,
    el JSON sigue existiendo con el estado real alcanzado. Una etapa con excepción
    se registra como ERROR y el guion continúa con las siguientes.

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
$reportPath = Join-Path $evidence "prepilot-report.json"
$progressPath = Join-Path $evidence "progress.log"

New-Item -ItemType Directory -Path $evidence -Force | Out-Null

$script:report = [ordered]@{
    inicio    = (Get-Date).ToString("o")
    entorno   = [ordered]@{}
    paso      = "inicio"
    checks    = [ordered]@{}
    resultado = "FAIL"
}
$script:checks = $script:report.checks

# Escribe el informe de forma atómica y sin BOM (el BOM rompe lectores JSON
# estrictos). Un fallo al escribir nunca debe ocultar el estado ya registrado,
# por eso el temporal se escribe primero.
function Save-Report {
    try {
        $json = $script:report | ConvertTo-Json -Depth 10
        $temporary = "$reportPath.tmp"
        [System.IO.File]::WriteAllText($temporary, $json, (New-Object System.Text.UTF8Encoding($false)))
        Move-Item -LiteralPath $temporary -Destination $reportPath -Force
    }
    catch {
        Write-Output "[WARN] no se pudo escribir el informe: $($_.Exception.Message)"
    }
}

function Set-Step {
    param([string]$Name)
    $script:report.paso = $Name
    try {
        Add-Content -LiteralPath $progressPath -Value "$(Get-Date -Format o) $Name" -Encoding UTF8
    }
    catch { }
    Save-Report
}

function Add-Check {
    param([string]$Name, [string]$Status, [string]$Detail = "")
    $script:checks[$Name] = [ordered]@{
        estado = $Status
        detalle = $Detail
        hora = (Get-Date).ToString("o")
    }
    Write-Output "[$Status] $Name $Detail"
    Save-Report
}

# Ejecuta una etapa aislada: una excepción queda registrada y el guion sigue.
function Invoke-Step {
    param([string]$Name, [scriptblock]$Action)
    Set-Step $Name
    try {
        & $Action
    }
    catch {
        Add-Check "${Name}_error" "ERROR" $_.Exception.Message
    }
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

# El instalador NSIS en modo silencioso devuelve el control cuando termina, pero
# el desinstalador se relanza desde una copia temporal y puede seguir trabajando:
# hay que esperar a que el binario esté realmente en su sitio antes de usarlo.
function Wait-ForInstalledBinary {
    param([int]$TimeoutSeconds = 60)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        if (Test-Path -LiteralPath $installedExe -PathType Leaf) { return $true }
        Start-Sleep -Milliseconds 500
    }
    return (Test-Path -LiteralPath $installedExe -PathType Leaf)
}

# El desinstalador de NSIS copia `Un_A.exe` a %TEMP% y termina antes que esa
# copia. Si se reinstala de inmediato, la copia tardía puede borrar el binario
# recién instalado; esa era la causa de que la etapa de reinstalación quedara sin
# evidencia.
function Wait-ForUninstallToSettle {
    param([int]$TimeoutSeconds = 60)
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while ((Get-Date) -lt $deadline) {
        $pending = @(Get-Process -ErrorAction SilentlyContinue |
            Where-Object { $_.ProcessName -in @("Un_A", "Au_", "uninstall", "uninst") })
        if ($pending.Count -eq 0 -and -not (Test-Path -LiteralPath $installedExe)) { return $true }
        Start-Sleep -Milliseconds 500
    }
    return $false
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
    if (-not (Test-Path -LiteralPath $installedExe -PathType Leaf)) {
        Add-Check $Name "FAIL" "no existe el binario para verificar: $installedExe"
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

function Stop-AppInstances {
    foreach ($process in @(Get-Process -Name "pos-desktop" -ErrorAction SilentlyContinue)) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
}

function Start-App {
    param([string]$Label, [int]$Seconds = 20)
    $log = Join-Path $evidence "$Label.log"
    if (-not (Test-Path -LiteralPath $installedExe -PathType Leaf)) {
        Add-Check "arranque_$Label" "FAIL" "no existe $installedExe"
        return
    }
    try {
        $process = Start-Process -FilePath $installedExe -PassThru
    }
    catch {
        Add-Check "arranque_$Label" "FAIL" "no se pudo iniciar: $($_.Exception.Message)"
        return
    }
    Start-Sleep -Seconds $Seconds
    Capture $Label
    $alive = $false
    try { $alive = -not $process.HasExited } catch { $alive = $false }
    if ($alive) {
        # Cierre ordenado para que la base quede en un estado recuperable.
        try { $process.CloseMainWindow() | Out-Null } catch { }
        if (-not $process.WaitForExit(15000)) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Add-Check "arranque_$Label" $(if ($alive) { "PASS" } else { "FAIL" }) "vivo=$alive"
    try {
        $process | Select-Object Id, HasExited, StartTime | Out-String | Set-Content -LiteralPath $log
    }
    catch { }
}

function Start-AppWithLogin {
    param([string]$Label)
    # El inicio de sesión por automatización de teclado no es fiable en Windows
    # Sandbox: se abre el menú Inicio en lugar del formulario. Se deja constancia
    # explícita y se conserva la captura para revisión manual.
    if (-not (Test-Path -LiteralPath $installedExe -PathType Leaf)) {
        Add-Check "arranque_$Label" "FAIL" "no existe $installedExe"
        return
    }
    try {
        $process = Start-Process -FilePath $installedExe -PassThru
    }
    catch {
        Add-Check "arranque_$Label" "FAIL" "no se pudo iniciar: $($_.Exception.Message)"
        return
    }
    Start-Sleep -Seconds 30
    Capture "$Label"
    $alive = $false
    try { $alive = -not $process.HasExited } catch { $alive = $false }
    if ($alive) {
        try { $process.CloseMainWindow() | Out-Null } catch { }
        if (-not $process.WaitForExit(15000)) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Add-Check "arranque_$Label" $(if ($alive) { "PASS" } else { "FAIL" }) "vivo=$alive"
    Add-Check "login_manual_$Label" "PENDIENTE" "requiere interacción; captura adjunta"
}

function Set-NetworkState {
    param([bool]$Enabled)
    $adapters = @(Get-NetAdapter -ErrorAction SilentlyContinue)
    foreach ($adapter in $adapters) {
        if ($Enabled) {
            Enable-NetAdapter -Name $adapter.Name -Confirm:$false -ErrorAction SilentlyContinue | Out-Null
        }
        elseif ($adapter.Status -eq "Up") {
            Disable-NetAdapter -Name $adapter.Name -Confirm:$false -ErrorAction SilentlyContinue | Out-Null
        }
    }
    $up = @(Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object { $_.Status -eq "Up" }).Count
    return $up
}

$transcript = $false
try {
    try {
        Start-Transcript -LiteralPath (Join-Path $evidence "install.log") -Force | Out-Null
        $transcript = $true
    }
    catch { }

    Save-Report

    # --- 1. Entorno --------------------------------------------------------
    Set-Step "entorno"
    Invoke-Step "entorno" {
        $os = Get-CimInstance Win32_OperatingSystem
        $script:report.entorno = [ordered]@{
            edicion      = $os.Caption
            version      = $os.Version
            arquitectura = $os.OSArchitecture
            webview2     = Get-WebView2Version
        }
        Add-Check "webview2_antes" $(if ($script:report.entorno.webview2) { "PASS" } else { "INFO" }) "version=$($script:report.entorno.webview2) (Windows Sandbox parte sin runtime)"
    }

    # Windows Sandbox no incluye WebView2: si el artefacto lo trae, se instala
    # primero para no depender de la descarga durante la instalación.
    Invoke-Step "webview2" {
        $webviewBootstrapper = Join-Path $artifacts "MicrosoftEdgeWebview2Setup.exe"
        if (-not $script:report.entorno.webview2 -and (Test-Path -LiteralPath $webviewBootstrapper)) {
            $wv = Start-Process -FilePath $webviewBootstrapper -ArgumentList "/silent", "/install" -PassThru -Wait
            Start-Sleep -Seconds 5
            $script:report.entorno.webview2_instalado = Get-WebView2Version
            Add-Check "webview2_preinstalado" $(if ($script:report.entorno.webview2_instalado) { "PASS" } else { "FAIL" }) "codigo=$($wv.ExitCode)"
        }
    }

    Set-Step "instalador"
    $candidate = Get-ChildItem -LiteralPath $artifacts -Filter "*-setup.exe" -File -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -First 1
    if (-not $candidate) {
        Add-Check "instalador" "FAIL" "no hay instalador en $artifacts"
        throw "sin instalador"
    }
    Add-Check "instalador" "PASS" "archivo=$($candidate.Name) bytes=$($candidate.Length)"

    # --- 2. Actualización (opcional) ---------------------------------------
    $previous = Get-ChildItem -LiteralPath (Join-Path $artifacts "previous") -Filter "*-setup.exe" -File -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($previous) {
        Invoke-Step "instalacion_anterior" {
            $code = Install-Package $previous.FullName
            Add-Check "instalacion_anterior" $(if ($code -eq 0) { "PASS" } else { "FAIL" }) "codigo=$code archivo=$($previous.Name)"
            [void](Wait-ForInstalledBinary 30)
            Start-App "arranque_anterior" 20
            Verify-Database "base_antes_de_actualizar"
            Stop-AppInstances
        }
    }

    # --- 3. Instalación del candidato --------------------------------------
    Invoke-Step "instalacion" {
        $code = Install-Package $candidate.FullName
        $installStatus = if ($code -eq 0) { "PASS" } elseif ($code -eq "timeout") { "OBSERVADO" } else { "FAIL" }
        Add-Check "instalacion" $installStatus "codigo=$code archivo=$($candidate.Name)"
        $present = Wait-ForInstalledBinary 60
        Add-Check "binario_instalado" $(if ($present) { "PASS" } else { "FAIL" }) $installedExe
        $uninstallEntry = Get-ItemProperty -LiteralPath $uninstallKey -ErrorAction SilentlyContinue
        Add-Check "registro_desinstalacion" $(if ($uninstallEntry) { "PASS" } else { "FAIL" }) "version=$($uninstallEntry.DisplayVersion)"

        # WebView2 puede instalarlo el propio instalador (downloadBootstrapper).
        $webviewAfter = Get-WebView2Version
        $script:report.entorno.webview2_tras_instalar = $webviewAfter
        Add-Check "webview2_tras_instalar" $(if ($webviewAfter) { "PASS" } else { "FAIL" }) "version=$webviewAfter"

        # El instalador NSIS arranca la aplicación al terminar. Se espera a que
        # cree la base y se cierra esa instancia antes de lanzar las controladas.
        Start-Sleep -Seconds 15
        $auto = @(Get-Process -Name "pos-desktop" -ErrorAction SilentlyContinue)
        Add-Check "autostart_del_instalador" $(if ($auto.Count -gt 0) { "OBSERVADO" } else { "NO_OBSERVADO" }) "procesos=$($auto.Count)"
        Stop-AppInstances
        # Una instalación limpia no crea la base: se crea en el primer arranque.
        Add-Check "base_antes_del_primer_arranque" $(if (-not (Test-Path -LiteralPath $database)) { "PASS" } else { "OBSERVADO" }) "existe=$((Test-Path -LiteralPath $database))"
        if ($previous) { Verify-Database "base_tras_actualizar" }
    }

    # --- 4. Primer arranque ------------------------------------------------
    Invoke-Step "primer_arranque" {
        Start-App "primer_arranque" 25
        Verify-Database "base_tras_primer_arranque"
    }

    # --- 5. Operación sin red ----------------------------------------------
    Invoke-Step "sin_red" {
        $stillUp = Set-NetworkState -Enabled $false
        Add-Check "red_desconectada" $(if ($stillUp -eq 0) { "PASS" } else { "WARN" }) "adaptadores_activos=$stillUp"
        Start-App "arranque_sin_red" 25
        Verify-Database "base_sin_red"
    }

    # --- 5b. Operación sobre datos reales, sin red -------------------------
    Invoke-Step "datos_operativos" {
        $seed = Join-Path $artifacts "seed.db"
        if (-not (Test-Path -LiteralPath $seed)) {
            Add-Check "datos_operativos" "NO EJECUTADO" "no se preparó una base semilla"
            return
        }
        Stop-AppInstances
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
            Stop-AppInstances
            Start-Sleep -Seconds 3
            $restore = & $installedExe --restore-backup $backupPath --database $database 2>&1
            $restoreCode = $LASTEXITCODE
            Add-Check "restauracion_sin_red" $(if ($restoreCode -eq 0) { "PASS" } else { "FAIL" }) (($restore | Out-String).Trim().Replace("`r", " ").Replace("`n", " "))
            Verify-Database "base_tras_restaurar_sin_red"
        }
        Add-Check "seed_sha256" "INFO" $seedHash
    }

    # --- 6. Desinstalación -------------------------------------------------
    Invoke-Step "desinstalacion" {
        Stop-AppInstances
        $uninstaller = Join-Path $installDir "uninstall.exe"
        if (-not (Test-Path -LiteralPath $uninstaller)) {
            Add-Check "desinstalacion" "FAIL" "no se encontró uninstall.exe"
            return
        }
        $process = Start-Process -FilePath $uninstaller -ArgumentList "/S" -PassThru
        [void]$process.WaitForExit(120000)
        # NSIS delega en una copia temporal que termina después: sin esta espera
        # la reinstalación posterior competía con el borrado del binario.
        $settled = Wait-ForUninstallToSettle 60
        $binaryGone = -not (Test-Path -LiteralPath $installedExe)
        Add-Check "desinstalacion" $(if ($binaryGone) { "PASS" } else { "FAIL" }) "codigo=$($process.ExitCode) asentado=$settled"
        Add-Check "base_tras_desinstalar" $(if (Test-Path -LiteralPath $database) { "PASS" } else { "FAIL" }) "se conserva la base"
    }

    # --- 7. Reinstalación --------------------------------------------------
    Invoke-Step "reinstalacion" {
        $code = Install-Package $candidate.FullName
        Add-Check "reinstalacion" $(if ($code -eq 0) { "PASS" } else { "FAIL" }) "codigo=$code"
        $present = Wait-ForInstalledBinary 60
        Add-Check "binario_tras_reinstalar" $(if ($present) { "PASS" } else { "FAIL" }) $installedExe
        Start-App "arranque_reinstalado" 25
        Verify-Database "base_tras_reinstalar"
    }

    # --- 8. Veredicto ------------------------------------------------------
    Set-Step "veredicto"
    $failed = @($script:checks.Keys | Where-Object { $script:checks[$_].estado -in @("FAIL", "ERROR") })
    $script:report.resultado = if ($failed.Count -eq 0) { "PASS" } else { "FAIL" }
    $script:report.fallos = $failed
    Save-Report
    Write-Output "resultado=$($script:report.resultado)"
}
catch {
    Add-Check "guion" "ERROR" $_.Exception.Message
    $script:report.resultado = "FAIL"
    Save-Report
}
finally {
    # Restaurar la red antes de cerrar permite revisar la VM si algo falla.
    try { [void](Set-NetworkState -Enabled $true) } catch { }
    Save-Report
    if ($transcript) {
        try { Stop-Transcript | Out-Null } catch { }
    }
}

$failedFinal = @($script:checks.Keys | Where-Object { $script:checks[$_].estado -in @("FAIL", "ERROR") })

# La VM es desechable: se apaga al terminar para liberar recursos del anfitrión.
# El informe ya está escrito antes de este punto.
if ($env:USERNAME -eq "WDAGUtilityAccount") {
    shutdown /s /t 10
}

if ($failedFinal.Count -gt 0) { exit 1 } else { exit 0 }
