#requires -Version 5.1
<#
.SYNOPSIS
    Empaqueta la APK ARM64 de HiQNet POS sin elevación ni enlaces simbólicos.

.DESCRIPTION
    El CLI de Tauri crea un symlink de la biblioteca Rust dentro de jniLibs, lo
    que falla en Windows sin privilegios. Este script compila con Cargo, copia
    la biblioteca real y ejecuta Gradle excluyendo solo la tarea que recrea el
    enlace. Requiere el NDK y el SDK ya instalados.

.PARAMETER Profile
    debug (por defecto) o release.

.PARAMETER Keystore
    Ruta opcional al keystore para firmar la APK release. Las contraseñas se
    leen de variables de entorno; nunca se escriben en el repositorio.
#>
param(
    [ValidateSet("debug", "release")]
    [string]$Profile = "debug"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$srcTauri = Join-Path $root "apps/pos-desktop/src-tauri"
$android = Join-Path $srcTauri "gen/android"
$ndk = Join-Path $env:LOCALAPPDATA "Android/Sdk/ndk/30.0.16248370/toolchains/llvm/prebuilt/windows-x86_64/bin"

$env:ANDROID_HOME = Join-Path $env:LOCALAPPDATA "Android/Sdk"
$env:CC_aarch64_linux_android = Join-Path $ndk "aarch64-linux-android21-clang.cmd"
$env:AR_aarch64_linux_android = Join-Path $ndk "llvm-ar.exe"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $env:CC_aarch64_linux_android

Push-Location (Join-Path $root "frontend")
try {
    pnpm run build
    if ($LASTEXITCODE -ne 0) { throw "Falló la compilación del frontend." }
} finally {
    Pop-Location
}

Push-Location $srcTauri
try {
    $cargoArgs = @("build", "--features", "tauri/custom-protocol", "--target", "aarch64-linux-android")
    if ($Profile -eq "release") { $cargoArgs += "--release" }
    cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "Falló la compilación Rust para ARM64." }
} finally {
    Pop-Location
}

$built = Join-Path $srcTauri "target/aarch64-linux-android/$Profile/libpos_desktop_lib.so"
$destination = Join-Path $android "app/src/main/jniLibs/arm64-v8a/libpos_desktop_lib.so"
if (Test-Path -LiteralPath $destination) { Remove-Item -LiteralPath $destination -Force }
Copy-Item -LiteralPath $built -Destination $destination

Push-Location $android
try {
    ./gradlew.bat :app:clean --quiet
    if ($Profile -eq "release") {
        ./gradlew.bat :app:assembleArm64Release -x :app:rustBuildArm64Release --quiet
        $apk = "app/build/outputs/apk/arm64/release/app-arm64-release-unsigned.apk"
    } else {
        ./gradlew.bat :app:assembleArm64Debug -x :app:rustBuildArm64Debug --quiet
        $apk = "app/build/outputs/apk/arm64/debug/app-arm64-debug.apk"
    }
    if ($LASTEXITCODE -ne 0) { throw "Falló el empaquetado de Gradle." }
} finally {
    Pop-Location
}

$apkPath = Join-Path $android $apk
$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $apkPath).Hash
$size = (Get-Item -LiteralPath $apkPath).Length
Write-Output "APK: $apkPath"
Write-Output "Bytes: $size"
Write-Output "SHA256: $hash"

if ($Profile -eq "release" -and $Keystore) {
    Write-Output "Firma pendiente: use apksigner con el keystore fuera del repositorio."
}
