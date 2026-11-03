# Decisión de licencia de código — pendiente

El repositorio es **público** por decisión del propietario, pero eso no implica
que sea software de código abierto.

- No se ha añadido MIT, Apache-2.0, GPL ni ninguna otra licencia permisiva.
- No se concede permiso de reutilización, modificación ni redistribución del
  código hasta que el propietario decida la licencia.
- La licencia del **producto** (suscripción Trial/Local/Pro/Enterprise) es una
  decisión comercial distinta de la licencia del **código fuente**.
- Las dependencias de terceros conservan sus propias licencias.

## Pendiente

- Licencia de código fuente (propietaria, source-available con condiciones, u
  otra).
- Texto legal definitivo y revisión por asesoría.
- Contradicciones con licencias de dependencias (revisar con `cargo deny` y
  auditoría npm antes de distribuir).

Hasta entonces, `Cargo.toml` usa `license = "LicenseRef-Proprietary"` como
marcador, no como licencia concedida.
