# Licenciamiento POS ↔ Platform

Revisión conjunta de `hiqnet-pos` y `hiqnet-platform` para el piloto interno.
**No se activa emisión productiva**: este documento define el flujo, señala el
bloqueo actual y propone los cambios seguros que faltan.

## 1. Estado real (verificado en código)

| Pieza | Dónde vive | Estado |
|---|---|---|
| Contrato `LicenseGrant` v1 | `pos-licensing` y `platform-core::license` | Implementado, JSON canónico con orden de campos fijo |
| Firma Ed25519 | `platform-licensing::Ed25519Signer` | Implementada; semilla inyectada por `settings.signing_key_b64` |
| Emisión | `apps/api/src/licensing.rs` → `PlatformService::issue_license` | Implementada y protegida por `allow_dev_issuance` |
| Verificación | `pos-licensing::Ed25519LicenseVerifier` | Implementada **y probada** (firma válida, payload manipulado, clave ajena, vencimiento) |
| Consumo en el POS | `local_installation_state` (`license_grant_json`, `license_signature_b64`) | **Almacena, pero no verifica** |

### Hallazgo crítico (corregido)

~~`pos-licensing` es dependencia de `pos-desktop`, pero **`Ed25519LicenseVerifier`
no se invoca en ningún punto del ejecutable**.~~ **Corregido**: ver §6.

El estado comercial que aplica el POS (`plan`, `license_status`, `features`,
`valid_until`) se leía de la base local sin comprobar la firma, de modo que
editar el archivo SQLite bastaba para extender una licencia.

Lo que ya estaba resuelto: el contrato de firma, la emisión en Platform (con la
clave inyectada por configuración y nunca en el repositorio) y las pruebas de
manipulación del documento firmado.

## 2. Flujo productivo propuesto

```text
Platform (autoridad comercial)
  1. Verifica suscripción vigente y OrganizationProduct activo
  2. Resuelve entitlements del plan
  3. Construye LicenseGrant v1 { installation_id, organization_id, plan,
                                features, issued_at, valid_until, grant_id }
  4. Firma con Ed25519 (clave en KMS/HSM o gestor de secretos)
  5. Audita la emisión (grant_id, actor, instalación, plan, vigencia)
  6. Entrega el grant + firma (API autenticada)

POS (consumidor, sin clave privada)
  7. Recibe grant + firma y las persiste
  8. Verifica la firma con la clave pública embebida
  9. Compara installation_id/organization_id con su estado local
 10. Aplica status_at(now): Active | Trial | ReadOnly
 11. Audita instalación/renovación del grant
```

### Custodia de la clave privada

- **Nunca** en el repositorio, en el binario del POS ni en variables de entorno
  de un proceso interactivo.
- En producción: KMS/HSM con operación de firma, o secreto del gestor de
  secretos del despliegue inyectado sólo en el servicio de Platform.
- El POS sólo contiene la **clave pública**.
- Rotación: requiere identificador de clave (`kid`) en el grant → **v2 del
  contrato**. Con v1 la rotación exige recompilar el POS y emitir de nuevo.

### Identidad y elegibilidad

La elegibilidad comercial se decide **antes** de firmar, en Platform. El POS no
consulta la suscripción: confía en el documento firmado y en su vigencia. Por
eso `valid_until` es la frontera real de control offline.

### Renovación, gracia, revocación y recuperación

| Aspecto | Política propuesta |
|---|---|
| Renovación | Nuevo grant firmado con `valid_until` extendido; el POS reemplaza el anterior de forma atómica y audita |
| Expiración | `status_at(now)` → `ReadOnly` (ya implementado) |
| Periodo de gracia | Requiere campo propio (`grace_until`) o un grant con margen; hoy no existe en v1 |
| Recuperación sin internet | Reenviar el mismo grant firmado (idempotente por `grant_id`); no requiere estado de servidor |
| Revocación | Sin red no hay revocación inmediata: el límite es `valid_until`. Mantener vigencias cortas (30–90 días) acota la ventana |
| Licencia caducada | El POS sigue operando en sólo lectura: consultar, exportar y respaldar, sin nuevas ventas |

## 3. Cambios seguros que faltan (y su alcance)

| Cambio | Riesgo | Requiere autorización |
|---|---|---|
| Invocar `Ed25519LicenseVerifier` en el POS al leer el estado comercial | Medio: cambia la fuente de verdad | No (es corrección de un defecto) |
| Inyectar la clave pública por configuración de compilación/entorno | Bajo | No |
| Auditar instalación, renovación y rechazo de firma | Bajo | No |
| Añadir `grace_until` y `kid` (contrato v2) | Medio: rompe compatibilidad v1 | Sí (decisión de negocio) |
| Activar emisión productiva y crear secretos reales | Alto | **Sí, explícita** |
| Contratar KMS/HSM | Alto (coste) | **Sí** |

### Comportamiento propuesto al integrar el verificador

1. Sin `license_grant_json` → se mantiene el comportamiento actual (trial local).
2. Con grant presente y firma válida → `status_at(now)` manda.
3. Con grant presente y firma inválida o clave ausente → `ReadOnly`, evento de
   auditoría `license.signature_rejected` y mensaje neutral en la interfaz.

El punto 3 es el que cierra el agujero; los puntos 1 y 2 preservan la operación
del piloto y de las instalaciones de prueba.

## 4. Bloqueos declarados

- **Emisión productiva**: no se activa. `allow_dev_issuance` sigue siendo la
  única vía y produce documentos con clave de desarrollo.
- **Custodia de clave**: no existe todavía un KMS/HSM ni un procedimiento de
  rotación; crearlo implica decisión de infraestructura y coste.
- **Contrato v2**: `grace_until` y `kid` no están implementados; sin ellos no
  hay periodo de gracia formal ni rotación sin recompilar el POS.

## 5. Evidencia de las pruebas existentes

`pos-licensing` cubre hoy: aceptación de firma válida, rechazo de payload
manipulado, rechazo de firma de otra clave y transición a sólo lectura al
vencer. Todas usan una **clave sintética** (`[7u8; 32]`) que nunca se usa en
instaladores.

## 6. Estado tras la corrección (POS verifica licencias)

El POS ya no confía en `plan`, `features` ni `valid_until` cuando existe un
documento firmado. La resolución vive en un único punto
(`pos-application::service::helpers::resolve_commercial`) y **todas** las
mutaciones protegidas pasan por `PosService::require_commercial`.

| Situación | Comportamiento |
|---|---|
| Sin grant (`license_grant_json` vacío) | Trial local: se conserva el comportamiento previo |
| Grant válido (firma + versión + identidades) | Plan y features **exclusivamente del documento**; vigencia desde el grant |
| Firma inválida, documento incompleto o identidad que no coincide | Solo lectura, sin features, `tracing::warn` |
| Clave pública no disponible | Solo lectura (no cae al trial) |
| Grant vencido | Solo lectura por `effective_status`, conservando lectura y respaldo |

Puntos de control: arranque (`status`), consulta de plan (`commercial_state`),
cada mutación protegida (`require_commercial`), instalación/reemplazo
(`install_license`) y, tras reiniciar, la resolución se repite desde SQLite (no
hay caché en memoria que pudiera sobrevivir a una revocación).

La clave pública se embebe en tiempo de compilación con
`POS_LICENSE_PUBLIC_KEY`; solo en compilaciones de depuración se admite
`HIQNET_POS_LICENSE_PUBLIC_KEY` para pruebas. El binario no contiene clave
privada ni semilla.

### Modelo de amenazas y límites

| Ataque | Resultado |
|---|---|
| Editar `plan`, `features` o `valid_until` en SQLite | Sin efecto: el documento firmado manda |
| Sustituir firma o grant por otro documento | Rechazado → solo lectura |
| Sustituir `installation_id`/`organization_id` | Rechazado → solo lectura |
| Instalar un grant firmado con otra clave | Rechazado → solo lectura |
| **Rollback de licencia**: restaurar un grant antiguo válidamente firmado | **No protegido**: mientras no haya vencido, sigue siendo un documento auténtico. Se acota con vigencias cortas |
| **Manipulación del reloj local** | **No protegido** en local-first: adelantar el reloj no extiende una licencia (la fecha del documento manda), pero atrasarlo puede mantenerla vigente. Requiere contrapeso de servidor |
| Atacante que modifica el propio ejecutable | Fuera del modelo: quien controla el binario puede parchear la verificación |

### Evidencia

- 14 pruebas de integración con SQLite real en
  `pos-storage-sqlite/tests/licensing.rs` (casos positivos y las diez
  manipulaciones exigidas).
- Validación nativa (`tools/windows-native-e2e/e2e_license.py`) sobre el
  ejecutable real: trial, instalación de un grant de prueba, entitlements
  aplicados, persistencia tras reinicio, firma manipulada y licencia vencida.
