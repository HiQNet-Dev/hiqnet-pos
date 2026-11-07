#requires -Version 5.1
<#
.SYNOPSIS
    Prepara la carpeta de artefactos y el archivo .wsb para Windows Sandbox.

.DESCRIPTION
    Copia el instalador NSIS y el script de verificación a una carpeta fuera del
    repositorio, calcula su SHA-256 y renderiza un .wsb que monta esa carpeta en
    modo solo lectura. El repositorio, las credenciales y las bases personales no
    se exponen a la VM.

.PARAMETER Installer
    Instalador candidato. Por defecto, el último generado en target/release.

.PARAMETER PreviousInstaller
    Instalador de la versión anterior, para probar la actualización.

.PARAMETER OutputRoot
    Carpeta contenedora. Por defecto %USERPROFILE%\HiQNet-Prepilot.
#>
param(
    [string]$Installer = "",
    [string]$PreviousInstaller = "",
    [string]$SeedDatabase = "",
    [string]$OutputRoot = (Join-Path $env:USERPROFILE "HiQNet-Prepilot")
)

$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not $Installer) {
    $bundle = Join-Path $repoRoot "apps/pos-desktop/src-tauri/target/release/bundle/nsis"
    $Installer = (Get-ChildItem -LiteralPath $bundle -Filter "*-setup.exe" |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
}
if (-not $Installer -or -not (Test-Path -LiteralPath $Installer -PathType Leaf)) {
    throw "No se encontró el instalador candidato. Compílalo con: pnpm dlx @tauri-apps/cli@2.12.4 build --bundles nsis"
}

$artifacts = Join-Path $OutputRoot "artifacts"
$evidence = Join-Path $OutputRoot "evidence"
New-Item -ItemType Directory -Path $artifacts -Force | Out-Null
New-Item -ItemType Directory -Path $evidence -Force | Out-Null

# Limpieza de artefactos anteriores: nunca se copian bases ni secretos.
# Se conserva el bootstrapper de WebView2, que no es sensible.
Get-ChildItem -LiteralPath $artifacts -File |
    Where-Object { $_.Name -ne "MicrosoftEdgeWebview2Setup.exe" } |
    Remove-Item -Force

$target = Join-Path $artifacts (Split-Path -Leaf $Installer)
Copy-Item -LiteralPath $Installer -Destination $target -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot "sandbox-verify.ps1") -Destination $artifacts -Force

# Windows Sandbox no incluye WebView2. Se descarga el bootstrapper oficial en el
# anfitrion para que la VM no dependa de la red durante la instalacion.
$webviewBootstrapper = Join-Path $artifacts "MicrosoftEdgeWebview2Setup.exe"
if (-not (Test-Path -LiteralPath $webviewBootstrapper)) {
    try {
        Invoke-WebRequest -Uri "https://go.microsoft.com/fwlink/p/?LinkId=2124703" `
            -OutFile $webviewBootstrapper -UseBasicParsing -TimeoutSec 120
    }
    catch {
        Write-Output "aviso=no_se_pudo_descargar_webview2 $($_.Exception.Message)"
    }
}
if (Test-Path -LiteralPath $webviewBootstrapper) {
    Write-Output "webview2_bootstrapper=$((Get-Item -LiteralPath $webviewBootstrapper).Length) bytes"
}

$previous = $null
if ($PreviousInstaller) {
    if (-not (Test-Path -LiteralPath $PreviousInstaller -PathType Leaf)) {
        throw "No existe el instalador anterior: $PreviousInstaller"
    }
    $previousDir = Join-Path $artifacts "previous"
    New-Item -ItemType Directory -Path $previousDir -Force | Out-Null
    $previous = Join-Path $previousDir (Split-Path -Leaf $PreviousInstaller)
    Copy-Item -LiteralPath $PreviousInstaller -Destination $previous -Force
}

$candidateHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $target).Hash
$previousHash = if ($previous) { (Get-FileHash -Algorithm SHA256 -LiteralPath $previous).Hash } else { $null }

# Base sintética con datos operativos de todas las fases (sin datos personales).
# Permite comprobar que la aplicación instalada opera sin red sobre datos reales.
$seedHash = $null
if ($SeedDatabase) {
    if (-not (Test-Path -LiteralPath $SeedDatabase -PathType Leaf)) {
        throw "No existe la base semilla: $SeedDatabase"
    }
    $seed = Join-Path $artifacts "seed.db"
    Copy-Item -LiteralPath $SeedDatabase -Destination $seed -Force
    Copy-Item -LiteralPath $SeedDatabase -Destination (Join-Path $artifacts "seed-backup.db") -Force
    $seedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $seed).Hash
}
$manifest = [ordered]@{
    preparado = (Get-Date).ToString("o")
    candidato = [ordered]@{
        archivo = (Split-Path -Leaf $target)
        bytes   = (Get-Item -LiteralPath $target).Length
        sha256  = $candidateHash
    }
    anterior = if ($previous) {
        [ordered]@{
            archivo = (Split-Path -Leaf $previous)
            bytes   = (Get-Item -LiteralPath $previous).Length
            sha256  = $previousHash
        }
    } else { $null }
    semilla = if ($seedHash) { [ordered]@{ sha256 = $seedHash } } else { $null }
    host = [ordered]@{
        edicion = (Get-CimInstance Win32_OperatingSystem).Caption
        version = (Get-CimInstance Win32_OperatingSystem).Version
    }
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $artifacts "manifest.json") -Encoding UTF8

$template = Get-Content -LiteralPath (Join-Path $PSScriptRoot "prepilot-sandbox.wsb.template") -Raw
$wsb = $template.Replace("__ARTIFACTS__", $artifacts).Replace("__EVIDENCE__", $evidence)
$wsbPath = Join-Path $OutputRoot "prepilot-sandbox.wsb"
Set-Content -LiteralPath $wsbPath -Value $wsb -Encoding UTF8

Write-Output "artefactos=$artifacts"
Write-Output "evidencia=$evidence"
Write-Output "wsb=$wsbPath"
Write-Output "sha256_candidato=$candidateHash"
if ($previousHash) { Write-Output "sha256_anterior=$previousHash" }
Write-Output "Nota: ejecuta WindowsSandbox.exe `"$wsbPath`" (requiere la característica habilitada)."
