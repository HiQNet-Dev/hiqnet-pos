#requires -Version 5.1
<#
.SYNOPSIS
    Genera la APK ARM64 de HiQNet POS con el flujo oficial de Tauri.

.DESCRIPTION
    Delega en `cargo tauri android build`, que es la única etapa que genera los
    fuentes Kotlin nativos (TauriActivity, WryActivity, RustWebView…) y crea el
    enlace de la biblioteca Rust dentro de jniLibs. No recrea archivos a mano ni
    depende de un `gen/android` previo.

    Requiere Windows con Developer Mode activo (symlinks sin elevación), el SDK
    de Android, el NDK indicado y el CLI de Tauri 2.

.PARAMETER Profile
    debug (por defecto) o release.

.PARAMETER Clean
    Elimina únicamente las salidas de build de Gradle antes de compilar.
#>
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug",
    [switch]$Clean
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$srcTauri = Join-Path $root "apps/pos-desktop/src-tauri"
$android = Join-Path $srcTauri "gen/android"
$sdk = Join-Path $env:LOCALAPPDATA "Android/Sdk"
$ndk = Join-Path $sdk "ndk/30.0.16248370"
$toolchain = Join-Path $ndk "toolchains/llvm/prebuilt/windows-x86_64/bin"

$env:ANDROID_HOME = $sdk
$env:NDK_HOME = $ndk
$env:CC_aarch64_linux_android = Join-Path $toolchain "aarch64-linux-android21-clang.cmd"
$env:AR_aarch64_linux_android = Join-Path $toolchain "llvm-ar.exe"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $env:CC_aarch64_linux_android

# --- 1. Toolchain ----------------------------------------------------------
if (-not (Test-Path -LiteralPath $toolchain)) { throw "No se encontró el NDK esperado en $ndk" }
if (-not (Test-Path -LiteralPath (Join-Path $sdk "platforms"))) { throw "No se encontró el SDK de Android en $sdk" }

Push-Location $srcTauri
try {
    $cliVersion = (cargo tauri --version 2>&1) -join " "
    if ($LASTEXITCODE -ne 0) { throw "El CLI de Tauri no está disponible." }
    Write-Output "CLI: $cliVersion"
    Write-Output "NDK: $ndk"
} finally {
    Pop-Location
}

$rustTargets = (rustup target list --installed) -join " "
if ($rustTargets -notmatch "aarch64-linux-android") {
    throw "Falta el target Rust aarch64-linux-android (rustup target add aarch64-linux-android)."
}

# --- 2. Limpieza opcional de salidas desechables ---------------------------
if ($Clean -and (Test-Path -LiteralPath $android)) {
    foreach ($relative in @("app/build", "build", ".gradle", ".kotlin")) {
        $target = Join-Path $android $relative
        if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Recurse -Force }
    }
    Write-Output "Salidas de build eliminadas."
}

# --- 3. Inicialización (solo la primera vez) -------------------------------
# `android build` exige gen/android. Los fuentes Kotlin nativos (TauriActivity,
# WryActivity…) NO se generan aquí: los produce la etapa de build.
if (-not (Test-Path -LiteralPath $android)) {
    Write-Output "Inicializando proyecto Android (gen/android)..."
    Push-Location $srcTauri
    try {
        cargo tauri android init --ci --skip-targets-install
        if ($LASTEXITCODE -ne 0) { throw "Falló 'cargo tauri android init'." }
    } finally {
        Pop-Location
    }
}

# --- 3. Compilación con el flujo oficial -----------------------------------
$started = Get-Date
Push-Location $srcTauri
try {
    $arguments = @("tauri", "android", "build", "--target", "aarch64")
    if ($Profile -eq "debug") { $arguments += "--debug" }
    cargo @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Falló 'cargo tauri android build'. Si el error menciona enlaces simbólicos, activa Developer Mode de Windows."
    }
} finally {
    Pop-Location
}

# --- 4. Verificación del artefacto ----------------------------------------
# No se filtra por fecha: Gradle puede reutilizar salidas y un APK válido de la
# misma compilación es un artefacto legítimo. Se toma el más reciente y solo se
# advierte si es anterior al inicio de esta ejecución.
$apkRoot = Join-Path $android "app/build/outputs/apk"
$apk = Get-ChildItem -LiteralPath $apkRoot -Recurse -Filter "*.apk" -File -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if (-not $apk) { throw "No se generó ninguna APK en $apkRoot" }
if ($apk.LastWriteTime -lt $started) {
    Write-Output "aviso=la APK es anterior a esta ejecución ($($apk.LastWriteTime)); Gradle reutilizó salidas"
}

Add-Type -AssemblyName System.IO.Compression.FileSystem
$archive = [System.IO.Compression.ZipFile]::OpenRead($apk.FullName)
try {
    $native = $archive.Entries | Where-Object { $_.FullName -like "lib/*" } |
        ForEach-Object { "{0} ({1} bytes)" -f $_.FullName, $_.Length }
    if ($native -notmatch "lib/arm64-v8a/libpos_desktop_lib\.so") {
        throw "La APK no contiene la biblioteca ARM64 esperada."
    }
} finally {
    $archive.Dispose()
}

$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $apk.FullName).Hash
Write-Output "APK: $($apk.FullName)"
Write-Output "Bytes: $($apk.Length)"
Write-Output "SHA256: $hash"
Write-Output "Nativo: $($native -join '; ')"
