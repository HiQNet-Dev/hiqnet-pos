# Pruebas

- Unitarias de dominio: ackend-rust/crates/pos-core (cargo test).
- Integración SQLite: ackend-rust/crates/pos-storage-sqlite/tests/persistence.rs.
- UI: rontend/src/App.test.tsx (Vitest).
- Smoke de interfaz: rontend/e2e/smoke.spec.ts (Playwright, navegador).
- E2E Tauri: pendiente de infraestructura WebDriver; no declarado como ejecutado.