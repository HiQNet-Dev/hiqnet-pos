# Licenciamiento POS ↔ Platform

Revisión conjunta de `hiqnet-pos` y `hiqnet-platform` para el piloto interno.
**No se activa emisión productiva**: este documento define el flujo, señala el
bloqueo actual y propone los cambios seguros que faltan.

## 1. Estado real (verificado en código)

| Pieza | Dónde vive | Estado |
|---|---|---|
| Contrato `LicenseGrant` v1 | `pos-licensing` y `platform-core::license` | Implementado, JSON canónico con orden de campos fijo |
| Firma Ed25519 | `platform-licensing::Ed25519Signer` | Implementada; semilla inyectada por `settings.signing_key_b64` |
| Emisión DEV | `apps/api/src/licensing.rs` → `PlatformService::issue_license` | Implementada y protegida por `allow_dev_issuance` |
| Emisión comercial | `apps/api/src/control_licensing.rs` → `PlatformService::issue_entitled_license` | Implementada (entorno de pruebas): sesión staff y `InstallationsManage`, elegibilidad desde `entitlements`, plan/features/vigencia derivados del derecho comercial, duplicados 409 por unicidad (instalación, suscripción) y auditoría transaccional. Habilitada sólo con `HIQNET_PLATFORM_ALLOW_LICENSE_ISSUANCE=true` y firmante configurado |
| Verificación | `pos-licensing::Ed25519LicenseVerifier` | Implementada **y probada** (firma válida, payload manipulado, clave ajena, vencimiento) |
| Consumo en el POS | `local_installation_state` (`license_grant_json`, `license_signature_b64`, `license_installed_at`) | Almacena y **verifica**; el marcador `license_installed_at` impide que borrar el grant reinicie el trial |

### Hallazgo crítico (corregido)

~~`pos-licensing` es dependencia de `pos-desktop`, pero **`Ed25519LicenseVerifier`
no se invoca en ningún punto del ejecutable**.~~ **Corregido**: ver §6.

### Hallazgo adicional corregido (2026-11)

~~Borrar el grant de SQLite hacía caer la resolución en el fallback del trial y
el POS volvía a confiar en las columnas `plan`/`license_status`/`valid_until`
que la licencia había escrito.~~ **Corregido**: la migración POS 0012 añade el
marcador persistente `license_installed_at`; `resolve_commercial` sólo acepta el
fallback del trial en instalaciones que **nunca** tuvieron licencia. Con marcador
y sin grant → solo lectura (razón `grant_absent`), sin reinicio de trial y sin
columnas que autorizen por sí solas.

**Migración de bases históricas**: 0012 rellena el marcador cuando la fila ya
tenía un documento almacenado (grant + firma persistidos); es la única evidencia
usada, nunca las columnas comerciales sin verificar. El guardado posterior
nunca reinicia el marcador a NULL (`COALESCE` en el upsert). Una instalación
que perdió toda evidencia de licenciamiento **antes** de actualizar no puede
reconstruir ese hecho con certeza a partir de SQLite: queda como trial local.

**Compatibilidad de claves de features**: las claves con punto
(`backups.local`, `sync.cloud`, `admin.remote`, `enterprise.integrations`) son
las canónicas del contrato v1 y Platform las emite así; el verificador POS las
acepta sin conversión (renombrado serde explícito en `FeatureKey`). La prueba
de interoperabilidad con el fixture real de Platform cubre este caso.

El estado comercial que aplica el POS (`plan`, `license_status`, `features`,
`valid_until`) se leía de la base local sin comprobar la firma, de modo que
editar el archivo SQLite bastaba para extender una licencia. Corregido: ver §6;
el documento firmado manda siempre que exista.

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
  única vía para el flujo DEV de planes/vigencias arbitrarias, y la emisión
  comercial controlada requiere además `HIQNET_PLATFORM_ALLOW_LICENSE_ISSUANCE`
  (desactivada por defecto; sólo pruebas controladas).
- **Custodia de clave**: no existe todavía un KMS/HSM ni un procedimiento de
  rotación; crearlo implica decisión de infraestructura y coste.
- **Contrato v2**: `grace_until` y `kid` no están implementados; sin ellos no
  hay periodo de gracia formal ni rotación sin recompilar el POS.

### Decisiones comerciales pendientes (contrato v1 congelado)

El contrato `LicenseGrant` v1 no incorpora `kid` ni `grace_until` de forma
unilateral. Quedan abiertas, y requieren decisión de negocio antes de cualquier
emisión productiva:

1. **Rotación de claves**: hoy exige recompilar el POS (una clave pública
   embebida). v2 añadiría `kid` y una lista de claves aceptadas.
2. **Vigencia y renovación**: duración estándar (propuesta actual: 30–90 días)
   y mecanismo de renovación (endpoint dedicado vs. reemisión por suscripción).
3. **Periodo de gracia**: si el POS tolerará operación limitada tras el
   vencimiento (`grace_until` en v2) o solo lectura inmediata (comportamiento
   actual de v1).
4. **Revocación offline**: la ventana depende de `valid_until`; vigencias cortas
   acotan el riesgo pero multiplican la operación de emisión.

### Compatibilidad del JSON canónico (v1)

Las fechas del grant se serializan con el formato por defecto de `time` en
ambas implementaciones (POS y Platform). Ambos lados deben evolucionar en
paralelo: cualquier cambio de formato o de campos exige subir `version` y
migrar el verificador del POS. Un v2 debería fijar RFC 3339 explícitamente.

**Prueba de interoperabilidad real**: el fixture
`platform-grant-v1.json` (generado con las primitivas reales de Platform, datos
sintéticos y semilla de pruebas) se instala en el POS tal cual —sin
reconstruir ni volver a firmar— en
`pos-storage-sqlite/tests/platform_grant_interop.rs`. Platform regenera el
documento en cada CI y lo compara byte a byte
(`platform-licensing/tests/interop_fixture.rs`): cualquier drift del contrato
(ordenes de campos, formato de fechas, firma) rompe antes de llegar al POS.
Detección histórica de drift: las claves de features con punto ya se cubren
explícitamente en el verificador.

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
| Sin grant y sin marcador (instalación nueva) | Trial local: se conserva el comportamiento previo |
| Sin grant pero con marcador (`license_installed_at`) | Solo lectura (razón `grant_absent`): borrar el grant no reinicia el trial ni autoriza con columnas sin firmar |
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

## 7. Preparación de la emisión productiva (análisis, sin provisión)

El POS ya verifica licencias. Lo que falta está del lado de Platform. Estado
verificado de cada pieza:

| Requisito | Estado real | Qué falta |
|---|---|---|
| Elegibilidad comercial | El endpoint de emisión comercial resuelve suscripción vigente y `OrganizationProduct` activo vía `CommercialService::entitlements` antes de firmar; instala ajena → 404, sin derecho → 403 | Nada para el piloto; quedan decisiones de negocio (abajo) |
| Suscripciones y entitlements | Implementados en `commercial_domain`; la traducción plan→features ocurre en un único lugar (`license_issuance::EntitledPlan`) | Nada; un plan o capacidad no representable en v1 rechaza la emisión (422 `unsupported_plan`/`unsupported_feature`), sin conversiones silenciosas |
| Firma Ed25519 | `Ed25519Signer` con semilla inyectada por `signing_key_b64` | Que la semilla venga de un KMS/HSM y no de una variable de entorno plana |
| Clave pública en el POS | Embebida con `POS_LICENSE_PUBLIC_KEY` | Definir el par de claves del piloto y compilar el binario con la pública |
| Auditoría de emisión | Evento `license.issued` (transaccional con el registro del grant) y `license.issuance_denied` para rechazos; sin secretos | Consulta del historial de emisión para Control (lectura) si se desea UI |
| Renovación | El POS reemplaza el grant de forma atómica y audita | Endpoint de renovación en Platform y entrega autenticada al POS |
| Vigencia y gracia | `valid_until` aplica solo lectura al vencer; no hay gracia | Decidir si se añade `grace_until` (contrato v2) o se emite con margen |
| Rotación de claves | El POS embebe una clave pública | Añadir `kid` al grant (v2) y una lista de claves aceptadas |
| Revocación offline | No existe: el límite es `valid_until` | Aceptar vigencias cortas (30–90 días) o añadir contrapeso de servidor |
| Autenticación de la entrega | La API exige sesión staff; la emisión controlada vive en `/api/v1/control/installations/{id}/licenses` | Definir si el POS consumirá la entrega por API autenticada o por archivo firmado |
| Idempotencia y duplicados | Unicidad parcial (instalación, suscripción) en `issued_license`; duplicado → 409; reintentos tras fallo o pérdida del grant emiten un grant nuevo | Nada para el piloto |
| Transiciones POS | Marcador `license_installed_at` (migración 0012): borrar el grant deja la instalación en solo lectura; el trial sólo existe para instalaciones nuevas | Nada |

### Secuencia recomendada

1. **Par de claves del piloto**: generar offline, guardar la privada fuera del
   repositorio y compilar el POS con la pública. Sin coste de infraestructura.
2. **Emisión controlada**: endpoint de emisión comercial con elegibilidad,
   duplicados y auditoría. Aún con clave local, no con KMS. La habilitación es
   explícita por entorno (`HIQNET_PLATFORM_ALLOW_LICENSE_ISSUANCE`) y su valor
   por defecto es desactivado.
3. **KMS/HSM**: solo cuando haya clientes reales. Es el único punto con coste
   recurrente; requiere cotización y autorización.
4. **Contrato v2** (`kid`, `grace_until`): decisión de negocio; hoy todo lo
   demás es compatible con v1.

### Costos

Los únicos costos son de custodia de clave (KMS/HSM gestionado) y, si se
quisiera revocación con contrapeso de servidor, de un servicio en línea. Este
documento no estima importes: requieren cotización real y autorización previa.
Nada de esto se ha provisionado.
