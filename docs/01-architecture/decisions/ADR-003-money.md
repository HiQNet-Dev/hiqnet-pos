# ADR-003 — Representación exacta de importes y cantidades

- Estado: aceptado
- Fecha: 2026-11-03

## Decisión

- `Money` almacena **centavos** en `i64` y expone `Decimal` solo para lectura y
  formateo. No admite valores negativos; los ajustes negativos se modelan como
  movimientos con signo explícito en el dominio correspondiente.
- `Quantity` almacena **milésimas** en `i64`, con hasta tres decimales.
- Las multiplicaciones redondean **half-up** a centavos de forma explícita.
- En JSON/IPC los importes viajan como cadena decimal (`"12.50"`), nunca como
  `number` de JavaScript.

## Motivo

`f64` no representa de forma exacta valores monetarios. Los errores de redondeo
en ventas, impuestos y arqueos son inaceptables.

## Pendiente

- Moneda: el MVP opera en MXN. La moneda no forma parte del value object todavía.
- Reglas fiscales mexicanas (CFDI/SAT) requieren análisis legal independiente.
- Ratificación de la política de redondeo por línea frente a total.

## Consecuencias

La UI valida formato con Zod, pero la autoridad del cálculo es Rust. Cualquier
cambio de escala exige migración de datos y una nueva versión del contrato.
