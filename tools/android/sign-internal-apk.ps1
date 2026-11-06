<#
.SYNOPSIS
    Firma un APK de pruebas internas con la clave HiQNet y verifica el certificado.

.DESCRIPTION
    Solicita la contraseña del keystore de forma segura (SecureString), la usa
    únicamente en variables de entorno del proceso y no la escribe en disco ni en
    la salida. Después alinea, firma con esquemas v2/v3 y compara el SHA-256 del
    certificado con la huella histórica.

    La clave y su contraseña viven fuera del repositorio. Este script no las
    genera, no las copia y no las registra.

.PARAMETER Apk
    APK sin firmar o ya alineada que se va a firmar.

.PARAMETER Output
    Ruta del APK firmado. Por defecto, junto al original con sufijo `-signed`.

.PARAMETER CheckOnly
    Solo verifica un APK existente; no firma ni solicita contraseña.

.EXAMPLE
    powershell -NoProfile -File tools/android/sign-internal-apk.ps1 `
        -Apk C:\build\app-universal-release-unsigned.apk
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Apk,

    [string]$Output = "",

    [string]$Keystore = (Join-Path $HOME "HiQNet-Internal-Test-5262880\android\keystore\hiqnet-test.jks"),

    [string]$Alias = "hiqnet",

    [string]$BuildTools = "",

    [string]$ExpectedSha256 = "3A522CD050CF0728268F71855521CBAF2718CC88123D1EF5E1153FD0619EA072",

    [switch]$CheckOnly
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $Apk -PathType Leaf)) {
    throw "No existe el APK: $Apk"
}

if (-not $BuildTools) {
    $sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } elseif ($env:ANDROID_SDK_ROOT) { $env:ANDROID_SDK_ROOT } else { "" }
    if (-not $sdk) { throw "Define ANDROID_HOME o usa -BuildTools." }
    $BuildTools = Get-ChildItem -Directory (Join-Path $sdk "build-tools") |
        Sort-Object { [version]$_.Name } -Descending |
        Select-Object -First 1 -ExpandProperty FullName
}
$apksigner = Join-Path $BuildTools "apksigner.bat"
$zipalign = Join-Path $BuildTools "zipalign.exe"
if (-not (Test-Path $apksigner) -or -not (Test-Path $zipalign)) {
    throw "No se encontraron apksigner/zipalign en $BuildTools"
}

function Show-Certificate([string]$path) {
    $details = & $apksigner verify --verbose --print-certs $path 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "El APK no verifica: $details"
    }
    Write-Output $details
    $match = [regex]::Match($details, "SHA-256 digest: ([0-9a-fA-F]+)")
    if (-not $match.Success) { throw "No se pudo leer el SHA-256 del certificado." }
    return $match.Groups[1].Value.ToUpperInvariant()
}

if ($CheckOnly) {
    $digest = Show-Certificate $Apk
    if ($digest -ne $ExpectedSha256) {
        throw "Certificado inesperado: $digest (esperado $ExpectedSha256)"
    }
    Write-Output "certificado=OK"
    exit 0
}

if (-not (Test-Path -LiteralPath $Keystore -PathType Leaf)) {
    throw "No existe el keystore: $Keystore"
}
if (-not $Output) {
    $Output = [System.IO.Path]::ChangeExtension($Apk, $null).TrimEnd('.') + "-signed.apk"
}

$secure = Read-Host -AsSecureString "Contraseña del keystore (no se mostrará)"
$pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try {
    $plain = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    if ([string]::IsNullOrEmpty($plain)) { throw "La contraseña no puede estar vacía." }
    $env:HQNET_KS_PASS = $plain
    $env:HQNET_KEY_PASS = $plain
} finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
}

$aligned = [System.IO.Path]::ChangeExtension($Output, ".aligned.apk")
try {
    & $zipalign -p -f 4 $Apk $aligned
    if ($LASTEXITCODE -ne 0) { throw "zipalign falló." }

    & $apksigner sign --ks $Keystore --ks-key-alias $Alias `
        --ks-pass env:HQNET_KS_PASS --key-pass env:HQNET_KEY_PASS `
        --v2-signing-enabled true --v3-signing-enabled true `
        --out $Output $aligned
    if ($LASTEXITCODE -ne 0) { throw "apksigner sign falló." }
} finally {
    Remove-Item Env:HQNET_KS_PASS -ErrorAction SilentlyContinue
    Remove-Item Env:HQNET_KEY_PASS -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $aligned -ErrorAction SilentlyContinue
}

$digest = Show-Certificate $Output
if ($digest -ne $ExpectedSha256) {
    throw "El certificado firmado ($digest) no coincide con la huella histórica ($ExpectedSha256)."
}
Write-Output "firmado=$Output"
Write-Output "certificado=OK"
