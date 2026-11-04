# ADR-008 — Cuentas locales y revocación de sesión

- Estado: aceptado
- Fecha: 2026-11-04
- Contexto: G2.6 incorpora administración de empleados desde la aplicación. Hasta
  ahora el propietario era la única cuenta y la sesión vivía solo en memoria.

## Decisión

1. Los roles siguen siendo **fijos** (`owner`, `cashier`). No hay editor de
   permisos ni roles dinámicos; añadir uno exige un ADR propio.
2. Toda operación de administración exige `users.manage` verificado en Rust. La
   interfaz oculta la sección *Usuarios* a quien no lo tiene, pero la
   autorización nunca depende de esa ocultación.
3. **La sesión se revalida en cada comando.** La capa IPC guarda el `Actor` en
   memoria, pero antes de ejecutar cualquier caso de uso lo reconstruye desde
   `app_user`. Si la cuenta está inactiva o ya no existe, la sesión se descarta y
   el comando devuelve `unauthenticated`.
4. Cambiar la contraseña exige la contraseña actual; restablecerla exige
   `users.manage`. Ambos invalidan la contraseña anterior de inmediato.
5. No se puede desactivar al último propietario activo (`last_active_owner`),
   incluida la cuenta propia.
6. Cada operación sensible (alta, baja, alta de nuevo, restablecimiento y cambio
   de contraseña) se audita en la misma transacción. La auditoría guarda usuario y
   rol; nunca contraseñas ni hashes.
7. El esquema `app_user` ya contenía `role`, `active` y `password_hash`: G2.6 no
   requiere migración.

## Motivo

Un POS local no tiene servidor de sesiones. Sin revalidación, desactivar a un
empleado no surte efecto hasta que cierre sesión, y un `Actor` en memoria
conservaría permisos revocados. Revalidar contra SQLite es barato (una lectura
local por comando) y elimina esa ventana.

## Consecuencias

- Cada comando autenticado hace una lectura adicional de `app_user`. Se acepta
  por ser local y estar indexada por clave primaria.
- La UI debe tratar `unauthenticated` como cierre de sesión, no como error
  recuperable.
- Un propietario puede desactivarse a sí mismo solo si queda otro propietario
  activo; su sesión termina en el siguiente comando.
- El listado de usuarios se limita a la organización y sucursal del actor.

## Alternativas descartadas

- Invalidación por notificación entre procesos: no existe canal entre instancias
  y añadiría infraestructura de sincronización fuera de alcance.
- Tokens de sesión persistidos con expiración: el escritorio es de un solo
  operador por instancia y el estado en memoria ya se revalida.
- Roles dinámicos: pospuesto; multiplica la superficie de autorización sin una
  necesidad comercial demostrada.
