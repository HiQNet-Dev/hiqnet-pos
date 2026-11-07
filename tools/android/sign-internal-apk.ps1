<#
.SYNOPSIS
    Firma un APK de pruebas internas con la clave HiQNet y verifica el certificado.

.DESCRIPTION
    Toma las contraseñas de un almacén local protegido por DPAPI (solo el usuario
    actual puede descifrarlo) o las solicita de forma segura. Admite contraseñas
    distintas para el almacén y para la clave, sin asumir que son iguales.

    Después alinea, firma con esquemas v2/v3 y compara el SHA-256 del certificado
    con la huella histórica. Nunca escribe las contraseñas en disco, en la salida
    ni en el repositorio.

.PARAMETER Apk
    APK sin firmar o ya alineada.

.PARAMETER Output
    Ruta del APK firmado. Por defecto, junto al original con sufijo `-signed`.

.PARAMETER CredentialFile
    Archivo DPAPI creado por el procedimiento de rotación. Si no existe o se usa
    `-Prompt`, las contraseñas se solicitan de forma interactiva.

.PARAMETER Prompt
    Fuerza la solicitud interactiva aunque exista el archivo DPAPI.

.PARAMETER CheckOnly
    Solo verifica un APK existente; no firma ni solicita contraseñas.

.EXAMPLE
    powershell -NoProfile -File tools/android/sign-internal-apk.ps1 `
        -Apk C:\build\app-universal-release-unsigned.apk
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Apk,

    [string]$Output = "",

    [string]$Keystore = "C:\Users\gaelg\HiQNet-Internal-Test-5262880\android\keystore\hiqnet-test.jks",

    [string]$Alias = "hiqnet",

    [string]$BuildTools = "",

    [string]$ExpectedSha256 = "3A522CD050CF0728268F71855521CBAF2718CC88123D1EF5E1153FD0619EA072",

    [string]$CredentialFile = (Join-Path $env:LOCALAPPDATA "HiQNet\credentials\internal-keystore.clixml"),

    [switch]$Prompt,

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
    if ($LASTEXITCODE -ne 0) { throw "El APK no verifica: $details" }
    Write-Host $details
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

function Unprotect([string]$value) {
    $secure = ConvertTo-SecureString $value
    $pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
    try {
        return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    } finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
    }
}

function Read-Secret([string]$label) {
    $secure = Read-Host -AsSecureString $label
    $pointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
    try {
        return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($pointer)
    } finally {
        [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($pointer)
    }
}

$storeSecret = $null
$keySecret = $null
if (-not $Prompt -and (Test-Path -LiteralPath $CredentialFile -PathType Leaf)) {
    $stored = Import-Clixml -LiteralPath $CredentialFile
    if ($stored.keystore) { $Keystore = $stored.keystore }
    if ($stored.alias) { $Alias = $stored.alias }
    $storeSecret = Unprotect $stored.store
    $keySecret = Unprotect $stored.key
} else {
    $storeSecret = Read-Secret "Contraseña del almacén (no se mostrará)"
    $keySecret = Read-Secret "Contraseña de la clave (vacío = misma del almacén)"
    if ([string]::IsNullOrEmpty($keySecret)) { $keySecret = $storeSecret }
}

if ([string]::IsNullOrEmpty($storeSecret) -or [string]::IsNullOrEmpty($keySecret)) {
    throw "Las contraseñas no pueden estar vacías."
}
$env:HQNET_KS_PASS = $storeSecret
$env:HQNET_KEY_PASS = $keySecret

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
