# ADR-004 — Licencias locales y entitlements firmados

- Estado: **propuesta técnica**, sujeta a ratificación (D-03, D-04, D-09)
- Fecha: 2026-11-03

## Decisión propuesta

1. HiQNet Platform emite una autorización de derechos firmada con **Ed25519**.
2. El POS solo embebe la **clave pública** necesaria para verificar.
3. El documento firmado (`LicenseGrant`) contiene: `version`, `grant_id`,
   `installation_id`, `organization_id`, `plan`, `features`, `issued_at` y
   `valid_until`.
4. La verificación se implementa en `pos-licensing`; el estado comercial
   observado se persiste en `local_installation_state`.
5. El Trial local dura 30 días y habilita las capacidades de Local.
6. Al vencer, el POS entra en **solo lectura**: conserva consulta, exportación y
   respaldo, y bloquea nuevas operaciones.

## Límites reconocidos

- Un PC totalmente offline y bajo control del usuario no permite impedir de
  forma absoluta la manipulación del reloj, la clonación o la revocación
  diferida.
- La política exacta de lease y gracia (D-04) no está decidida; el código actual
  no implementa gracia.
- La activación inicial sin Internet y la unicidad del Trial (D-03) están
  pendientes.

## Lo implementado en 0.1.0

- Contrato `LicenseGrant` y serialización canónica.
- Verificador Ed25519 con pruebas de firma válida, payload manipulado y clave
  incorrecta.
- Estado comercial con Trial de 30 días y transición a solo lectura probada con
  reloj fijo.
- Persistencia de estado comercial y bloqueo de mutaciones al vencer.

## Lo que **no** está implementado

- Emisión productiva de licencias (pertenece a HiQNet Platform).
- Rotación y revocación de claves.
- Activación offline por código firmado.
- Renovación, upgrade y downgrade.
- Cualquier cobro real.
