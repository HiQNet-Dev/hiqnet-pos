//! Devoluciones y reportes: pruebas de integracion sobre SQLite real.
//!
//! Cada escenario vive en su propio modulo; los fixtures, en `common`.

mod common;
mod concurrencia;
mod idempotencia;
mod integridad;
mod parciales;
mod permisos;
mod persistencia;
mod reportes;
mod rollback;
mod validaciones;
