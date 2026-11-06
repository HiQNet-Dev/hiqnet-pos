# Internal Test Build 0.1.0 — evidencia Android

Build interno de pruebas. **No es una release comercial** y no se publica.

## Artefactos

| Artefacto | ABI | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `hiqnet-pos-0.1.0-internal-arm64.apk` (firmada) | `arm64-v8a` | 11 917 504 | `E45914FCA557F8680642D2546939D9B7B8C6EB66663FFCC5431EF91B70A5094A` |
| `hiqnet-pos-0.1.0-internal-arm64.signed.apk` (antes de firmar) | `arm64-v8a` | 11 855 213 | `EE12B2C2C39E2F3218144C6FD01AC596E6E3141F268C0036FBB737BF15874C4E` |
| `hiqnet-pos-0.1.0-internal-x86_64-emulador.apk` | `x86_64` | 12 728 509 | (solo emulador; no es un entregable) |

- Commit: `5262880` (`chore/integration-android-ux`).
- `libpos_desktop_lib.so` dentro de la APK ARM64: 10 239 592 bytes, SHA-256
  `E1E8E4ED19368316705F0B823FF6EE146BF3EB072AF448CFA361F1399665354B`
  (idéntico al generado por `cargo` para `aarch64-linux-android`).
- ABI incluida: **solo** `lib/arm64-v8a`. No se empaquetó `libc++_shared.so`: la
  biblioteca Rust ya enlaza el runtime de C++ y las únicas dependencias
  declaradas son `libandroid`, `libdl`, `liblog`, `libm` y `libc` del sistema.
- Frontend empaquetado dentro de la biblioteca Rust: `index-CyAARPWc.js` y
  `index-Cyn9ahAB.css`, los mismos hashes que `frontend/dist`. La aplicación **no
  requiere Vite ni `localhost:5173`** en ejecución.
- Firma: esquemas **v2 y v3** verificados con `apksigner`. Certificado
  `CN=HiQNet Internal Test, OU=QA, O=HiQNet`, digest SHA-256
  `3a522cd050cf0728268f71855521cbaf2718cc88123d1ef5e1153fd0619ea072`.
  La clave está **fuera del repositorio** y es exclusivamente de pruebas.

## Validación en emulador

Dispositivo: AVD `hiqnet_tablet`, Android 17, `x86_64`, 2560×1600 @ 320 dpi.
Se instaló la APK `x86_64` porque el emulador disponible no ejecuta `arm64-v8a`;
la APK ARM64 solo puede validarse en hardware físico.

| Comprobación | Estado | Evidencia |
| --- | --- | --- |
| Instalación limpia | PASS | `adb install` tras desinstalar la firma anterior |
| Arranque en frío | PASS | `am start -W` → `LaunchState: COLD`, `TotalTime: 778 ms` |
| Interfaz React embebida | PASS | captura `emulador-arranque-1.png` (onboarding) |
| Safe areas (barra de estado y navegación) | PASS | la tarjeta no queda bajo las barras del sistema |
| SQLite privada | PASS | `/data/user/0/com.hiqnet.pos/data/hiqnet-pos.db` (modo `-rw-------`) |
| Migraciones | PASS | 10/10 aplicadas: `init` … `sale returns` |
| Integridad | PASS | `integrity_check = ok`, `foreign_key_check` sin filas |
| Operación sin red | PASS | wifi y datos desactivados, `force-stop` y reapertura → captura `emulador-sin-red.png` |
| Ausencia de errores nativos | PASS | logcat sin `FATAL`, `AndroidRuntime` ni `panic` |
| Memoria en reposo | PASS | PSS total 100 373 KB (RSS 261 376 KB), Java 5 476 KB, nativo 24 912 KB |
| Onboarding, venta y persistencia por interfaz | NO EJECUTADO | requiere interacción táctil; no se automatiza con coordenadas |
| Respaldo/restauración SAF | BLOCKED | no implementado |
| Tablet ARM64 física | BLOCKED | sin hardware disponible |

Las cifras de memoria corresponden al emulador `x86_64` con renderizado por
software; no son extrapolables a una tablet ARM64 real.

## Respaldo en Android: estado real

`backup_database` (Rust) crea un snapshot consistente con `VACUUM INTO` en
`<datos de la app>/backups/` y lo audita. En Android ese directorio es privado y
el usuario no puede extraerlo sin herramientas de desarrollo.

**No se implementó SAF.** Sustituir ese comportamiento por una carpeta privada o
por un plugin Android sin revisión habría producido una función de recuperación
aparentemente operativa y no verificada, que es peor que declararlo pendiente.

Diseño propuesto, con autoridad en Rust:

1. Comando IPC `backup_database` sin cambios de semántica: sigue generando el
   snapshot y validándolo con `integrity_check`.
2. Adaptador móvil mínimo que reciba un `content://` URI desde
   `ACTION_CREATE_DOCUMENT` y escriba el archivo con `ContentResolver`.
3. Importación: `ACTION_OPEN_DOCUMENT` → copia a un archivo temporal privado →
   validación de formato, integridad y compatibilidad de esquema → respaldo
   previo de la base vigente → sustitución con las conexiones cerradas →
   reapertura.
4. Prohibido en cualquier caso: pedir permiso amplio de almacenamiento, tratar
   `content://` como ruta, o dejar una base parcialmente sustituida si falla.

## Limitaciones declaradas

- Sin tablet ARM64 física no se puede afirmar `ANDROID HARDWARE VALIDADO`.
- Sin SAF no hay recuperación externa verificada; Android queda apto para pruebas
  supervisadas con datos desechables, no para pruebas prolongadas.
- La APK AAB no se generó como entregable y no se publica.

## APK tras la auditoría UX (2026-11-05)

El frontend cambió con la auditoría UX/UI (`docs/04-quality/ux-audit-0.1.0.md`),
así que las APK de la tabla corresponden al commit `5262880` y no representan la
interfaz vigente. En esta sesión se compiló una APK **sin firmar** para
`arm64-v8a`:

| Artefacto | ABI | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `app-universal-release-unsigned.apk` | `arm64-v8a` | 11 850 965 | `6C3BD7E9134E551BEE0C6ABD328C6DE465D2005609493C2FCBE701D2FB81762E` |

Queda pendiente firmarla con la clave interna (fuera del repositorio), instalarla
en el emulador y revisar la interfaz táctil. La verificación en hardware ARM64
sigue **BLOCKED**.
