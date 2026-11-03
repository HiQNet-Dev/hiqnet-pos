# Política de seguridad

## Reporte de vulnerabilidades

No abras un issue público con detalles explotables. Envía un correo al
propietario del proyecto o usa el reporte privado de GitHub
("Security → Report a vulnerability"). Incluye versión, sistema operativo,
pasos de reproducción e impacto.

## Fronteras de seguridad de este repositorio

- **IPC Tauri**: capabilities mínimas (`core:default`). No se habilitan plugins
  de filesystem, shell ni HTTP.
- **Autorización**: RBAC en Rust; la UI no es autoridad. Toda operación sensible
  exige `Actor` autenticado en memoria del proceso.
- **Entitlements**: se validan en Rust y son independientes de RBAC.
- **Credenciales**: Argon2id (19 MiB, 2 iteraciones, 1 hilo). No se registran
  contraseñas ni hashes.
- **SQL**: consultas parametrizadas; no se concatena entrada del usuario.
- **Datos**: base SQLite local con WAL; nunca se versiona.
- **Secretos**: `.env`, claves privadas, certificados y respaldos están excluidos
  por `.gitignore` y deben revisarse antes de cada push.
- **Licencias**: la clave privada de emisión solo puede existir en HiQNet
  Platform; este repositorio solo contiene claves públicas de verificación.

## Prácticas prohibidas

- Hardcodear tokens, claves o contraseñas.
- Publicar bases de datos, dumps, respaldos o fixtures con datos reales.
- Ejecutar comandos del sistema con entrada no confiable.
- Deshabilitar `foreign_keys`, triggers de auditoría o validaciones para "hacer
  pasar" una prueba.
- Añadir permisos globales de Tauri sin justificación documentada.

## Limitaciones conocidas

- Un PC offline controlado por el usuario no permite impedir de forma absoluta la
  manipulación de reloj, clonación de instalación o revocación diferida.
- No hay todavía rate limiting de intentos de login.
- No hay firma Authenticode ni updater firmado en 0.1.0.

## CI

Los workflows ejecutan formato, Clippy con `-D warnings`, pruebas, typecheck,
lint, build y un escaneo de archivos prohibidos. La auditoría de dependencias se
ejecuta con `rustsec/audit-check` y `pnpm audit` informativo.
