# Internal Test Build 0.1.0 — evidencia Android

Build interno de pruebas. **No es una release comercial** y no se publica.

> El historial de artefactos se conserva abajo. La evidencia nueva de SAF vive en la rama
> aislada `feat/android-saf-internal-backup`; no sustituye la aprobación de una APK firmada ni
> una prueba instalada. Al verificar localmente el archivo histórico disponible
> `hiqnet-pos-0.1.0-internal-arm64.signed.apk`, `apksigner` devolvió `DOES NOT VERIFY` / `Missing
> META-INF/MANIFEST.MF`; ese archivo no se puede usar como prueba de continuidad del certificado.
> El JKS histórico sigue fuera de Git, pero la firma queda bloqueada hasta que un operador aporte
> la credencial mediante un canal seguro.

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
así que las APK de la tabla inicial corresponden a `5262880` y quedan como
historial. Artefactos de la pasada intermedia, compilados y firmados desde
`21bde50`:

| Artefacto | ABI | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `hiqnet-pos-0.1.0-internal-arm64.apk` (firmada, entrega) | `arm64-v8a` | 11 913 408 | `ACEA7BED64A22D487C148A9D7BCCB188EEF58FF6BD8337F2F3ACEF218C6E1FD1` |
| `hiqnet-pos-0.1.0-internal-arm64-unsigned.apk` (antes de firmar) | `arm64-v8a` | 11 851 029 | `4CFE084E0F15DE72BAB540EBD87A9213F1DBEF39B33B507D1D7C2CBD5AE0B75` |
| `hiqnet-pos-0.1.0-internal-x86_64-emulador.apk` (emulador, no es entregable) | `x86_64` | 12 728 509 | `F887E24ACB19AB818D4C4939374BE29AE84C31121B24A20B9303A98DAB64EA04` |

- Firma: `zipalign -p -f 4` + `apksigner sign` con `hiqnet-test.jks` (fuera del
  repositorio, carpeta `~/HiQNet-Internal-Test-5262880/android/keystore/`).
  `apksigner verify --print-certs` → esquemas **v2 y v3**, certificado
  `CN=HiQNet Internal Test, OU=QA, O=HiQNet`, digest SHA-256
  `3A522CD050CF0728268F71855521CBAF2718CC88123D1EF5E1153FD0619EA072`.
- Frontend embebido en `lib/arm64-v8a/libpos_desktop_lib.so`: `index-CPbDWAOQ.js`
  e `index-Hp6fmz8Q.css`, los mismos que `frontend/dist` del commit.
- ABI incluida: **solo** `lib/arm64-v8a`.
- Ubicación de la entrega: `~/HiQNet-Internal-Test-21bde50/android/`.

## Verificación en emulador (2026-11-05)

AVD `hiqnet_tablet`, Android 17, `x86_64`, 2560×1600 @ 320 dpi (`w1280dp h800dp`).
APK instalada: la de `x86_64`. La interacción se dirigió leyendo el árbol de
accesibilidad (`uiautomator dump`) y pulsando el centro del control, no por
coordenadas fijas.

| Comprobación | Estado | Evidencia |
| --- | --- | --- |
| Instalación limpia (`adb install -r` tras `pm clear`) | PASS | — |
| Arranque en frío | PASS | `am start -W` → `LaunchState: COLD`, 548–842 ms |
| Onboarding por interfaz | PASS | `emulador-01/02-onboarding*.png` |
| Inicio de sesión y shell con navegación expandida (1280 dp) | PASS | `emulador-03-ventas.png` |
| Navegación a Productos, Inventario, Cajas y turnos, Gestión | PASS | `emulador-04..07` |
| Modal de alta de producto (foco, cierre, acciones) | PASS | `emulador-08-modal-producto.png` |
| Alta real de producto (IPC de escritura) y aparición en Ventas | PASS | `emulador-14b/15/16` |
| Navegación en vertical 800 dp (riel compacto de 88 px) | PASS | `emulador-18-catalogo-desplazado.png` |
| Teclado virtual: el viewport se reduce y el campo sigue visible | PASS | `emulador-12-teclado-ventas.png` |
| Safe areas con `enableEdgeToEdge` | PASS | contenido bajo la barra de estado y sobre la de gestos |
| Back con un diálogo abierto | **FAIL en `5262880`** → **PASS en `1821d2d`** | ver «Corrección del Back de Android» |
| Scroll con desbordamiento real | NO EJECUTADO | el fixture tenía 7 productos y caben en pantalla |
| Hardware ARM64 físico | BLOCKED | sin dispositivo |

### Corrección del Back de Android (2026-11-06)

#### Causa raíz

Con el modal de «Nuevo producto» abierto, `KEYCODE_BACK` cerraba el diálogo y
además finalizaba `MainActivity`; el proceso moría y Android registraba

```text
F libc  : FORTIFY: pthread_mutex_lock called on a destroyed mutex
```

en el hilo de Rust durante el desmontaje (`onActivityFinishing`). Con
`am force-stop` no aparecía: el aborto era específico de `Activity.finish()` y,
además, **intermitente** (3 de 5 y 1 de 5 intentos en dos series), así que una
sola ejecución no demostraba nada.

El origen no estaba en la aplicación. Tauri 2.11.6 registra en
`AppPlugin.kt` un `OnBackPressedCallback(true)` que reenvía el evento
`back-button` a JavaScript. Cuando **no hay ningún listener registrado**, el
plugin comprueba si la WebView puede retroceder y, al no poder hacerlo —una SPA
de una sola entrada nunca puede—, llama a `activity.onBackPressed()`, que
ejecuta `finish()`. Cada pulsación de Back terminaba la Activity.

#### Solución aplicada

API oficial `onBackButtonPress` de `@tauri-apps/api/app` (2.12.4; disponible
desde 2.9). **No se añadió ningún adaptador Kotlin** y `src-tauri/gen/android/`
sigue intacto: nada que se pierda al regenerar el proyecto Android.

- `frontend/src/lib/backNavigation.ts`: pila de manejadores (gana el último
  registrado), un único listener nativo con conteo de referencias para
  sobrevivir a React StrictMode, liberación idempotente y baja diferida si la
  promesa de registro se resuelve después del desmontaje.
- `App.tsx` instala el guardia una vez por ciclo de vida.
- `Modal.tsx` registra una capa que llama al mismo `onClose` que la X,
  «Cancelar» y Escape: no hay un segundo mecanismo de cierre.
- `SalesScreen.tsx` registra una capa que devuelve del ticket al catálogo sin
  tocar las partidas.

Prioridad efectiva: **teclado del sistema** (lo consume Android antes del
callback) → **diálogo** → **vista de ticket** → **raíz**, donde el evento se
consume sin finalizar la Activity. En Windows el evento no existe y no cambia
nada; Escape, foco y modales conservan su comportamiento.

#### Matriz de Back sobre la APK real

`.devkit/back-matrix.py` sobre `hiqnet-pos-0.1.0-internal-x86_64-emulador.apk`
(compilada desde `1821d2d`), leyendo el árbol de accesibilidad para cada toque.

| Escenario | Resultado |
| --- | --- |
| Back en onboarding | PASS: no cierra y conserva el formulario |
| Back en pantalla raíz | PASS: la app sigue viva y en Ventas |
| Back con diálogo abierto | PASS: cierra solo el diálogo; PID sin cambios |
| Back repetido en diálogo | PASS: un solo cierre, sin salir |
| 20 ciclos modal → Back → reapertura → Back | PASS: 20/20 |
| Back con teclado virtual abierto | PASS: cierra el teclado y conserva el diálogo |
| Vista de ticket | PASS: vuelve al catálogo y conserva la partida |
| Back durante el checkout | PASS: cierra el diálogo y conserva el ticket |
| Back tras confirmar la venta | PASS: sin cierre y ticket vacío |
| Gesto del sistema (deslizar desde el borde) | PASS: mismo comportamiento que el botón |
| Back tras background/resume | PASS |
| Force-stop y arranque posterior | PASS |
| Back en login | PASS: no cierra |
| Abortos nativos (`FORTIFY`, `Fatal signal`) | PASS: 0 coincidencias en logcat |
| Persistencia tras reinicio | PASS: «Venta confirmada» en Gestión, 0 abortos |

**33/33 PASS.** Evidencia: `after/46-android-back-modal-cerrado.png`,
`after/47-android-back-ticket.png` y
`after/48-android-back-persistencia.png`. El criterio no fue «el diálogo
desaparece»: cada comprobación verifica además que el proceso sigue vivo y que
no aparece ningún aborto en `logcat`.

#### Artefactos vigentes

Frontend, iconos y Back cambian respecto a `21bde50`, así que los instaladores se
regeneran desde `1821d2d` (release, assets `index-BsmKnnR5.js` e
`index-9D_khkjU.css`).

| Artefacto | ABI | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `hiqnet-pos-0.1.0-internal-arm64.apk` (firmada, entrega) | `arm64-v8a` | 11 860 301 | `FC80389DE88B92B687F983CFB6A4EA1BF61D4F8BF684CE15A196C59B6EC84206` |
| `hiqnet-pos-0.1.0-internal-arm64-unsigned.apk` (antes de firmar) | `arm64-v8a` | 11 852 553 | `8E735EB8C33417DC709A2DE30EA9704F23EA5328755C40274A1208BE21701A9C` |
| `hiqnet-pos-0.1.0-internal-x86_64-emulador.apk` (emulador, no es entregable) | `x86_64` | 12 675 402 | `1BF089123F721D5781C26D8AAE254ACFA64881E7EC8C8143ED77176C83EE760B` |

- `lib/arm64-v8a/libpos_desktop_lib.so` dentro de la APK ARM64: 10 239 504
  bytes, SHA-256
  `CF2F1FC795BCA42B63993B7F3E0B5B2475C71A8351FD83AF39D32A2A68A0A237`.
- Firma: `zipalign -p -f 4` + `apksigner sign` con `hiqnet-test.jks` (fuera del
  repositorio). `apksigner verify --print-certs` → esquemas **v2 y v3**,
  certificado `CN=HiQNet Internal Test, OU=QA, O=HiQNet`, digest SHA-256
  `3A522CD050CF0728268F71855521CBAF2718CC88123D1EF5E1153FD0619EA072`.
- ABI incluida: **solo** `lib/arm64-v8a`; la variante `x86_64` existe únicamente
  para el emulador.
- Ubicación de la entrega: `~/HiQNet-Internal-Test-backfix2/android/`.

#### Artefactos de la pasada de precios (2026-11-06)

El contrato de catálogo y la interfaz cambiaron, así que la APK se regenera desde
`f5744d9`. Los artefactos de `1821d2d` (`FC80389D…`) quedan como historial.

| Artefacto | ABI | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `hiqnet-pos-0.1.0-internal-arm64.apk` (firmada, entrega) | `arm64-v8a` | 11 905 357 | `28664634C5E5F8729CAC37A86E4968A2B7D0628155340783E3108DAAFBEC23E0` |
| `hiqnet-pos-0.1.0-internal-arm64-unsigned.apk` (antes de firmar) | `arm64-v8a` | 11 900 193 | `B81EB9708F33CEE3C657C6B15C13EA9FAB0CF152B6D8E88B014602015AF1A422` |
| `hiqnet-pos-0.1.0-internal-x86_64-emulador.apk` (emulador, no es entregable) | `x86_64` | 12 728 650 | `33822F7E08F405E3B19AAC6255215E5D4A6E1A3AA7599DD616A0A3BB4412CB5F` |

- `lib/arm64-v8a/libpos_desktop_lib.so`: 10 287 144 bytes, SHA-256
  `632B161FEB9293BB0736E155A741D94916F700D57701300F591081BD61AE9CA4`.
- Frontend embebido: `index-Bx8ah-xo.js` e `index-CiRAZ_O3.css` (presentes en la
  biblioteca).
- Firma verificada con `apksigner`: esquemas **v2 y v3**, certificado
  `3A522CD050CF0728268F71855521CBAF2718CC88123D1EF5E1153FD0619EA072`.
- Ubicación: `~/HiQNet-Internal-Test-pricing/android/`.

Verificación en emulador `x86_64` (Android 17, 2560×1600) con esta APK: matriz de
Back 33/33 PASS, 0 abortos nativos, 20/20 ciclos modal/Back, venta confirmada y
persistencia tras reinicio. La tarjeta de Ventas publica el precio al público
(MXN 11.60 para 10.00 + 16 % y MXN 13.92 para la variante de 12.00) en
`after/50-android-precio-publico.png`.

#### Bloqueos que siguen abiertos

- **Hardware ARM64 físico: BLOCKED.** Solo emulador `x86_64` (Android 17). No se
  declara `ANDROID HARDWARE VALIDADO`.
- **SAF (respaldo/restauración externa): BLOCKED.** No implementado.
- Scroll Android con desbordamiento real: no forzado en el emulador (el fixture
  cabe en pantalla); verificado en Playwright y en el E2E nativo de Windows.

**Android no se declara terminado por completo**: Back, navegación, modales y
tecleado están verificados en emulador; SAF y hardware ARM64 siguen pendientes.

## SAF en la rama aislada `feat/android-saf-internal-backup` (2026-11-06)

Esta pasada no modifica `src-tauri/gen/android`: el transporte SAF vive en el plugin local
versionado `plugins/tauri-plugin-saf-document` (Rust + Kotlin) y Rust conserva la autoridad sobre
snapshots, validación, identidad, estado comercial y sustitución de la base.

### Artefacto compilado

| Artefacto | ABI | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `app-universal-release-unsigned.apk` | `arm64-v8a` | 12 057 633 | `6A02DAD9B741DEFAC4763D860ED4CBB27642D3B9628899870610D1F62782D023` |

- `aapt2 dump badging` → `package=com.hiqnet.pos`, `versionName=0.1.0`,
  `targetSdkVersion=37`, `native-code: 'arm64-v8a'`.
- **Sin firma.** `apksigner` no puede verificarla; no es una entrega instalable.
- El binario Windows de esta misma pasada se generó como NSIS:
  `HiQNet POS_0.1.0_x64-setup.exe`, 3 918 388 bytes, SHA-256
  `2DD5E0F6C02BD854C8CF3509D8C2345D55B9AF2D8659DEB8979D2F27BC502616`.

### Verificación en emulador desechable

AVD nuevo `hiqnet_saf_test` (Android 17, `x86_64`, 2560×1600 @ 320 dpi), creado desde
`system-images;android-37.0;google_apis;x86_64`. La instalación firmada preexistente de
`hiqnet_tablet` **no** se desinstaló ni se sobrescribió. La app de prueba usa la clave debug
estándar solo en este AVD y no se publica.

Ciclo ejecutado dos veces, con datos sintéticos y árbol de accesibilidad en cada paso:

1. onboarding real y bootstrap;
2. **Exportar respaldo** → selector `ACTION_CREATE_DOCUMENT` → `Downloads/hiqnet-pos-*.db`
   (442 368 bytes). El temporal privado se eliminó después de escribir;
3. el archivo se extrajo y se verificó con `pos-desktop.exe --verify-backup` → `resultado=OK`;
4. se creó un producto posterior al respaldo;
5. **Importar respaldo** → selector `ACTION_OPEN_DOCUMENT` → validación Rust → diálogo de
   confirmación con vigencia y «Se conserva el estado vigente»;
6. **Restaurar y reiniciar** → la app se cierra (sin `pid`),
7. nuevo arranque en frío consume el marcador antes de abrir SQLite, exige login y muestra
   «El respaldo se restauró correctamente.»;
8. el producto posterior ya no existe y el Trial conserva su vencimiento.

Casos adversariales en el mismo dispositivo:

| Caso | Resultado | Detalle |
| --- | --- | --- |
| Archivo corrupto | PASS | «el respaldo no es válido»; app viva, sin reinicio |
| `installation_id` ajeno | PASS | «el respaldo pertenece a otra instalación u organización» |
| Temporales | PASS | export eliminado; import del ciclo 2 eliminado tras restaurar |
| Abortos nativos | PASS | 0 `FORTIFY`, `Fatal signal`, `pthread_mutex_lock`, `panic` |

Evidencia local en `.devkit/artifacts/android-saf/` (capturas `01`–`25` y árboles `ui-01`–`ui-32`).

### Firma: BLOCKED

El keystore histórico existe fuera del repositorio
(`~/HiQNet-Internal-Test-5262880/android/keystore/hiqnet-test.jks`), pero esta sesión no dispone de
la credencial autorizada y no se adivinan contraseñas. Comprobar el archivo histórico
`hiqnet-pos-0.1.0-internal-arm64.signed.apk` con `apksigner` devolvió
`DOES NOT VERIFY: Missing META-INF/MANIFEST.MF`, así que tampoco sirve para confirmar la
continuidad del certificado. **No hay APK firmada de esta pasada.**

### Estado por etapa

- `ANDROID BUILD VALIDADO`: sí (release ARM64 sin firmar).
- `ANDROID FIRMA VALIDADA`: **no** (BLOCKED).
- `ANDROID E2E EMULADOR APROBADO`: sí, sobre el build de depuración instalado en el AVD aislado.
- `ANDROID BACKUP/RESTORE SAF APROBADO`: sí en emulador; no en APK firmada de distribución.
- `ANDROID HARDWARE ARM64 PENDIENTE`: sí.

## Bloqueos restantes

- Firma interna v2/v3: requiere la contraseña del JKS por un canal seguro.
- Hardware ARM64 físico: sin dispositivo disponible.
- SAF en la APK firmada: depende de la firma y de una instalación autorizada.
- Scroll Android con desbordamiento real en dispositivo: no forzado.

## Cierre de firma, release E2E y actualización (2026-11-06)

### Firma interna recuperada

El keystore histórico se usó **sin copiarlo al repositorio** y sin imprimir la contraseña. El APK
final verifica con esquemas **v2 y v3** y certificado
`CN=HiQNet Internal Test, OU=QA, O=HiQNet`, SHA-256
`3A522CD050CF0728268F71855521CBAF2718CC88123D1EF5E1153FD0619EA072`.

| Artefacto | ABI | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `hiqnet-pos-0.1.0-internal-arm64.apk` | `arm64-v8a` | 12 122 640 | `82AD7A9163594B0FEB1E55A1C7C9503DD9112C463D16052A087DD4C7ED08E32F` |
| `hiqnet-pos-0.1.0-internal-x86_64-emulador.apk` (solo actualización en AVD) | `x86_64` | 12 970 509 | `04EDE9A6C21E1B4BDC600E3B55E14C49611DA851DA2BE2E5E513243DDFBF9F0B` |
| `hiqnet-pos-0.1.0-avd-test-x86_64.apk` (clave aislada, no entregable) | `x86_64` | 12 970 511 | `6275C09B05D98DE39F838FBB5B8942321B83DAD18E3F28F03DA0F140C1570E03` |

`zipalign -c -p 4` pasa y `aapt2` confirma `native-code: 'arm64-v8a'` y `targetSdk 37`. La APK
ARM64 se ubica en `~/HiQNet-Internal-Test-saf/android/` junto con los dos artefactos de emulador.
El NSIS de esta misma pasada es `HiQNet POS_0.1.0_x64-setup.exe`, 3 918 937 bytes, SHA-256
`81989E2133E7821F121EBE9AAD7D8F0E53778A2885AB687D848A37159A8B5BCC`.

Procedimiento: `tools/android/sign-internal-apk.ps1`. Solicita la contraseña con `SecureString`,
la expone solo en variables de entorno del proceso, firma con `--v2-signing-enabled` y
`--v3-signing-enabled`, y falla si el SHA-256 del certificado no coincide con el histórico.

> La contraseña del keystore se rotó tras la exposición: storepass y keypass son distintos y
> viven en `%LOCALAPPDATA%\HiQNet\credentials\internal-keystore.clixml` protegidos por DPAPI.
> `LEEME.txt` ya no contiene contraseñas. El par de claves y el certificado no cambiaron.

### E2E sobre APK release (AVD desechable)

AVD `hiqnet_saf_test`, Android 17, `x86_64`, 2560×1600. La APK de E2E se firmó con una identidad
**aislada** (`CN=HiQNet AVD Test`), distinta de la interna, para no confundirla con una entrega.

| Paso | Resultado |
| --- | --- |
| Instalación limpia y arranque en frío | PASS (967 ms) |
| Onboarding y bootstrap | PASS |
| Cierre de sesión y login | PASS |
| Categoría y producto | PASS (CAF-001) |
| Precio público con IVA | PASS ($11.60; IVA $1.60) |
| Entrada de existencias | PASS (20) |
| Caja y turno con fondo inicial | PASS ($100.00) |
| Venta en efectivo | PASS (folio V-D4C85A24F4; total $11.60; cambio $8.40 con $20.00) |
| Persistencia tras `force-stop` | PASS (venta en auditoría) |
| Exportación SAF | PASS (442 368 bytes; `--verify-backup` → `resultado=OK`) |
| Importación + reinicio | PASS (aviso correcto; producto posterior revertido) |
| Integridad y Trial | PASS (`integrity_check=ok`, mismo vencimiento) |
| Back en modal y en raíz | PASS (cierra solo el modal; el proceso sigue vivo) |
| Teclado virtual | PASS (campo visible; Back oculta el IME y conserva el modal) |
| Scroll con desbordamiento real | PASS (61 productos; «Producto 60» visible y agregado al ticket) |
| Suspensión y reanudación | PASS (ticket conservado) |
| Operación sin Internet | PASS (Wi-Fi y datos desactivados; login y catálogo locales) |
| Abortos nativos | PASS (0 `FORTIFY`/`Fatal signal`/`panic`) |

### Actualización entre APK

La APK histórica `hiqnet-pos-0.1.0-internal-x86_64-emulador.apk` (commit `5262880`) **sí** verifica
con el certificado interno. Sobre el mismo AVD se instaló esa versión, se creó el producto
`UPD-001` y se aplicó la nueva APK con `adb install -r`, sin desinstalar.

| Comprobación | Resultado |
| --- | --- |
| Instalación de actualización con la misma clave | PASS (`Success`) |
| Datos previos | PASS (UPD-001 conserva nombre y SKU) |
| Nuevo precio público | PASS ($14.50 para 12.50 + 16 %) |
| Migraciones tras actualizar | PASS (11/11, `max=11`) |
| Integridad y claves foráneas | PASS (`integrity_check=ok`, 0 violaciones) |
| SAF tras actualizar | PASS (exportación y verificación) |

### Concurrencia y robustez del plugin

Se corrigieron riesgos reproducibles en `SafDocumentPlugin`:

- el turno se mantiene tomado hasta terminar la copia (antes se liberaba al volver del selector);
- executor compartido y daemon, sin acumular hilos al recrear la Activity;
- `ContentResolver` del contexto de aplicación, no de la Activity que abrió el selector;
- excepción al abrir el selector capturada y traducida a error recuperable;
- copia en streaming con tope de 512 MiB, sin cargar el archivo en memoria;
- borrado best-effort del documento parcial si la escritura falla;
- respuestas ignoradas si el proceso ya no escucha.

Cubierto por 7 pruebas JVM (`OperationGateTest`, `DocumentIoTest`) ejecutadas con
`:tauri-plugin-saf-document:testDebugUnitTest`.

### Instalador Windows en Sandbox (esta pasada)

Ejecución aislada en Windows Sandbox con la red activa solo durante la instalación (para que el
instalador obtenga WebView2) y desactivada antes del arranque:

| Comprobación | Resultado |
| --- | --- |
| Instalación silenciosa desde cero | PASS (`install_exit=0`) |
| Archivos, desinstalador y versión `0.1.0` | PASS |
| Arranque offline | PASS |
| Base creada | PASS (11 migraciones, `ok`, 0 FK) |
| Desinstalación | PASS (programa eliminado) |
| Datos tras desinstalar y reinstalar | PASS (conservados) |
| Firma de editor | `NotSigned` (declarado) |

No se modificó la instalación real del host. Un rebuild desde el árbol congelado y la nueva
contraseña produjo exactamente los mismos SHA-256, lo que confirma el build reproducible.

El informe completo está en `docs/04-quality/internal-pilot-acceptance-0.1.0.md`.

### Estado por etapa (esta pasada)

- `ANDROID BUILD VALIDADO`: sí (release ARM64 y x86_64).
- `ANDROID FIRMA VALIDADA`: sí, v2/v3 con el certificado histórico.
- `ANDROID E2E EMULADOR APROBADO`: sí, sobre APK **release**.
- `ANDROID BACKUP/RESTORE SAF APROBADO`: sí en release sobre AVD.
- `ACTUALIZACIÓN DE APK PRESERVANDO DATOS`: sí en emulador `x86_64`.
- `ANDROID HARDWARE ARM64 PENDIENTE`: sí; no se declara validación física.
