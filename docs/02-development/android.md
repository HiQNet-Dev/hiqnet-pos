# Desarrollo y empaquetado Android

Android es una plataforma soportada del producto, no el entorno principal de
desarrollo. La app comparte el mismo núcleo Rust y el mismo frontend.

## Requisitos

| Elemento | Valor |
| --- | --- |
| Target Rust | `aarch64-linux-android` |
| NDK | `30.0.16248370` |
| SDK | Android SDK con `platform-tools` |
| JDK | 17 |
| Salida | APK ARM64 |

Variables esperadas por el flujo oficial:

```bash
export ANDROID_HOME="$HOME/Android/Sdk"     # o ANDROID_SDK_ROOT
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/30.0.16248370"
export NDK_HOME="$ANDROID_NDK_HOME"
rustup target add aarch64-linux-android
```

En Windows el SDK suele estar en `%LOCALAPPDATA%\Android\Sdk`; el script
`tools/android/build-apk.ps1` ya configura toolchain y verificaciones.

## Compilación

Windows:

```powershell
powershell -NoProfile -File tools/android/build-apk.ps1 -Profile debug
```

Linux o cualquier host con SDK/NDK:

```bash
just build-android
# equivale a:
# pnpm dlx @tauri-apps/cli@2.12.4 android build --target aarch64 --apk
```

`gen/android` es generado y está excluido de Git; `cargo tauri android init` se
ejecuta solo cuando falta.

## Datos y respaldo

La base vive en el almacenamiento privado de la aplicación
(`app_data_dir/data/hiqnet-pos.db`). El respaldo usa el plugin SAF
(`plugins/tauri-plugin-saf-document`) para exportar e importar documentos
elegidos por el usuario. La restauración solo se acepta para la misma
instalación.

## Firma

Los keystores, `.jks`, `.keystore`, `keystore.properties` y credenciales de
firma **nunca** se versionan. En Windows el procedimiento es
`tools/android/sign-internal-apk.ps1`, que puede leer credenciales protegidas por
DPAPI o solicitarlas de forma interactiva.

## CI

La compilación Android vive en `.github/workflows/android.yml` y es opt-in:

- ejecución manual (`workflow_dispatch`);
- PR etiquetado con `android` que toque código compartido.

No se ejecuta en cada PR de documentación o de escritorio.

Primera ejecución verificada en PR #22 (run `42372723076`):

- Gradle compiló la APK con JDK 17, SDK y NDK `30.0.16248370`.
- La APK contiene `lib/arm64-v8a/libpos_desktop_lib.so` y no otras ABIs.
- 193 494 769 bytes; SHA-256 `B404A74C3F134AD41C62B57229067687A48906BFFC24F39D5C5A97630B4E3B04`.
- Firmada con el certificado **Android Debug** (`CN=Android Debug`), no con la
  clave interna. El workflow no usa secretos de firma.
- Artefacto `hiqnet-pos-android-debug`, retención 7 días.

Esto valida toolchain, SDK/NDK, plugin SAF Kotlin, target ARM64 y empaquetado.
**No valida hardware**: SAF, exportación/importación y navegación siguen
pendientes de prueba en dispositivo.

## Límites

- No se versionan APK, AAB ni artefactos de firma.
- No se declara Android validado solo porque el APK compile: el flujo SAF,
  exportación/importación y navegación requieren prueba en dispositivo.
