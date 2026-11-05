# Validación prepiloto en Windows Sandbox

Arnés para instalar y operar HiQNet POS en una VM Windows desechable, sin tocar
la instalación de trabajo del anfitrión.

## Estado del entorno (preflight)

Comprobado el 2026-11-04 en el equipo de desarrollo, sin elevación:

| Dato | Valor |
| --- | --- |
| Edición | Windows 11 Pro |
| Versión | `10.0.26300` (x64) |
| Hipervisor | presente |
| Servicios `vmms` / `vmcompute` | en ejecución |
| Memoria | 31.9 GB |
| Procesadores lógicos | 12 |
| Disco libre | 324 GB |
| Sesión elevada | **no** |
| `C:\Windows\System32\WindowsSandbox.exe` | **ausente** |
| `Get-VM` | permiso denegado (falta pertenecer a *Hyper-V Administrators*) |

**Conclusión:** Windows Sandbox **no está habilitado** y no hay VM accesible. El
resto de requisitos (memoria, disco, hipervisor) es suficiente.

## Habilitar Windows Sandbox (requiere administrador)

Ejecutar en PowerShell **como administrador**, en el anfitrión:

```powershell
Enable-WindowsOptionalFeature -Online -FeatureName "Containers-DisposableClientVM" -All -NoRestart
```

Efectos: habilita la característica opcional *Windows Sandbox* (usa Hyper-V),
añade `WindowsSandbox.exe` y **requiere reiniciar** el equipo. No instala ni
desinstala aplicaciones del anfitrión y no cambia los adaptadores de red.

Después del reinicio:

```powershell
shutdown /r /t 0
```

Alternativa con Hyper-V (solo si se necesita persistencia entre reinicios):

```powershell
# Añade el usuario actual al grupo que administra VMs (requiere administrador).
Add-LocalGroupMember -Group "Hyper-V Administrators" -Member "$env:USERNAME"
```

Esta alternativa exige además una ISO de Windows y una licencia; Windows Sandbox
es más simple para una instalación limpia.

## Preparar artefactos

El repositorio **no** se monta en la VM. Solo se expone una carpeta con el
instalador y el script de verificación:

```powershell
pwsh -NoProfile -File tools/prepilot-sandbox/Prepare-SandboxArtifacts.ps1
```

Genera:

```text
%USERPROFILE%\HiQNet-Prepilot\
  artifacts\            (montada en la VM como C:\HiQNetArtifacts, solo lectura)
    HiQNet POS_0.1.0_x64-setup.exe
    sandbox-verify.ps1
    manifest.json       (versión, tamaño y SHA-256 del instalador)
  evidence\             (montada en la VM como C:\Evidence, escritura)
  prepilot-sandbox.wsb  (archivo de configuración ya renderizado)
```

Para probar una actualización, añade el instalador anterior:

```powershell
pwsh -NoProfile -File tools/prepilot-sandbox/Prepare-SandboxArtifacts.ps1 `
    -PreviousInstaller "D:\builds\HiQNet POS_0.0.9_x64-setup.exe"
```

## Ejecutar

```powershell
WindowsSandbox.exe "$env:USERPROFILE\HiQNet-Prepilot\prepilot-sandbox.wsb"
```

La VM arranca, ejecuta `sandbox-verify.ps1` y deja en `evidence\`:

- `prepilot-report.json` con cada comprobación (PASS/FAIL y detalle),
- `install.log`, `app-launch.log`,
- capturas de pantalla posteriores a cada arranque.

La sesión se destruye al cerrar la ventana: no queda ninguna instalación en el
anfitrión.

## Qué verifica el script

1. Versión de Windows y presencia de WebView2.
2. Instalación silenciosa (`/S`) y código de salida.
3. Archivos instalados y entrada de desinstalación en el registro.
4. Primer arranque: creación de `%LOCALAPPDATA%\HiQNet\POS\data\hiqnet-pos.db`,
   captura y verificación estructural con `pos-desktop.exe --verify-backup`.
5. **Operación offline:** desactiva los adaptadores *dentro de la VM*, vuelve a
   arrancar la aplicación y repite la verificación.
6. Desinstalación silenciosa: el programa desaparece y la base **permanece**.
7. Reinstalación: la base sigue intacta y la aplicación vuelve a arrancar.
8. Si hay instalador anterior: se instala primero, se arranca para crear la base
   y después se instala el candidato, comprobando que la base sobrevive.

## Límites conocidos

- El bootstrap de negocio y la venta requieren interacción; el script deja la
  aplicación abierta y captura la pantalla, pero **no automatiza formularios**.
  Esas operaciones se validan en el E2E nativo del anfitrión sobre SQLite
  aislada y deben completarse a mano en la sesión de sandbox cuando se revise la
  evidencia.
- La instalación puede necesitar Internet si falta WebView2: el paquete usa
  `downloadBootstrapper`. Si el instalador falla sin red, eso es un hallazgo, no
  un defecto del script.
- Windows Sandbox comparte la conexión del anfitrión; el script desactiva los
  adaptadores **de la VM** para la fase offline y no toca la red del anfitrión.
