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
    param([string]$Path)
    $process = Start-Process -FilePath $Path -ArgumentList "/S" -PassThru -Wait
    return $process.ExitCode
}

function Verify-Database {
    param([string]$Name)
    if (-not (Test-Path -LiteralPath $database -PathType Leaf)) {
        Add-Check $Name "FAIL" "no existe $database"
        return
    }
    $copy = Join-Path $evidence "$Name.db"
    Copy-Item -LiteralPath $database -Destination $copy -Force
    $output = & $installedExe --verify-backup $copy 2>&1
    $code = $LASTEXITCODE
    Add-Check $Name "PASS_$([int]($code -eq 0))" (($output | Out-String).Trim().Replace("`r", " ").Replace("`n", " "))
    if ($code -eq 0) { $checks[$Name].estado = "PASS" } else { $checks[$Name].estado = "FAIL" }
}

function Start-App {
    param([string]$Label, [int]$Seconds = 20)
    $log = Join-Path $evidence "$Label.log"
    $process = Start-Process -FilePath $installedExe -PassThru
    Start-Sleep -Seconds $Seconds
    Capture $Label
    $alive = -not $process.HasExited
    if ($alive) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
    Add-Check "arranque_$Label" $(if ($alive) { "PASS" } else { "FAIL" }) "vivo=$alive"
    $process | Select-Object Id, HasExited, StartTime | Out-String | Set-Content -LiteralPath $log
}

# --- 1. Entorno ------------------------------------------------------------
$os = Get-CimInstance Win32_OperatingSystem
$report.entorno = [ordered]@{
    edicion      = $os.Caption
    version      = $os.Version
    arquitectura = $os.OSArchitecture
    webview2     = Get-WebView2Version
}
Add-Check "webview2" $(if ($report.entorno.webview2) { "PASS" } else { "FAIL" }) "version=$($report.entorno.webview2)"

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
Add-Check "instalacion" $(if ($code -eq 0) { "PASS" } else { "FAIL" }) "codigo=$code archivo=$($candidate.Name)"
Add-Check "binario_instalado" $(if (Test-Path -LiteralPath $installedExe) { "PASS" } else { "FAIL" }) $installedExe
$uninstallEntry = Get-ItemProperty -LiteralPath $uninstallKey -ErrorAction SilentlyContinue
Add-Check "registro_desinstalacion" $(if ($uninstallEntry) { "PASS" } else { "FAIL" }) "version=$($uninstallEntry.DisplayVersion)"
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

if ($failed.Count -gt 0) { exit 1 } else { exit 0 }
